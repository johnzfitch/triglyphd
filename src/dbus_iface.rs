//! D-Bus interface for triglyphd.
//!
//! Service: org.freedesktop.Triglyph1
//! Object:  /org/freedesktop/Triglyph1
//!
//! Methods:
//!   Index(path: s) -> (success: b, message: s)
//!   Search(query: s) -> a(sttss)  // [(path, size, mtime, index_root, state)]
//!   SearchIn(path: s, query: s) -> a(sttss)
//!   Status(path: s) -> (path: s, file_count: t, indexed_at: s, state: s)
//!   List() -> a(stss)  // [(path, file_count, indexed_at, state)]
//!   Remove(path: s) -> (success: b, message: s)
//!   Cancel() -> (success: b, message: s)
//!
//! Also implements org.gnome.Shell.SearchProvider2 for GNOME integration.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;
use zbus::interface;
use zbus::zvariant::Value;

use crate::error::Error;
use crate::extract::ExtractConfig;
use crate::index_manager::IndexManager;
use crate::indexer::build_index_sync;
use crate::searcher::Searcher;

/// D-Bus interface implementation
pub struct TriglyphService {
    manager: Arc<IndexManager>,
}

impl TriglyphService {
    pub fn new(manager: Arc<IndexManager>) -> Self {
        Self { manager }
    }
}

#[interface(name = "org.freedesktop.Triglyph1")]
impl TriglyphService {
    /// Index a directory. Returns (success, message).
    /// Note: Progress signals are emitted synchronously during indexing.
    async fn index(&self, path: &str) -> (bool, String) {
        let path = PathBuf::from(path);

        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(e) => return (false, format!("Invalid path: {}", e)),
        };

        if !canonical.is_dir() {
            return (false, "Path is not a directory".to_string());
        }

        self.manager.reset_cancel();
        self.manager.set_indexing(Some(canonical.clone())).await;

        // Run indexing in blocking task (progress signals handled via separate poll)
        let cancel = self.manager.cancel_flag();
        let config = ExtractConfig::default();
        let canonical_clone = canonical.clone();

        let result = tokio::task::spawn_blocking(move || {
            build_index_sync(&canonical_clone, None, cancel, config)
        })
        .await;

        self.manager.set_indexing(None).await;
        self.manager.invalidate(&canonical).await;

        match result {
            Ok(Ok(stats)) => {
                if let Err(e) = self.manager.save_meta(&canonical, stats.file_count) {
                    return (false, format!("Failed to save metadata: {}", e));
                }
                (true, format!("Indexed {} files in {}ms", stats.file_count, stats.duration_ms))
            }
            Ok(Err(Error::Cancelled)) => (false, "Indexing cancelled".to_string()),
            Ok(Err(e)) => (false, e.to_string()),
            Err(e) => (false, format!("Task failed: {}", e)),
        }
    }

    /// Search all indexed directories.
    /// Returns array of (path, size, mtime, index_root).
    async fn search(&self, query: &str) -> Vec<(String, u64, u64, String)> {
        Searcher::search_all(&self.manager, query)
            .await
            .into_iter()
            .map(|r| (r.path, r.size, r.mtime, r.index_root))
            .collect()
    }

    /// Search a specific indexed directory.
    async fn search_in(&self, path: &str, query: &str) -> Vec<(String, u64, u64, String)> {
        let path = PathBuf::from(path);
        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };

        match Searcher::search_in(&self.manager, &canonical, query).await {
            Ok(results) => results
                .into_iter()
                .map(|r| (r.path, r.size, r.mtime, r.index_root))
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Get status of an index.
    /// Returns (path, file_count, indexed_at, state).
    async fn status(&self, path: &str) -> (String, u64, String, String) {
        let path = PathBuf::from(path);
        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => return (String::new(), 0, String::new(), "error".to_string()),
        };

        match self.manager.status(&canonical).await {
            Some(s) => (
                s.path.to_string_lossy().to_string(),
                s.file_count as u64,
                s.indexed_at.to_rfc3339(),
                s.state.as_str().to_string(),
            ),
            None => (String::new(), 0, String::new(), "not_indexed".to_string()),
        }
    }

    /// List all indexed directories.
    /// Returns array of (path, file_count, indexed_at, state).
    async fn list(&self) -> Vec<(String, u64, String, String)> {
        self.manager
            .list_all()
            .into_iter()
            .map(|s| (
                s.path.to_string_lossy().to_string(),
                s.file_count as u64,
                s.indexed_at.to_rfc3339(),
                s.state.as_str().to_string(),
            ))
            .collect()
    }

    /// Remove an index.
    async fn remove(&self, path: &str) -> (bool, String) {
        let path = PathBuf::from(path);
        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(e) => return (false, format!("Invalid path: {}", e)),
        };

        match self.manager.remove(&canonical).await {
            Ok(()) => (true, "Index removed".to_string()),
            Err(e) => (false, e.to_string()),
        }
    }

    /// Cancel current indexing operation.
    async fn cancel(&self) -> (bool, String) {
        if self.manager.get_indexing().await.is_some() {
            self.manager.cancel();
            (true, "Cancellation requested".to_string())
        } else {
            (true, "Nothing to cancel".to_string())
        }
    }

}

/// Well-known D-Bus name
pub const DBUS_NAME: &str = "org.freedesktop.Triglyph1";

/// D-Bus object path
pub const DBUS_PATH: &str = "/org/freedesktop/Triglyph1";

/// GNOME Shell Search Provider path
pub const SEARCH_PROVIDER_PATH: &str = "/org/freedesktop/Triglyph1/SearchProvider";

/// GNOME Shell SearchProvider2 implementation
/// Allows triglyphd to appear in GNOME Shell overview search
pub struct TriglyphSearchProvider {
    manager: Arc<IndexManager>,
    /// Cache of recent search results: result_id -> file_path
    result_cache: Arc<RwLock<HashMap<String, String>>>,
}

impl TriglyphSearchProvider {
    pub fn new(manager: Arc<IndexManager>) -> Self {
        Self {
            manager,
            result_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate a unique result ID from a file path
    fn path_to_id(path: &str) -> String {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        path.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}

#[interface(name = "org.gnome.Shell.SearchProvider2")]
impl TriglyphSearchProvider {
    /// Get initial search results for the given terms
    #[zbus(name = "GetInitialResultSet")]
    async fn get_initial_result_set(&self, terms: Vec<String>) -> Vec<String> {
        let query = terms.join(" ");
        if query.len() < 3 {
            return Vec::new();
        }

        let results = Searcher::search_all(&self.manager, &query).await;

        // Cache results and return IDs
        let mut cache = self.result_cache.write().await;
        cache.clear();

        results
            .into_iter()
            .take(20) // Limit results for UI
            .map(|r| {
                let id = Self::path_to_id(&r.path);
                cache.insert(id.clone(), r.path);
                id
            })
            .collect()
    }

    /// Refine search results based on previous results and new terms
    #[zbus(name = "GetSubsearchResultSet")]
    async fn get_subsearch_result_set(
        &self,
        _previous_results: Vec<String>,
        terms: Vec<String>,
    ) -> Vec<String> {
        // Just do a fresh search for simplicity
        self.get_initial_result_set(terms).await
    }

    /// Get metadata for the given result IDs
    #[zbus(name = "GetResultMetas")]
    async fn get_result_metas(
        &self,
        identifiers: Vec<String>,
    ) -> Vec<HashMap<String, Value<'static>>> {
        let cache = self.result_cache.read().await;

        identifiers
            .into_iter()
            .filter_map(|id| {
                let path = cache.get(&id)?;
                let file_name = std::path::Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(path);

                let mut meta: HashMap<String, Value<'static>> = HashMap::new();
                meta.insert("id".to_string(), Value::new(id));
                meta.insert("name".to_string(), Value::new(file_name.to_string()));
                meta.insert("description".to_string(), Value::new(path.clone()));
                meta.insert("gicon".to_string(), Value::new("text-x-generic".to_string()));

                Some(meta)
            })
            .collect()
    }

    /// Activate (open) a result
    #[zbus(name = "ActivateResult")]
    async fn activate_result(&self, identifier: String, _terms: Vec<String>, _timestamp: u32) {
        let cache = self.result_cache.read().await;
        if let Some(path) = cache.get(&identifier) {
            // Open with default application
            let _ = std::process::Command::new("xdg-open")
                .arg(path)
                .spawn();
        }
    }

    /// Launch the search application with the given terms
    #[zbus(name = "LaunchSearch")]
    async fn launch_search(&self, terms: Vec<String>, _timestamp: u32) {
        // Open file manager with search query
        let query = terms.join(" ");
        let _ = std::process::Command::new("nautilus")
            .arg("--search")
            .arg(&query)
            .spawn();
    }
}
