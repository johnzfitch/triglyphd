use std::path::Path;

use triglyph::{extract_trigrams, search_trigrams};

use crate::error::Result;
use crate::index_manager::{IndexManager, LoadedIndex};

/// A single search result
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub path: String,
    pub size: u64,
    pub mtime: u64,
    pub index_root: String,
}

/// Search across indices
pub struct Searcher;

impl Searcher {
    /// Search all indexed directories
    pub async fn search_all(manager: &IndexManager, query: &str) -> Vec<SearchResult> {
        let trigrams = extract_trigrams(query);
        if trigrams.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();

        for status in manager.list_all() {
            if let Ok(index) = manager.load(&status.path).await {
                results.extend(Self::search_index(&index, &trigrams));
            }
        }

        results.sort_by(|a, b| b.mtime.cmp(&a.mtime));
        results
    }

    /// Search a specific indexed directory
    pub async fn search_in(
        manager: &IndexManager,
        path: &Path,
        query: &str,
    ) -> Result<Vec<SearchResult>> {
        let trigrams = extract_trigrams(query);
        if trigrams.is_empty() {
            return Ok(Vec::new());
        }

        let index = manager.load(path).await?;
        let mut results = Self::search_index(&index, &trigrams);
        results.sort_by(|a, b| b.mtime.cmp(&a.mtime));
        Ok(results)
    }

    fn search_index(index: &LoadedIndex, trigrams: &[u32]) -> Vec<SearchResult> {
        let doc_ids = match search_trigrams(&index.reader, index.presence.as_ref(), trigrams) {
            Ok(ids) => ids,
            Err(_) => return Vec::new(),
        };

        let mut results = Vec::new();

        for doc_id in doc_ids {
            if let Ok(entry) = index.files.get(doc_id) {
                results.push(SearchResult {
                    path: entry.path.to_string(),
                    size: entry.size,
                    mtime: entry.mtime,
                    index_root: index.root.to_string_lossy().into_owned(),
                });
            }
        }

        results
    }
}
