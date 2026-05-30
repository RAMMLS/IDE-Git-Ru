//! Core primitives for the Aura version control system.
//!
//! The crate exposes an async [`Repository`] API that operates on a working tree
//! containing a hidden `.aura` directory. Objects are stored similarly to Git:
//! the object id is a SHA-1 hash over `"<type> <len>\0<body>"`, the payload is
//! compressed with zlib, and objects are placed under `.aura/objects/xx/yyyy...`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod index;
pub mod object;
pub mod pack;
pub mod refs;
mod repository;

pub use repository::{
    ChangeKind, ChangeStats, CommitOutcome, CommitSummary, DiffLine, FetchSummary, FileChangeStat,
    FileDiff, MergeStatus, MergeSummary, PullStatus, PullSummary, PushSummary, RebaseStatus,
    RebaseSummary, RepositoryStatus, StatusEntry,
};

/// Name of the metadata directory used by Aura repositories.
pub const AURA_DIR: &str = ".aura";

/// Default author and committer used for all Aura commits.
pub const DEFAULT_AUTHOR: &str = "Aura User <aura@local>";

/// Result alias used across the crate.
pub type Result<T> = std::result::Result<T, AuraError>;

/// Errors returned by Aura repository operations.
#[derive(Debug, Error)]
pub enum AuraError {
    /// A filesystem operation failed.
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
    /// Binary index serialization or deserialization failed.
    #[error("bincode error: {0}")]
    Bincode(#[from] bincode::Error),
    /// UTF-8 conversion failed while reading textual metadata.
    #[error("utf-8 decoding error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    /// Hex conversion failed while decoding object ids.
    #[error("hex decoding error: {0}")]
    Hex(#[from] hex::FromHexError),
    /// HTTP transport request failed.
    #[error("http transport error: {0}")]
    Reqwest(#[from] reqwest::Error),
    /// A path does not belong to the repository worktree.
    #[error("path `{0}` is outside of the repository root")]
    PathOutsideRepository(String),
    /// The repository has not been initialized yet.
    #[error("`{0}` does not look like an Aura repository")]
    NotRepository(String),
    /// A repository reference contains invalid data.
    #[error("invalid reference `{0}`")]
    InvalidReference(String),
    /// The `HEAD` file contains invalid data.
    #[error("invalid HEAD contents")]
    InvalidHead,
    /// An object with the requested identifier could not be found.
    #[error("object `{0}` not found")]
    ObjectNotFound(String),
    /// An object exists but does not match the expected type.
    #[error("object `{oid}` has type `{actual}`, expected `{expected}`")]
    UnexpectedObjectType {
        /// The object id that was loaded.
        oid: String,
        /// The actual object type present on disk.
        actual: String,
        /// The type expected by the caller.
        expected: String,
    },
    /// An object could not be parsed from its raw representation.
    #[error("corrupt object `{0}`")]
    CorruptObject(String),
    /// A transport packfile is malformed or unsupported.
    #[error("invalid packfile: {0}")]
    InvalidPackfile(String),
    /// A named reference could not be resolved.
    #[error("reference `{0}` not found")]
    ReferenceNotFound(String),
    /// The current repository state requires a symbolic `HEAD`.
    #[error("operation requires a symbolic HEAD, but the repository is detached")]
    DetachedHead,
    /// A branch already exists.
    #[error("branch `{0}` already exists")]
    BranchExists(String),
    /// A branch could not be located.
    #[error("branch `{0}` does not exist")]
    BranchNotFound(String),
    /// A remote could not be located.
    #[error("remote `{0}` does not exist")]
    RemoteNotFound(String),
    /// A remote branch could not be located.
    #[error("remote branch `{remote}/{branch}` does not exist")]
    RemoteBranchNotFound {
        /// Remote name.
        remote: String,
        /// Branch name.
        branch: String,
    },
    /// Pull cannot proceed because the local branch diverged from the remote branch.
    #[error("cannot fast-forward local branch `{local}` from `{remote}`")]
    DivergedBranches {
        /// Local branch name.
        local: String,
        /// Remote branch identifier.
        remote: String,
    },
    /// A three-way merge or rebase detected overlapping edits.
    #[error("conflicting changes in: {0}")]
    ConflictingChanges(String),
    /// Checkout or pull cannot proceed while staged or unstaged tracked changes are present.
    #[error("working tree has staged or unstaged changes; commit or stash them before switching branches or pulling")]
    WorkingTreeNotClean,
    /// Checkout or pull would overwrite an untracked path in the working tree.
    #[error("operation would overwrite untracked path `{0}`")]
    UntrackedWouldBeOverwritten(String),
    /// Remote transport returned an invalid response.
    #[error("transport error: {0}")]
    Transport(String),
}

/// Timestamp captured from the backing filesystem.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileTimestamp {
    /// Whole seconds since the Unix epoch.
    pub seconds: i64,
    /// Nanoseconds within the current second.
    pub nanoseconds: u32,
}

impl FileTimestamp {
    /// Creates a timestamp from a [`SystemTime`].
    pub fn from_system_time(time: SystemTime) -> Self {
        match time.duration_since(UNIX_EPOCH) {
            Ok(duration) => Self {
                seconds: duration.as_secs() as i64,
                nanoseconds: duration.subsec_nanos(),
            },
            Err(_) => Self::default(),
        }
    }
}

/// Metadata returned by the [`FileSystem`] abstraction.
#[derive(Debug, Clone, Default)]
pub struct FsMetadata {
    /// Whether the path points to a regular file.
    pub is_file: bool,
    /// Whether the path points to a directory.
    pub is_dir: bool,
    /// File size in bytes.
    pub len: u64,
    /// Creation timestamp when the platform provides it.
    pub created: Option<FileTimestamp>,
    /// Last modification timestamp when the platform provides it.
    pub modified: Option<FileTimestamp>,
}

/// Async filesystem abstraction used by the repository.
///
/// The trait makes the storage layer replaceable, which is useful when Aura is
/// embedded in environments such as WASM where direct filesystem access is not
/// always available.
#[async_trait]
pub trait FileSystem: Send + Sync {
    /// Creates a directory and all missing parent directories.
    async fn create_dir_all(&self, path: &Path) -> Result<()>;

    /// Reads an entire file into memory.
    async fn read(&self, path: &Path) -> Result<Vec<u8>>;

    /// Writes the provided bytes, replacing the destination when it exists.
    async fn write(&self, path: &Path, contents: &[u8]) -> Result<()>;

    /// Returns `true` when the path exists.
    async fn exists(&self, path: &Path) -> Result<bool>;

    /// Returns metadata for the given path.
    async fn metadata(&self, path: &Path) -> Result<FsMetadata>;

    /// Lists the direct children of a directory.
    async fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>>;

    /// Removes a file when it exists.
    async fn remove_file(&self, path: &Path) -> Result<()>;

    /// Reads a UTF-8 text file into a string.
    async fn read_to_string(&self, path: &Path) -> Result<String> {
        Ok(String::from_utf8(self.read(path).await?)?)
    }
}

/// Default [`FileSystem`] implementation backed by `tokio::fs`.
#[derive(Debug, Default)]
pub struct TokioFileSystem;

#[async_trait]
impl FileSystem for TokioFileSystem {
    async fn create_dir_all(&self, path: &Path) -> Result<()> {
        tokio::fs::create_dir_all(path).await?;
        Ok(())
    }

    async fn read(&self, path: &Path) -> Result<Vec<u8>> {
        Ok(tokio::fs::read(path).await?)
    }

    async fn write(&self, path: &Path, contents: &[u8]) -> Result<()> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, contents).await?;
        Ok(())
    }

    async fn exists(&self, path: &Path) -> Result<bool> {
        Ok(tokio::fs::try_exists(path).await?)
    }

    async fn metadata(&self, path: &Path) -> Result<FsMetadata> {
        let metadata = tokio::fs::metadata(path).await?;
        Ok(FsMetadata {
            is_file: metadata.is_file(),
            is_dir: metadata.is_dir(),
            len: metadata.len(),
            created: metadata.created().ok().map(FileTimestamp::from_system_time),
            modified: metadata
                .modified()
                .ok()
                .map(FileTimestamp::from_system_time),
        })
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let mut entries = Vec::new();
        let mut reader = tokio::fs::read_dir(path).await?;
        while let Some(entry) = reader.next_entry().await? {
            entries.push(entry.path());
        }
        entries.sort();
        Ok(entries)
    }

    async fn remove_file(&self, path: &Path) -> Result<()> {
        if tokio::fs::try_exists(path).await? {
            tokio::fs::remove_file(path).await?;
        }
        Ok(())
    }
}

/// Main Aura repository handle.
///
/// The public structure keeps the repository root path visible as requested by
/// the project API. All operations are implemented asynchronously.
#[derive(Clone)]
pub struct Repository {
    /// Root of the working tree that contains `.aura`.
    pub path: PathBuf,
    fs: Arc<dyn FileSystem>,
}

impl std::fmt::Debug for Repository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Repository")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl Repository {
    /// Creates a repository handle backed by the default Tokio filesystem.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self::with_fs(path, Arc::new(TokioFileSystem))
    }

    /// Creates a repository handle with a custom filesystem implementation.
    pub fn with_fs(path: impl Into<PathBuf>, fs: Arc<dyn FileSystem>) -> Self {
        Self {
            path: path.into(),
            fs,
        }
    }

    /// Returns the absolute path to the `.aura` metadata directory.
    pub fn aura_dir(&self) -> PathBuf {
        self.path.join(AURA_DIR)
    }

    /// Exposes the active filesystem implementation for advanced integrations.
    pub fn filesystem(&self) -> &dyn FileSystem {
        self.fs.as_ref()
    }
}
