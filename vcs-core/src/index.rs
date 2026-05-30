//! Binary index handling for Aura repositories.
//!
//! The Aura index is stored at `.aura/index` and serialized with `bincode`. Each
//! entry records the repository-relative path, a simplified file mode, the blob
//! id and a small metadata snapshot used by higher-level operations.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{FileSystem, FileTimestamp, FsMetadata, Result, AURA_DIR};

/// Timestamp snapshot stored inside the Aura index.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct IndexTimestamp {
    /// Whole seconds since the Unix epoch.
    pub seconds: i64,
    /// Nanoseconds within the current second.
    pub nanoseconds: u32,
}

impl From<Option<FileTimestamp>> for IndexTimestamp {
    fn from(value: Option<FileTimestamp>) -> Self {
        value
            .map(|timestamp| Self {
                seconds: timestamp.seconds,
                nanoseconds: timestamp.nanoseconds,
            })
            .unwrap_or_default()
    }
}

/// Single file entry inside the Aura index.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexEntry {
    /// Repository-relative file path using `/` separators.
    pub path: String,
    /// File mode. Aura currently writes regular files as `100644`.
    pub mode: u32,
    /// Blob id of the indexed contents.
    pub oid: String,
    /// File size in bytes captured at add/checkout time.
    pub size: u64,
    /// Creation timestamp snapshot.
    pub created: IndexTimestamp,
    /// Last modification timestamp snapshot.
    pub modified: IndexTimestamp,
}

/// Complete Aura index file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexFile {
    /// On-disk format version.
    pub version: u32,
    /// Indexed files.
    pub entries: Vec<IndexEntry>,
}

impl Default for IndexFile {
    fn default() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
        }
    }
}

impl IndexFile {
    /// Inserts or replaces an entry while keeping the index sorted by path.
    pub fn upsert(&mut self, entry: IndexEntry) {
        if let Some(slot) = self
            .entries
            .iter_mut()
            .find(|existing| existing.path == entry.path)
        {
            *slot = entry;
        } else {
            self.entries.push(entry);
        }
        self.entries
            .sort_by(|left, right| left.path.cmp(&right.path));
    }

    /// Removes an entry by repository-relative path.
    pub fn remove(&mut self, path: &str) -> Option<IndexEntry> {
        let index = self.entries.iter().position(|entry| entry.path == path)?;
        Some(self.entries.remove(index))
    }

    /// Returns an immutable entry by path.
    pub fn get(&self, path: &str) -> Option<&IndexEntry> {
        self.entries.iter().find(|entry| entry.path == path)
    }
}

/// Returns the path to `.aura/index`.
pub fn index_path(repo_path: &Path) -> std::path::PathBuf {
    repo_path.join(AURA_DIR).join("index")
}

/// Loads the index file or returns an empty one when it does not exist yet.
pub async fn load_index(fs: &dyn FileSystem, repo_path: &Path) -> Result<IndexFile> {
    let path = index_path(repo_path);
    if !fs.exists(&path).await? {
        return Ok(IndexFile::default());
    }

    let bytes = fs.read(&path).await?;
    Ok(bincode::deserialize(&bytes)?)
}

/// Saves the index file back to `.aura/index`.
pub async fn save_index(fs: &dyn FileSystem, repo_path: &Path, index: &IndexFile) -> Result<()> {
    let bytes = bincode::serialize(index)?;
    fs.write(&index_path(repo_path), &bytes).await
}

/// Builds an [`IndexEntry`] from filesystem metadata.
pub fn entry_from_metadata(
    path: String,
    mode: u32,
    oid: String,
    metadata: &FsMetadata,
) -> IndexEntry {
    IndexEntry {
        path,
        mode,
        oid,
        size: metadata.len,
        created: IndexTimestamp::from(metadata.created),
        modified: IndexTimestamp::from(metadata.modified),
    }
}
