use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Instant, UNIX_EPOCH};

use ignore::WalkBuilder;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use tokio::sync::mpsc;

use triglyph::{build_atomic_index, build_file_list, IndexedFile};

use crate::error::{Error, Result};
use crate::extract::{extract_file_trigrams, is_indexable, ExtractConfig};
use crate::paths::{self, IndexPaths};

/// Progress update for D-Bus signals
#[derive(Debug, Clone)]
pub enum Progress {
    Scanning { files_found: usize },
    Indexing { current: usize, total: usize },
    Building,
    Complete { file_count: usize, duration_ms: u64 },
    Error(String),
}

/// Result of indexing operation
#[derive(Debug, Clone)]
pub struct IndexStats {
    pub file_count: usize,
    pub duration_ms: u64,
}

/// Format number with k/m suffix
fn fmt_num(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}m", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{}k", n / 1_000)
    } else {
        n.to_string()
    }
}

/// Build an index for a directory (blocking, run in spawn_blocking)
pub fn build_index_sync(
    root: &Path,
    progress_tx: Option<mpsc::UnboundedSender<Progress>>,
    cancel: Arc<AtomicBool>,
    config: ExtractConfig,
) -> Result<IndexStats> {
    let start = Instant::now();
    let root = root.canonicalize()?;

    let paths = IndexPaths::new(&root);
    paths::ensure_index_dir(&root)?;

    // Single progress bar for all phases
    let pb = ProgressBar::new(100);
    pb.set_style(ProgressStyle::default_bar()
        .template("Forging [{bar:30.cyan/blue}] {msg}")
        .unwrap()
        .progress_chars("=>-"));
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    // Phase 1: Walk and collect files (0-20%)
    send_progress(&progress_tx, Progress::Scanning { files_found: 0 });
    pb.set_position(0);
    pb.set_message("scan: 0");

    let mut files: Vec<PathBuf> = Vec::new();

    let walker = WalkBuilder::new(&root)
        .hidden(false)
        .git_ignore(config.respect_gitignore)
        .git_global(config.respect_gitignore)
        .git_exclude(config.respect_gitignore)
        .follow_links(config.follow_symlinks)
        .filter_entry(|entry| {
            entry.file_name().to_str().map(|s| {
                !matches!(s, ".git" | ".hg" | ".svn" | "node_modules" | "__pycache__" | ".cache" | "target" | "build" | "dist")
            }).unwrap_or(true)
        })
        .build();

    for entry in walker.flatten() {
        if cancel.load(Ordering::Relaxed) {
            pb.finish_and_clear();
            return Err(Error::Cancelled);
        }

        let path = entry.path();
        if is_indexable(path, &config) {
            files.push(path.to_path_buf());

            if files.len() % 5000 == 0 {
                pb.set_message(format!("scan: {}", fmt_num(files.len())));
                send_progress(&progress_tx, Progress::Scanning { files_found: files.len() });
            }
        }
    }

    let total = files.len();
    pb.set_position(20);
    pb.set_message(format!("scan: {} | extract: 0/{}", fmt_num(total), fmt_num(total)));
    send_progress(&progress_tx, Progress::Scanning { files_found: total });

    // Phase 2: Extract trigrams (20-70%)
    let extracted_count = AtomicUsize::new(0);
    let cancel_ref = &cancel;
    let extracted_ref = &extracted_count;

    let extracted: Vec<_> = files
        .par_iter()
        .filter_map(|path| {
            if cancel_ref.load(Ordering::Relaxed) {
                return None;
            }

            let trigrams = extract_file_trigrams(path)?;
            let meta = path.metadata().ok()?;
            let mtime = meta
                .modified()
                .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs())
                .unwrap_or(0);

            let count = extracted_ref.fetch_add(1, Ordering::Relaxed) + 1;
            if count % 10000 == 0 || count == total {
                let pct = 20 + (count * 50 / total);
                pb.set_position(pct as u64);
                pb.set_message(format!("scan: {} | extract: {}/{}",
                    fmt_num(total), fmt_num(count), fmt_num(total)));
            }

            Some((path.to_string_lossy().into_owned(), mtime, meta.len(), trigrams))
        })
        .collect();

    if cancel.load(Ordering::Relaxed) {
        pb.finish_and_clear();
        return Err(Error::Cancelled);
    }

    // Phase 3: Merge postings (70-90%)
    let extracted_len = extracted.len();
    pb.set_position(70);
    pb.set_message(format!("scan: {} | extract: {} | merge: 0/{}",
        fmt_num(total), fmt_num(extracted_len), fmt_num(extracted_len)));

    let mut postings: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut indexed_files: Vec<IndexedFile> = Vec::with_capacity(extracted_len);

    for (i, (path, mtime, size, trigrams)) in extracted.into_iter().enumerate() {
        let doc_id = indexed_files.len() as u32;
        indexed_files.push(IndexedFile { path, mtime, size });

        for trigram in trigrams {
            postings.entry(trigram).or_default().push(doc_id);
        }

        if (i + 1) % 50000 == 0 {
            let pct = 70 + ((i + 1) * 20 / extracted_len);
            pb.set_position(pct as u64);
            pb.set_message(format!("scan: {} | extract: {} | merge: {}/{} | {}tri",
                fmt_num(total), fmt_num(extracted_len), fmt_num(i + 1), fmt_num(extracted_len), fmt_num(postings.len())));
        }
    }

    send_progress(&progress_tx, Progress::Indexing { current: total, total });

    // Phase 4: Write index (90-100%)
    send_progress(&progress_tx, Progress::Building);

    if cancel.load(Ordering::Relaxed) {
        pb.finish_and_clear();
        return Err(Error::Cancelled);
    }

    pb.set_position(90);
    pb.set_message(format!("{} files | {}tri | writing index...",
        fmt_num(indexed_files.len()), fmt_num(postings.len())));
    build_atomic_index(&paths.index, &postings, |t| Some(t as usize))?;

    if cancel.load(Ordering::Relaxed) {
        let _ = paths.remove_all();
        pb.finish_and_clear();
        return Err(Error::Cancelled);
    }

    pb.set_position(95);
    pb.set_message(format!("{} files | {}tri | writing files...",
        fmt_num(indexed_files.len()), fmt_num(postings.len())));
    build_file_list(&paths.files_str, &paths.files_dir, &indexed_files)?;

    pb.set_position(100);
    pb.finish_with_message(format!("{} files | {}tri | done",
        fmt_num(indexed_files.len()), fmt_num(postings.len())));

    let duration = start.elapsed();
    let stats = IndexStats {
        file_count: indexed_files.len(),
        duration_ms: duration.as_millis() as u64,
    };

    send_progress(&progress_tx, Progress::Complete {
        file_count: stats.file_count,
        duration_ms: stats.duration_ms,
    });

    Ok(stats)
}

fn send_progress(tx: &Option<mpsc::UnboundedSender<Progress>>, progress: Progress) {
    if let Some(tx) = tx {
        let _ = tx.send(progress);
    }
}
