use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;

/// Base directory: ~/.local/share/triglyph/
pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from(".local/share"))
        .join("triglyph")
}

/// Index directory for a specific path: ~/.local/share/triglyph/<hash>/
pub fn index_dir_for(path: &Path) -> PathBuf {
    data_dir().join(hash_path(path))
}

/// Hash a path to a 16-character hex string using BLAKE3.
pub fn hash_path(path: &Path) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let hash = blake3::hash(canonical.to_string_lossy().as_bytes());
    let bytes = hash.as_bytes();
    format!(
        "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5], bytes[6], bytes[7]
    )
}

/// Ensure the data directory exists.
pub fn ensure_data_dir() -> Result<PathBuf> {
    let dir = data_dir();
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Ensure an index directory exists.
pub fn ensure_index_dir(path: &Path) -> Result<PathBuf> {
    let dir = index_dir_for(path);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Index file paths within an index directory.
pub struct IndexPaths {
    pub dir: PathBuf,
    pub index: PathBuf,
    pub presence: PathBuf,
    pub files_str: PathBuf,
    pub files_dir: PathBuf,
    pub meta: PathBuf,
}

impl IndexPaths {
    pub fn new(root: &Path) -> Self {
        let dir = index_dir_for(root);
        Self {
            index: dir.join("index.tri"),
            presence: dir.join("index.tri.presence"),
            files_str: dir.join("index.files.str"),
            files_dir: dir.join("index.files.dir"),
            meta: dir.join("meta.json"),
            dir,
        }
    }

    pub fn exists(&self) -> bool {
        self.index.exists() && self.files_dir.exists() && self.meta.exists()
    }

    pub fn remove_all(&self) -> std::io::Result<()> {
        if self.dir.exists() {
            fs::remove_dir_all(&self.dir)
        } else {
            Ok(())
        }
    }
}
