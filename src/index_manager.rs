use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use memmap2::Mmap;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use triglyph::{FileListReader, IndexReader, PresenceBitset};

use crate::error::{Error, Result};
use crate::paths::{self, IndexPaths};

/// Index state for D-Bus reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexState {
    Ready,
    Indexing,
    Error,
}

impl IndexState {
    pub fn as_str(&self) -> &'static str {
        match self {
            IndexState::Ready => "ready",
            IndexState::Indexing => "indexing",
            IndexState::Error => "error",
        }
    }
}

/// Metadata stored alongside the index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMeta {
    pub root: PathBuf,
    pub indexed_at: DateTime<Utc>,
    pub file_count: usize,
    pub version: String,
}

impl IndexMeta {
    pub fn new(root: PathBuf, file_count: usize) -> Self {
        Self {
            root,
            indexed_at: Utc::now(),
            file_count,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}

/// A loaded index ready for querying
pub struct LoadedIndex {
    pub root: PathBuf,
    pub meta: IndexMeta,
    pub reader: IndexReader<'static>,
    pub presence: Option<PresenceBitset>,
    pub files: FileListReader<'static>,
    // Keep mmaps alive (leaked for 'static lifetime)
    _index_mmap: &'static Mmap,
    _str_mmap: &'static Mmap,
    _dir_mmap: &'static Mmap,
}

impl LoadedIndex {
    fn load(paths: &IndexPaths) -> Result<Self> {
        let meta = IndexMeta::load(&paths.meta)?;

        let index_file = fs::File::open(&paths.index)?;
        let str_file = fs::File::open(&paths.files_str)?;
        let dir_file = fs::File::open(&paths.files_dir)?;

        // Leak mmaps for 'static lifetime (long-lived daemon)
        let index_mmap = Box::leak(Box::new(unsafe { Mmap::map(&index_file)? }));
        let str_mmap = Box::leak(Box::new(unsafe { Mmap::map(&str_file)? }));
        let dir_mmap = Box::leak(Box::new(unsafe { Mmap::map(&dir_file)? }));

        let reader = IndexReader::open(index_mmap)
            .map_err(|e| Error::Index(e.to_string()))?;

        let files = FileListReader::open(str_mmap, dir_mmap)
            .map_err(|e| Error::Index(e.to_string()))?;

        let presence = PresenceBitset::load(&paths.presence).ok();

        Ok(Self {
            root: meta.root.clone(),
            meta,
            reader,
            presence,
            files,
            _index_mmap: index_mmap,
            _str_mmap: str_mmap,
            _dir_mmap: dir_mmap,
        })
    }
}

/// Index status for D-Bus reporting
#[derive(Debug, Clone)]
pub struct IndexStatus {
    pub path: PathBuf,
    pub file_count: usize,
    pub indexed_at: DateTime<Utc>,
    pub state: IndexState,
}

/// Thread-safe index manager with async support
pub struct IndexManager {
    cache: RwLock<HashMap<PathBuf, Arc<LoadedIndex>>>,
    cancel_flag: Arc<AtomicBool>,
    indexing: RwLock<Option<PathBuf>>,
}

impl IndexManager {
    pub fn new() -> Result<Self> {
        paths::ensure_data_dir()?;
        Ok(Self {
            cache: RwLock::new(HashMap::new()),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            indexing: RwLock::new(None),
        })
    }

    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel_flag.clone()
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::Relaxed)
    }

    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }

    pub fn reset_cancel(&self) {
        self.cancel_flag.store(false, Ordering::Relaxed);
    }

    pub async fn set_indexing(&self, path: Option<PathBuf>) {
        *self.indexing.write().await = path;
    }

    pub async fn get_indexing(&self) -> Option<PathBuf> {
        self.indexing.read().await.clone()
    }

    pub fn is_indexed(&self, path: &Path) -> bool {
        IndexPaths::new(path).exists()
    }

    pub async fn status(&self, path: &Path) -> Option<IndexStatus> {
        let paths = IndexPaths::new(path);
        if !paths.meta.exists() {
            return None;
        }

        let meta = IndexMeta::load(&paths.meta).ok()?;
        let state = if self.get_indexing().await.as_deref() == Some(path) {
            IndexState::Indexing
        } else if paths.index.exists() {
            IndexState::Ready
        } else {
            IndexState::Error
        };

        Some(IndexStatus {
            path: meta.root,
            file_count: meta.file_count,
            indexed_at: meta.indexed_at,
            state,
        })
    }

    pub fn list_all(&self) -> Vec<IndexStatus> {
        let data_dir = paths::data_dir();
        let mut results = Vec::new();

        if let Ok(entries) = fs::read_dir(&data_dir) {
            for entry in entries.flatten() {
                let meta_path = entry.path().join("meta.json");
                if meta_path.exists() {
                    if let Ok(meta) = IndexMeta::load(&meta_path) {
                        let paths = IndexPaths::new(&meta.root);
                        let state = if paths.index.exists() {
                            IndexState::Ready
                        } else {
                            IndexState::Error
                        };
                        results.push(IndexStatus {
                            path: meta.root,
                            file_count: meta.file_count,
                            indexed_at: meta.indexed_at,
                            state,
                        });
                    }
                }
            }
        }

        results.sort_by(|a, b| b.indexed_at.cmp(&a.indexed_at));
        results
    }

    pub async fn load(&self, path: &Path) -> Result<Arc<LoadedIndex>> {
        // Check cache
        {
            let cache = self.cache.read().await;
            if let Some(index) = cache.get(path) {
                return Ok(index.clone());
            }
        }

        // Load from disk
        let paths = IndexPaths::new(path);
        if !paths.exists() {
            return Err(Error::NotIndexed(path.display().to_string()));
        }

        let index = Arc::new(LoadedIndex::load(&paths)?);

        // Update cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(path.to_path_buf(), index.clone());
        }

        Ok(index)
    }

    pub async fn invalidate(&self, path: &Path) {
        let mut cache = self.cache.write().await;
        cache.remove(path);
    }

    pub async fn remove(&self, path: &Path) -> Result<()> {
        self.invalidate(path).await;
        let paths = IndexPaths::new(path);
        paths.remove_all()?;
        Ok(())
    }

    pub fn save_meta(&self, path: &Path, file_count: usize) -> Result<()> {
        let paths = IndexPaths::new(path);
        let meta = IndexMeta::new(path.to_path_buf(), file_count);
        meta.save(&paths.meta)?;
        Ok(())
    }
}

impl Default for IndexManager {
    fn default() -> Self {
        Self::new().expect("Failed to initialize index manager")
    }
}
