//! High-level repository operations for Aura.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_recursion::async_recursion;
use chrono::{DateTime, Utc};

use crate::index::{self, IndexFile};
use crate::object::{self, Blob, Commit, Tree, TreeEntry};
use crate::refs::{self, HeadRef};
use crate::{AuraError, FileSystem, Repository, Result, DEFAULT_AUTHOR};

const FILE_MODE_INDEX: u32 = 100644;
const FILE_MODE_TREE: &str = "100644";
const DIRECTORY_MODE_TREE: &str = "40000";

/// Summary returned by [`Repository::log`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSummary {
    /// Commit id.
    pub oid: String,
    /// Root tree id.
    pub tree: String,
    /// Parent commit ids.
    pub parents: Vec<String>,
    /// Commit author.
    pub author: String,
    /// Commit message.
    pub message: String,
    /// Commit timestamp in UTC.
    pub timestamp: DateTime<Utc>,
}

/// Type of change reported by [`Repository::status`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// File exists in the target snapshot but not in the base snapshot.
    Added,
    /// File exists in both snapshots with different blob ids.
    Modified,
    /// File exists in the base snapshot but not in the target snapshot.
    Deleted,
}

/// Single file change returned by [`Repository::status`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    /// Repository-relative file path.
    pub path: PathBuf,
    /// Change kind for the path.
    pub kind: ChangeKind,
}

/// Full repository state returned by [`Repository::status`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryStatus {
    /// Current branch name, or `None` for detached `HEAD`.
    pub branch: Option<String>,
    /// Current `HEAD` commit id when available.
    pub head: Option<String>,
    /// Changes staged in the index compared with `HEAD`.
    pub staged: Vec<StatusEntry>,
    /// Changes in the working tree compared with the index.
    pub unstaged: Vec<StatusEntry>,
    /// Files that are present only in the working tree.
    pub untracked: Vec<PathBuf>,
}

/// Result returned by [`Repository::push`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushSummary {
    /// Remote name used for the push.
    pub remote: String,
    /// Branch name updated on the remote side.
    pub branch: String,
    /// Commit id written to the remote branch.
    pub oid: String,
    /// Target repository path for the remote.
    pub target: PathBuf,
}

/// Per-file line statistics for a tree-to-tree change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChangeStat {
    /// Repository-relative file path.
    pub path: PathBuf,
    /// Change kind for the file.
    pub kind: ChangeKind,
    /// Number of added lines.
    pub insertions: usize,
    /// Number of removed lines.
    pub deletions: usize,
}

/// Aggregate line statistics for a tree-to-tree change.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChangeStats {
    /// Number of paths that changed.
    pub files_changed: usize,
    /// Number of added lines across all changed files.
    pub insertions: usize,
    /// Number of removed lines across all changed files.
    pub deletions: usize,
    /// Per-file statistics sorted by path.
    pub files: Vec<FileChangeStat>,
}

/// Summary returned by [`Repository::commit_with_summary`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitOutcome {
    /// Commit id.
    pub oid: String,
    /// Active branch name when `HEAD` is symbolic.
    pub branch: Option<String>,
    /// Commit message used for the new commit.
    pub message: String,
    /// Diff statistics between the parent commit and the new commit.
    pub stats: ChangeStats,
}

/// Result mode returned by [`Repository::pull`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullStatus {
    /// Local branch already contains the requested remote commit.
    AlreadyUpToDate,
    /// Local branch was advanced to the remote commit without a merge commit.
    FastForward,
}

/// Summary returned by [`Repository::pull`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullSummary {
    /// Remote name used for the pull.
    pub remote: String,
    /// Remote branch used as the pull source.
    pub source_branch: String,
    /// Local branch that received the update.
    pub local_branch: String,
    /// Commit id written or confirmed locally.
    pub oid: String,
    /// Local branch tip before the pull, when it existed.
    pub previous_oid: Option<String>,
    /// Pull result mode.
    pub status: PullStatus,
    /// Diff statistics for a fast-forward update.
    pub stats: Option<ChangeStats>,
    /// Source repository path for the remote.
    pub target: PathBuf,
}

/// One line in a line-oriented diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    /// Unchanged line.
    Context(String),
    /// Added line.
    Addition(String),
    /// Removed line.
    Deletion(String),
}

/// Diff output for a single file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    /// Repository-relative file path.
    pub path: PathBuf,
    /// Myers diff output.
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Default)]
struct TreeNode {
    files: BTreeMap<String, String>,
    directories: BTreeMap<String, TreeNode>,
}

impl TreeNode {
    fn insert(&mut self, path: &str, oid: String) {
        let mut parts = path.split('/').peekable();
        let mut node = self;

        while let Some(part) = parts.next() {
            if parts.peek().is_none() {
                node.files.insert(part.to_string(), oid.clone());
            } else {
                node = node.directories.entry(part.to_string()).or_default();
            }
        }
    }
}

impl Repository {
    /// Creates a new Aura repository on disk and returns an initialized handle.
    pub async fn init(path: impl Into<PathBuf>) -> Result<Self> {
        let repository = Self::new(path.into());
        repository.init_repository().await?;
        Ok(repository)
    }

    /// Creates a new Aura repository using a custom filesystem implementation.
    pub async fn init_with_fs(path: impl Into<PathBuf>, fs: Arc<dyn FileSystem>) -> Result<Self> {
        let repository = Self::with_fs(path.into(), fs);
        repository.init_repository().await?;
        Ok(repository)
    }

    /// Initializes the repository metadata directory structure in-place.
    pub async fn init_repository(&self) -> Result<()> {
        self.fs.create_dir_all(&self.path).await?;
        refs::ensure_layout(self.fs.as_ref(), &self.path).await?;

        if !self.fs.exists(&refs::head_path(&self.path)).await? {
            refs::write_head_symbolic(self.fs.as_ref(), &self.path, "main").await?;
        }

        if !self.fs.exists(&index::index_path(&self.path)).await? {
            index::save_index(self.fs.as_ref(), &self.path, &IndexFile::default()).await?;
        }

        Ok(())
    }

    /// Adds files or directories to the index.
    ///
    /// Directories are traversed recursively. Aura currently stores regular file
    /// permissions as `100644` for every indexed file.
    pub async fn add<I, P>(&self, paths: I) -> Result<Vec<String>>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        self.ensure_initialized().await?;

        let mut index_file = index::load_index(self.fs.as_ref(), &self.path).await?;
        let mut targets = Vec::new();

        for path in paths {
            targets.push(self.resolve_input_path(path.as_ref())?);
        }

        let mut stack = targets;
        let mut written = Vec::new();

        while let Some(path) = stack.pop() {
            let metadata = self.fs.metadata(&path).await?;
            if metadata.is_dir {
                for child in self.fs.read_dir(&path).await? {
                    if is_aura_dir(&child) {
                        continue;
                    }
                    stack.push(child);
                }
                continue;
            }

            if !metadata.is_file {
                continue;
            }

            let relative = self.repo_relative(&path)?;
            let bytes = self.fs.read(&path).await?;
            let oid = object::write_blob(self.fs.as_ref(), &self.path, &Blob::new(bytes)).await?;
            let entry = index::entry_from_metadata(relative, FILE_MODE_INDEX, oid.clone(), &metadata);
            index_file.upsert(entry);
            written.push(oid);
        }

        index::save_index(self.fs.as_ref(), &self.path, &index_file).await?;
        Ok(written)
    }

    /// Adds the full working tree to the index and stages deletions.
    pub async fn add_all(&self) -> Result<Vec<String>> {
        self.ensure_initialized().await?;

        let mut stack = vec![self.path.clone()];
        let mut index_file = IndexFile::default();
        let mut written = Vec::new();

        while let Some(directory) = stack.pop() {
            for child in self.fs.read_dir(&directory).await? {
                if is_aura_dir(&child) {
                    continue;
                }

                let metadata = self.fs.metadata(&child).await?;
                if metadata.is_dir {
                    stack.push(child);
                    continue;
                }

                if !metadata.is_file {
                    continue;
                }

                let relative = self.repo_relative(&child)?;
                let bytes = self.fs.read(&child).await?;
                let oid = object::write_blob(self.fs.as_ref(), &self.path, &Blob::new(bytes)).await?;
                let entry = index::entry_from_metadata(relative, FILE_MODE_INDEX, oid.clone(), &metadata);
                index_file.upsert(entry);
                written.push(oid);
            }
        }

        index::save_index(self.fs.as_ref(), &self.path, &index_file).await?;
        Ok(written)
    }

    /// Creates a commit from the current index and updates the active reference.
    pub async fn commit(&self, message: impl Into<String>) -> Result<String> {
        Ok(self.commit_with_summary(message).await?.oid)
    }

    /// Creates a commit and returns a Git-like summary for CLI output.
    pub async fn commit_with_summary(&self, message: impl Into<String>) -> Result<CommitOutcome> {
        self.ensure_initialized().await?;

        let branch = refs::current_branch(self.fs.as_ref(), &self.path).await?;
        let index_file = index::load_index(self.fs.as_ref(), &self.path).await?;
        let tree_oid = self.write_tree_from_index(&index_file).await?;
        let timestamp = Utc::now().timestamp();
        let parent = refs::resolve_head(self.fs.as_ref(), &self.path).await?;
        let message = message.into();
        let commit = Commit {
            tree: tree_oid,
            parents: parent.into_iter().collect(),
            author: DEFAULT_AUTHOR.to_string(),
            committer: DEFAULT_AUTHOR.to_string(),
            timestamp,
            message,
        };
        let parent_tree = match commit.parents.first() {
            Some(parent_oid) => Some(object::read_commit(self.fs.as_ref(), &self.path, parent_oid).await?.tree),
            None => None,
        };
        let stats = self
            .diff_stats_between_trees(parent_tree.as_deref(), Some(commit.tree.as_str()))
            .await?;

        let oid = object::write_commit(self.fs.as_ref(), &self.path, &commit).await?;
        match refs::read_head(self.fs.as_ref(), &self.path).await? {
            HeadRef::Symbolic(reference) => {
                let branch = reference
                    .strip_prefix("refs/heads/")
                    .unwrap_or(reference.as_str());
                refs::write_branch(self.fs.as_ref(), &self.path, branch, &oid).await?;
            }
            HeadRef::Detached(_) => {
                refs::write_head_detached(self.fs.as_ref(), &self.path, &oid).await?;
            }
        }

        Ok(CommitOutcome {
            oid,
            branch,
            message: commit.message,
            stats,
        })
    }

    /// Returns the commit history by following the first parent chain from `HEAD`.
    pub async fn log(&self) -> Result<Vec<CommitSummary>> {
        self.ensure_initialized().await?;

        let mut current = refs::resolve_head(self.fs.as_ref(), &self.path).await?;
        let mut commits = Vec::new();

        while let Some(oid) = current {
            let commit = object::read_commit(self.fs.as_ref(), &self.path, &oid).await?;
            commits.push(CommitSummary {
                oid: oid.clone(),
                tree: commit.tree.clone(),
                parents: commit.parents.clone(),
                author: commit.author.clone(),
                message: commit.message.clone(),
                timestamp: to_utc(commit.timestamp),
            });
            current = commit.parents.first().cloned();
        }

        Ok(commits)
    }

    /// Creates a new branch at the current `HEAD` commit.
    pub async fn branch(&self, name: &str) -> Result<()> {
        self.ensure_initialized().await?;

        if refs::read_branch(self.fs.as_ref(), &self.path, name)
            .await?
            .is_some()
        {
            return Err(AuraError::BranchExists(name.to_string()));
        }

        let head = refs::resolve_head(self.fs.as_ref(), &self.path)
            .await?
            .ok_or_else(|| AuraError::ReferenceNotFound("HEAD".to_string()))?;
        refs::write_branch(self.fs.as_ref(), &self.path, name, &head).await
    }

    /// Configures a named remote that currently points to another local Aura repository.
    pub async fn add_remote(&self, name: &str, target: impl Into<PathBuf>) -> Result<PathBuf> {
        self.ensure_initialized().await?;

        let target = target.into();
        refs::write_remote(
            self.fs.as_ref(),
            &self.path,
            name,
            &target.to_string_lossy(),
        )
        .await?;
        Ok(target)
    }

    /// Pushes the requested branch to a named remote.
    pub async fn push(&self, remote: Option<&str>, branch: Option<&str>) -> Result<PushSummary> {
        self.ensure_initialized().await?;

        let remote_name = remote.unwrap_or("origin").to_string();
        let branch_name = match branch {
            Some(value) => value.to_string(),
            None => refs::current_branch(self.fs.as_ref(), &self.path)
                .await?
                .ok_or(AuraError::DetachedHead)?,
        };

        let oid = refs::read_branch(self.fs.as_ref(), &self.path, &branch_name)
            .await?
            .ok_or_else(|| AuraError::BranchNotFound(branch_name.clone()))?;
        let remote_target = refs::read_remote(self.fs.as_ref(), &self.path, &remote_name)
            .await?
            .ok_or_else(|| AuraError::RemoteNotFound(remote_name.clone()))?;
        let target = PathBuf::from(remote_target);

        let remote_repo = Repository::with_fs(target.clone(), self.fs.clone());
        if !self.fs.exists(&remote_repo.aura_dir()).await? {
            remote_repo.init_repository().await?;
        }

        let mut copied = BTreeSet::new();
        self.copy_commit_to(&remote_repo, &oid, &mut copied).await?;
        refs::write_branch(remote_repo.fs.as_ref(), &remote_repo.path, &branch_name, &oid).await?;

        Ok(PushSummary {
            remote: remote_name,
            branch: branch_name,
            oid,
            target,
        })
    }

    /// Pulls a remote branch into the current local branch using fast-forward semantics.
    pub async fn pull(&self, remote: Option<&str>, branch: Option<&str>) -> Result<PullSummary> {
        self.ensure_initialized().await?;

        let local_branch = refs::current_branch(self.fs.as_ref(), &self.path)
            .await?
            .ok_or(AuraError::DetachedHead)?;
        let remote_name = remote.unwrap_or("origin").to_string();
        let source_branch = branch.unwrap_or(local_branch.as_str()).to_string();
        let remote_target = refs::read_remote(self.fs.as_ref(), &self.path, &remote_name)
            .await?
            .ok_or_else(|| AuraError::RemoteNotFound(remote_name.clone()))?;
        let remote_path = PathBuf::from(remote_target);
        let remote_repo = Repository::with_fs(remote_path.clone(), self.fs.clone());
        if !self.fs.exists(&remote_repo.aura_dir()).await? {
            return Err(AuraError::NotRepository(remote_path.display().to_string()));
        }

        let remote_oid = refs::read_branch(remote_repo.fs.as_ref(), &remote_repo.path, &source_branch)
            .await?
            .ok_or_else(|| AuraError::RemoteBranchNotFound {
                remote: remote_name.clone(),
                branch: source_branch.clone(),
            })?;

        let mut copied = BTreeSet::new();
        remote_repo.copy_commit_to(self, &remote_oid, &mut copied).await?;
        refs::write_remote_branch(
            self.fs.as_ref(),
            &self.path,
            &remote_name,
            &source_branch,
            &remote_oid,
        )
        .await?;

        let previous_oid = refs::read_branch(self.fs.as_ref(), &self.path, &local_branch).await?;
        if previous_oid.as_deref() == Some(remote_oid.as_str()) {
            return Ok(PullSummary {
                remote: remote_name,
                source_branch,
                local_branch,
                oid: remote_oid,
                previous_oid,
                status: PullStatus::AlreadyUpToDate,
                stats: None,
                target: remote_path,
            });
        }

        if let Some(local_oid) = previous_oid.as_deref() {
            if self.is_ancestor(&remote_oid, local_oid).await? {
                return Ok(PullSummary {
                    remote: remote_name,
                    source_branch,
                    local_branch,
                    oid: local_oid.to_string(),
                    previous_oid,
                    status: PullStatus::AlreadyUpToDate,
                    stats: None,
                    target: remote_path,
                });
            }

            if !self.is_ancestor(local_oid, &remote_oid).await? {
                return Err(AuraError::DivergedBranches {
                    local: local_branch,
                    remote: format!("{remote_name}/{source_branch}"),
                });
            }
        }

        let current = match previous_oid.as_deref() {
            Some(oid) => {
                let commit = object::read_commit(self.fs.as_ref(), &self.path, oid).await?;
                self.collect_tree_entries(&commit.tree).await?
            }
            None => BTreeMap::new(),
        };
        let target_commit = object::read_commit(self.fs.as_ref(), &self.path, &remote_oid).await?;
        let target_map = self.collect_tree_entries(&target_commit.tree).await?;
        self.ensure_worktree_can_apply_target(&current, &target_map).await?;
        let stats = self
            .diff_stats_between_commits(previous_oid.as_deref(), Some(remote_oid.as_str()))
            .await?;
        self.apply_snapshot_to_worktree(&current, &target_map).await?;
        refs::write_branch(self.fs.as_ref(), &self.path, &local_branch, &remote_oid).await?;

        Ok(PullSummary {
            remote: remote_name,
            source_branch,
            local_branch,
            oid: remote_oid,
            previous_oid,
            status: PullStatus::FastForward,
            stats: Some(stats),
            target: remote_path,
        })
    }

    /// Switches the working tree to the requested branch.
    ///
    /// Aura checks out the branch tree into the worktree and rewrites the index
    /// to match the resulting files. Files tracked by the current index but not
    /// present in the target commit are removed; unrelated untracked files are
    /// left untouched.
    pub async fn checkout(&self, branch: &str) -> Result<()> {
        self.ensure_initialized().await?;

        let commit_oid = refs::read_branch(self.fs.as_ref(), &self.path, branch)
            .await?
            .ok_or_else(|| AuraError::BranchNotFound(branch.to_string()))?;
        let commit = object::read_commit(self.fs.as_ref(), &self.path, &commit_oid).await?;
        let target = self.collect_tree_entries(&commit.tree).await?;
        let current_head = refs::resolve_head(self.fs.as_ref(), &self.path).await?;
        let current = match current_head.as_deref() {
            Some(oid) => {
                let commit = object::read_commit(self.fs.as_ref(), &self.path, oid).await?;
                self.collect_tree_entries(&commit.tree).await?
            }
            None => BTreeMap::new(),
        };
        self.ensure_worktree_can_apply_target(&current, &target).await?;
        self.apply_snapshot_to_worktree(&current, &target).await?;
        refs::set_head_to_branch(self.fs.as_ref(), &self.path, branch).await
    }

    /// Forces the working tree and index to match the requested branch.
    ///
    /// This is intended for server-managed repositories and temporary transport
    /// clones where Aura owns the whole worktree and can safely rewrite it.
    pub async fn materialize_branch(&self, branch: &str) -> Result<()> {
        self.ensure_initialized().await?;

        let commit_oid = refs::read_branch(self.fs.as_ref(), &self.path, branch)
            .await?
            .ok_or_else(|| AuraError::BranchNotFound(branch.to_string()))?;
        let commit = object::read_commit(self.fs.as_ref(), &self.path, &commit_oid).await?;
        let target = self.collect_tree_entries(&commit.tree).await?;
        let current = self.collect_worktree_oids().await?;

        self.apply_snapshot_to_worktree(&current, &target).await?;
        refs::set_head_to_branch(self.fs.as_ref(), &self.path, branch).await
    }

    /// Returns repository status information.
    pub async fn status(&self) -> Result<RepositoryStatus> {
        self.ensure_initialized().await?;

        let branch = refs::current_branch(self.fs.as_ref(), &self.path).await?;
        let head = refs::resolve_head(self.fs.as_ref(), &self.path).await?;
        let index_file = index::load_index(self.fs.as_ref(), &self.path).await?;
        let head_map = match head.as_deref() {
            Some(oid) => {
                let commit = object::read_commit(self.fs.as_ref(), &self.path, oid).await?;
                self.collect_tree_entries(&commit.tree).await?
            }
            None => BTreeMap::new(),
        };
        let index_map = index_to_map(&index_file);
        let worktree_map = self.collect_worktree_oids().await?;

        let staged = compare_maps(&head_map, &index_map);
        let unstaged = compare_tracked_maps(&index_map, &worktree_map);
        let untracked = worktree_map
            .keys()
            .filter(|path| !index_map.contains_key(*path))
            .map(|path| repo_path_to_native(path))
            .collect();

        Ok(RepositoryStatus {
            branch,
            head,
            staged,
            unstaged,
            untracked,
        })
    }

    /// Produces a line-oriented diff between the index and the current worktree.
    ///
    /// The algorithm is Myers-based and operates on UTF-8 text lines. Binary
    /// files are decoded lossily with `String::from_utf8_lossy`.
    pub async fn diff(&self) -> Result<Vec<FileDiff>> {
        self.ensure_initialized().await?;

        let index_file = index::load_index(self.fs.as_ref(), &self.path).await?;
        let worktree = self.collect_worktree_oids().await?;

        let mut all_paths = BTreeSet::new();
        for entry in &index_file.entries {
            all_paths.insert(entry.path.clone());
        }
        for path in worktree.keys() {
            all_paths.insert(path.clone());
        }

        let mut diffs = Vec::new();
        for path in all_paths {
            let old_bytes = if let Some(entry) = index_file.get(&path) {
                object::read_blob(self.fs.as_ref(), &self.path, &entry.oid)
                    .await?
                    .content
            } else {
                Vec::new()
            };

            let absolute = self.path.join(repo_path_to_native(&path));
            let new_bytes = if self.fs.exists(&absolute).await? {
                self.fs.read(&absolute).await?
            } else {
                Vec::new()
            };

            if old_bytes == new_bytes {
                continue;
            }

            let old_text = String::from_utf8_lossy(&old_bytes).into_owned();
            let new_text = String::from_utf8_lossy(&new_bytes).into_owned();
            diffs.push(FileDiff {
                path: repo_path_to_native(&path),
                lines: myers_diff(&old_text, &new_text),
            });
        }

        Ok(diffs)
    }

    async fn ensure_initialized(&self) -> Result<()> {
        if !self.fs.exists(&self.aura_dir()).await? {
            return Err(AuraError::NotRepository(self.path.display().to_string()));
        }
        Ok(())
    }

    fn resolve_input_path(&self, input: &Path) -> Result<PathBuf> {
        let absolute = if input.is_absolute() {
            input.to_path_buf()
        } else {
            self.path.join(input)
        };

        let relative = absolute
            .strip_prefix(&self.path)
            .map_err(|_| AuraError::PathOutsideRepository(absolute.display().to_string()))?;
        if relative.components().next().is_some_and(|component| component.as_os_str() == ".aura") {
            return Err(AuraError::PathOutsideRepository(absolute.display().to_string()));
        }

        Ok(absolute)
    }

    fn repo_relative(&self, path: &Path) -> Result<String> {
        let relative = path
            .strip_prefix(&self.path)
            .map_err(|_| AuraError::PathOutsideRepository(path.display().to_string()))?;
        Ok(to_repo_path(relative))
    }

    async fn write_tree_from_index(&self, index_file: &IndexFile) -> Result<String> {
        let mut root = TreeNode::default();
        for entry in &index_file.entries {
            root.insert(&entry.path, entry.oid.clone());
        }
        self.write_tree_node(&root).await
    }

    #[async_recursion]
    async fn write_tree_node(&self, node: &TreeNode) -> Result<String> {
        let mut entries = Vec::new();

        for (name, oid) in &node.files {
            entries.push(TreeEntry {
                mode: FILE_MODE_TREE.to_string(),
                name: name.clone(),
                oid: oid.clone(),
            });
        }

        for (name, child) in &node.directories {
            let oid = self.write_tree_node(child).await?;
            entries.push(TreeEntry {
                mode: DIRECTORY_MODE_TREE.to_string(),
                name: name.clone(),
                oid,
            });
        }

        object::write_tree(self.fs.as_ref(), &self.path, &Tree { entries }).await
    }

    #[async_recursion]
    async fn collect_tree_entries(&self, tree_oid: &str) -> Result<BTreeMap<String, String>> {
        self.collect_tree_entries_with_prefix(tree_oid, Path::new("")).await
    }

    #[async_recursion]
    async fn collect_tree_entries_with_prefix(
        &self,
        tree_oid: &str,
        prefix: &Path,
    ) -> Result<BTreeMap<String, String>> {
        let tree = object::read_tree(self.fs.as_ref(), &self.path, tree_oid).await?;
        let mut entries = BTreeMap::new();

        for entry in tree.entries {
            let path = prefix.join(&entry.name);
            if entry.mode == DIRECTORY_MODE_TREE {
                let nested = self.collect_tree_entries_with_prefix(&entry.oid, &path).await?;
                entries.extend(nested);
            } else {
                entries.insert(to_repo_path(&path), entry.oid);
            }
        }

        Ok(entries)
    }

    #[async_recursion]
    async fn copy_commit_to(
        &self,
        destination: &Repository,
        oid: &str,
        copied: &mut BTreeSet<String>,
    ) -> Result<()> {
        if !copied.insert(oid.to_string()) {
            return Ok(());
        }

        let commit = object::read_commit(self.fs.as_ref(), &self.path, oid).await?;
        object::write_commit(destination.fs.as_ref(), &destination.path, &commit).await?;
        self.copy_tree_to(destination, &commit.tree, copied).await?;

        for parent in &commit.parents {
            self.copy_commit_to(destination, parent, copied).await?;
        }

        Ok(())
    }

    #[async_recursion]
    async fn copy_tree_to(
        &self,
        destination: &Repository,
        oid: &str,
        copied: &mut BTreeSet<String>,
    ) -> Result<()> {
        if !copied.insert(oid.to_string()) {
            return Ok(());
        }

        let tree = object::read_tree(self.fs.as_ref(), &self.path, oid).await?;
        object::write_tree(destination.fs.as_ref(), &destination.path, &tree).await?;

        for entry in tree.entries {
            if entry.mode == DIRECTORY_MODE_TREE {
                self.copy_tree_to(destination, &entry.oid, copied).await?;
            } else {
                self.copy_blob_to(destination, &entry.oid, copied).await?;
            }
        }

        Ok(())
    }

    async fn copy_blob_to(
        &self,
        destination: &Repository,
        oid: &str,
        copied: &mut BTreeSet<String>,
    ) -> Result<()> {
        if !copied.insert(oid.to_string()) {
            return Ok(());
        }

        let blob = object::read_blob(self.fs.as_ref(), &self.path, oid).await?;
        object::write_blob(destination.fs.as_ref(), &destination.path, &blob).await?;
        Ok(())
    }

    async fn collect_worktree_oids(&self) -> Result<BTreeMap<String, String>> {
        let mut stack = vec![self.path.clone()];
        let mut files = BTreeMap::new();

        while let Some(directory) = stack.pop() {
            for child in self.fs.read_dir(&directory).await? {
                if is_aura_dir(&child) {
                    continue;
                }

                let metadata = self.fs.metadata(&child).await?;
                if metadata.is_dir {
                    stack.push(child);
                    continue;
                }

                if metadata.is_file {
                    let relative = self.repo_relative(&child)?;
                    let bytes = self.fs.read(&child).await?;
                    let oid = object::hash_bytes(object::ObjectKind::Blob, &bytes);
                    files.insert(relative, oid);
                }
            }
        }

        Ok(files)
    }

    async fn diff_stats_between_commits(
        &self,
        base_commit: Option<&str>,
        target_commit: Option<&str>,
    ) -> Result<ChangeStats> {
        let base_tree = match base_commit {
            Some(oid) => Some(object::read_commit(self.fs.as_ref(), &self.path, oid).await?.tree),
            None => None,
        };
        let target_tree = match target_commit {
            Some(oid) => Some(object::read_commit(self.fs.as_ref(), &self.path, oid).await?.tree),
            None => None,
        };
        self.diff_stats_between_trees(base_tree.as_deref(), target_tree.as_deref())
            .await
    }

    async fn diff_stats_between_trees(
        &self,
        base_tree: Option<&str>,
        target_tree: Option<&str>,
    ) -> Result<ChangeStats> {
        let base = match base_tree {
            Some(oid) => self.collect_tree_entries(oid).await?,
            None => BTreeMap::new(),
        };
        let target = match target_tree {
            Some(oid) => self.collect_tree_entries(oid).await?,
            None => BTreeMap::new(),
        };
        self.diff_stats_between_maps(&base, &target).await
    }

    async fn diff_stats_between_maps(
        &self,
        base: &BTreeMap<String, String>,
        target: &BTreeMap<String, String>,
    ) -> Result<ChangeStats> {
        let mut paths = BTreeSet::new();
        paths.extend(base.keys().cloned());
        paths.extend(target.keys().cloned());

        let mut stats = ChangeStats::default();
        for path in paths {
            let kind = match (base.get(&path), target.get(&path)) {
                (Some(left), Some(right)) if left == right => continue,
                (None, Some(_)) => ChangeKind::Added,
                (Some(_), None) => ChangeKind::Deleted,
                (Some(_), Some(_)) => ChangeKind::Modified,
                (None, None) => continue,
            };
            let old_bytes = match base.get(&path) {
                Some(oid) => object::read_blob(self.fs.as_ref(), &self.path, oid).await?.content,
                None => Vec::new(),
            };
            let new_bytes = match target.get(&path) {
                Some(oid) => object::read_blob(self.fs.as_ref(), &self.path, oid).await?.content,
                None => Vec::new(),
            };
            let (insertions, deletions) = line_change_counts(&old_bytes, &new_bytes);
            stats.files_changed += 1;
            stats.insertions += insertions;
            stats.deletions += deletions;
            stats.files.push(FileChangeStat {
                path: repo_path_to_native(&path),
                kind,
                insertions,
                deletions,
            });
        }

        Ok(stats)
    }

    async fn ensure_worktree_can_apply_target(
        &self,
        current: &BTreeMap<String, String>,
        target: &BTreeMap<String, String>,
    ) -> Result<()> {
        let status = self.status().await?;
        if !status.staged.is_empty() || !status.unstaged.is_empty() {
            return Err(AuraError::WorkingTreeNotClean);
        }

        let worktree_map = self.collect_worktree_oids().await?;

        for path in target.keys() {
            if !current.contains_key(path) && worktree_map.contains_key(path) {
                return Err(AuraError::UntrackedWouldBeOverwritten(
                    repo_path_to_native(path).display().to_string(),
                ));
            }
        }

        Ok(())
    }

    async fn apply_snapshot_to_worktree(
        &self,
        current: &BTreeMap<String, String>,
        target: &BTreeMap<String, String>,
    ) -> Result<()> {
        for path in current.keys() {
            if target.contains_key(path) {
                continue;
            }
            let absolute = self.path.join(repo_path_to_native(path));
            self.fs.remove_file(&absolute).await?;
        }

        let mut new_index = IndexFile::default();
        for (path, oid) in target {
            let absolute = self.path.join(repo_path_to_native(path));
            if let Some(parent) = absolute.parent() {
                self.fs.create_dir_all(parent).await?;
            }

            let blob = object::read_blob(self.fs.as_ref(), &self.path, oid).await?;
            self.fs.write(&absolute, &blob.content).await?;
            let metadata = self.fs.metadata(&absolute).await?;
            let entry = index::entry_from_metadata(path.clone(), FILE_MODE_INDEX, oid.clone(), &metadata);
            new_index.upsert(entry);
        }

        index::save_index(self.fs.as_ref(), &self.path, &new_index).await
    }

    #[async_recursion]
    async fn is_ancestor(&self, ancestor: &str, descendant: &str) -> Result<bool> {
        if ancestor == descendant {
            return Ok(true);
        }

        let commit = object::read_commit(self.fs.as_ref(), &self.path, descendant).await?;
        for parent in &commit.parents {
            if self.is_ancestor(ancestor, parent).await? {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

fn index_to_map(index: &IndexFile) -> BTreeMap<String, String> {
    index
        .entries
        .iter()
        .map(|entry| (entry.path.clone(), entry.oid.clone()))
        .collect()
}

fn compare_maps(base: &BTreeMap<String, String>, target: &BTreeMap<String, String>) -> Vec<StatusEntry> {
    let mut paths = BTreeSet::new();
    paths.extend(base.keys().cloned());
    paths.extend(target.keys().cloned());

    let mut result = Vec::new();
    for path in paths {
        match (base.get(&path), target.get(&path)) {
            (None, Some(_)) => result.push(StatusEntry {
                path: repo_path_to_native(&path),
                kind: ChangeKind::Added,
            }),
            (Some(_), None) => result.push(StatusEntry {
                path: repo_path_to_native(&path),
                kind: ChangeKind::Deleted,
            }),
            (Some(left), Some(right)) if left != right => result.push(StatusEntry {
                path: repo_path_to_native(&path),
                kind: ChangeKind::Modified,
            }),
            _ => {}
        }
    }
    result
}

fn compare_tracked_maps(
    base: &BTreeMap<String, String>,
    target: &BTreeMap<String, String>,
) -> Vec<StatusEntry> {
    let mut result = Vec::new();
    for path in base.keys() {
        match (base.get(path), target.get(path)) {
            (Some(_), None) => result.push(StatusEntry {
                path: repo_path_to_native(path),
                kind: ChangeKind::Deleted,
            }),
            (Some(left), Some(right)) if left != right => result.push(StatusEntry {
                path: repo_path_to_native(path),
                kind: ChangeKind::Modified,
            }),
            _ => {}
        }
    }
    result
}

fn to_repo_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn repo_path_to_native(path: &str) -> PathBuf {
    path.split('/')
        .filter(|part| !part.is_empty())
        .collect::<PathBuf>()
}

fn is_aura_dir(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name == crate::AURA_DIR)
}

fn to_utc(seconds: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(seconds, 0)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).expect("unix epoch is valid"))
}

fn split_lines(input: &str) -> Vec<String> {
    if input.is_empty() {
        return Vec::new();
    }

    input
        .split_inclusive('\n')
        .map(|line| line.to_string())
        .collect()
}

fn line_change_counts(old_bytes: &[u8], new_bytes: &[u8]) -> (usize, usize) {
    let old_text = String::from_utf8_lossy(old_bytes).into_owned();
    let new_text = String::from_utf8_lossy(new_bytes).into_owned();
    let mut insertions = 0usize;
    let mut deletions = 0usize;

    for line in myers_diff(&old_text, &new_text) {
        match line {
            DiffLine::Addition(_) => insertions += 1,
            DiffLine::Deletion(_) => deletions += 1,
            DiffLine::Context(_) => {}
        }
    }

    (insertions, deletions)
}

fn myers_diff(old: &str, new: &str) -> Vec<DiffLine> {
    let a = split_lines(old);
    let b = split_lines(new);
    let n = a.len() as isize;
    let m = b.len() as isize;
    if n == 0 && m == 0 {
        return Vec::new();
    }
    let max = (n + m) as usize;
    let offset = max as isize;
    let mut v = vec![0isize; 2 * max + 1];
    let mut trace = Vec::new();

    'search: for d in 0..=max {
        trace.push(v.clone());
        let d_isize = d as isize;
        let mut k = -d_isize;
        while k <= d_isize {
            let index = (k + offset) as usize;
            let x = if k == -d_isize
                || (k != d_isize && v[(k - 1 + offset) as usize] < v[(k + 1 + offset) as usize])
            {
                v[(k + 1 + offset) as usize]
            } else {
                v[(k - 1 + offset) as usize] + 1
            };
            let mut y = x - k;
            let mut x2 = x;

            while x2 < n && y < m && a[x2 as usize] == b[y as usize] {
                x2 += 1;
                y += 1;
            }

            v[index] = x2;
            if x2 >= n && y >= m {
                trace.push(v.clone());
                break 'search;
            }
            k += 2;
        }
    }

    let mut x = n;
    let mut y = m;
    let mut result = Vec::new();

    for d in (1..trace.len()).rev() {
        let v = &trace[d - 1];
        let d_isize = (d - 1) as isize;
        let k = x - y;

        let prev_k = if k == -d_isize
            || (k != d_isize && v[(k - 1 + offset) as usize] < v[(k + 1 + offset) as usize])
        {
            k + 1
        } else {
            k - 1
        };

        let prev_x = v[(prev_k + offset) as usize];
        let prev_y = prev_x - prev_k;

        while x > prev_x && y > prev_y {
            result.push(DiffLine::Context(a[(x - 1) as usize].clone()));
            x -= 1;
            y -= 1;
        }

        if d > 1 {
            if x == prev_x && y > 0 {
                result.push(DiffLine::Addition(b[(y - 1) as usize].clone()));
                y -= 1;
            } else if x > 0 {
                result.push(DiffLine::Deletion(a[(x - 1) as usize].clone()));
                x -= 1;
            }
        }
    }

    while x > 0 && y > 0 {
        result.push(DiffLine::Context(a[(x - 1) as usize].clone()));
        x -= 1;
        y -= 1;
    }
    while x > 0 {
        result.push(DiffLine::Deletion(a[(x - 1) as usize].clone()));
        x -= 1;
    }
    while y > 0 {
        result.push(DiffLine::Addition(b[(y - 1) as usize].clone()));
        y -= 1;
    }

    result.reverse();
    result
}

#[cfg(test)]
mod tests {
    use super::{PullStatus, Repository};
    use crate::refs;

    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_repo_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("aura-{name}-{unique}"))
    }

    async fn write_file(path: &std::path::Path, contents: &[u8]) {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .expect("parent directory should be created");
        }
        tokio::fs::write(path, contents)
            .await
            .expect("file should be written");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn commit_summary_reports_line_stats() {
        let root = temp_repo_path("commit-summary");
        let repo = Repository::init(&root).await.expect("repo should init");

        let file = root.join("note.txt");
        write_file(&file, b"one\n").await;
        repo.add(["note.txt"]).await.expect("file should be staged");
        repo.commit("initial commit")
            .await
            .expect("initial commit should succeed");

        write_file(&file, b"one\nthree\n").await;
        repo.add(["note.txt"]).await.expect("updated file should be staged");
        let summary = repo
            .commit_with_summary("expand note")
            .await
            .expect("commit with summary should succeed");

        assert_eq!(summary.branch.as_deref(), Some("main"));
        assert_eq!(summary.stats.files_changed, 1);
        assert_eq!(summary.stats.insertions, 1);
        assert_eq!(summary.stats.deletions, 0);
        assert_eq!(summary.stats.files.len(), 1);
        assert_eq!(summary.stats.files[0].insertions, 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn pull_fast_forwards_current_branch() {
        let remote_root = temp_repo_path("pull-remote");
        let local_root = temp_repo_path("pull-local");
        let remote = Repository::init(&remote_root)
            .await
            .expect("remote repo should init");
        let local = Repository::init(&local_root)
            .await
            .expect("local repo should init");

        write_file(&remote_root.join("hello.txt"), b"from remote\n").await;
        remote
            .add(["hello.txt"])
            .await
            .expect("remote file should be staged");
        let remote_commit = remote
            .commit("remote commit")
            .await
            .expect("remote commit should succeed");

        local
            .add_remote("origin", &remote_root)
            .await
            .expect("remote should be configured");
        let summary = local.pull(None, None).await.expect("pull should succeed");

        assert_eq!(summary.status, PullStatus::FastForward);
        assert_eq!(summary.oid, remote_commit);
        assert_eq!(summary.local_branch, "main");
        assert_eq!(
            tokio::fs::read_to_string(local_root.join("hello.txt"))
                .await
                .expect("local file should exist"),
            "from remote\n"
        );
        assert_eq!(
            refs::read_branch(local.filesystem(), &local.path, "main")
                .await
                .expect("local main should be readable")
                .as_deref(),
            Some(remote_commit.as_str())
        );
        assert_eq!(summary.stats.expect("fast-forward should include stats").insertions, 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn add_remote_updates_existing_target() {
        let root = temp_repo_path("remote-update-local");
        let first_remote = temp_repo_path("remote-update-first");
        let second_remote = temp_repo_path("remote-update-second");
        let repo = Repository::init(&root).await.expect("repo should init");

        repo.add_remote("origin", &first_remote)
            .await
            .expect("initial remote should be added");
        let updated_target = repo
            .add_remote("origin", &second_remote)
            .await
            .expect("existing remote should be updated");

        assert_eq!(updated_target, second_remote);
        assert_eq!(
            refs::read_remote(repo.filesystem(), &repo.path, "origin")
                .await
                .expect("remote should be readable")
                .as_deref(),
            Some(second_remote.to_string_lossy().as_ref())
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn push_copies_commit_chain_to_remote_branch() {
        let local_root = temp_repo_path("push-local");
        let remote_root = temp_repo_path("push-remote");
        let local = Repository::init(&local_root)
            .await
            .expect("local repo should init");

        write_file(&local_root.join("hello.txt"), b"hello\n").await;
        local.add_all().await.expect("initial file should be staged");
        let first_commit = local
            .commit("initial commit")
            .await
            .expect("initial commit should succeed");

        write_file(&local_root.join("hello.txt"), b"hello\nsecond line\n").await;
        local.add_all().await.expect("updated file should be staged");
        let second_commit = local
            .commit("second commit")
            .await
            .expect("second commit should succeed");

        local
            .add_remote("origin", &remote_root)
            .await
            .expect("remote should be configured");
        let summary = local.push(None, None).await.expect("push should succeed");

        let remote = Repository::new(&remote_root);
        let log = remote.log().await.expect("remote log should be readable");

        assert_eq!(summary.remote, "origin");
        assert_eq!(summary.branch, "main");
        assert_eq!(summary.oid, second_commit);
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].oid, second_commit);
        assert_eq!(log[1].oid, first_commit);
        assert_eq!(
            refs::read_branch(remote.filesystem(), &remote.path, "main")
                .await
                .expect("remote main should be readable")
                .as_deref(),
            Some(second_commit.as_str())
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn pull_updates_remote_tracking_branch_reference() {
        let remote_root = temp_repo_path("pull-tracking-remote");
        let local_root = temp_repo_path("pull-tracking-local");
        let remote = Repository::init(&remote_root)
            .await
            .expect("remote repo should init");
        let local = Repository::init(&local_root)
            .await
            .expect("local repo should init");

        write_file(&remote_root.join("tracked.txt"), b"tracked from remote\n").await;
        remote
            .add_all()
            .await
            .expect("remote file should be staged");
        let remote_commit = remote
            .commit("tracked commit")
            .await
            .expect("remote commit should succeed");

        local
            .add_remote("origin", &remote_root)
            .await
            .expect("remote should be configured");
        local.pull(None, None).await.expect("pull should succeed");

        assert_eq!(
            refs::read_remote_branch(local.filesystem(), &local.path, "origin", "main")
                .await
                .expect("remote tracking branch should be readable")
                .as_deref(),
            Some(remote_commit.as_str())
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn checkout_restores_target_branch_snapshot() {
        let root = temp_repo_path("checkout-snapshot");
        let repo = Repository::init(&root).await.expect("repo should init");

        write_file(&root.join("main.c"), b"main\n").await;
        write_file(&root.join("lib.c"), b"lib\n").await;
        repo.add_all().await.expect("base files should be staged");
        repo.commit("base commit")
            .await
            .expect("base commit should succeed");
        repo.branch("test").await.expect("test branch should be created");

        repo.checkout("test")
            .await
            .expect("checkout test should succeed");
        write_file(&root.join("test.c"), b"test-only\n").await;
        repo.add_all()
            .await
            .expect("branch-only file should be staged");
        repo.commit("branch commit")
            .await
            .expect("branch commit should succeed");
        assert!(
            tokio::fs::try_exists(root.join("test.c"))
                .await
                .expect("existence check should succeed")
        );

        repo.checkout("main")
            .await
            .expect("checkout main should succeed");
        assert!(
            !tokio::fs::try_exists(root.join("test.c"))
                .await
                .expect("existence check should succeed")
        );
        assert_eq!(
            refs::current_branch(repo.filesystem(), &repo.path)
                .await
                .expect("current branch should be readable")
                .as_deref(),
            Some("main")
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn checkout_rejects_conflicting_untracked_file() {
        let root = temp_repo_path("checkout-untracked");
        let repo = Repository::init(&root).await.expect("repo should init");

        write_file(&root.join("base.txt"), b"base\n").await;
        repo.add_all().await.expect("base file should be staged");
        repo.commit("base commit")
            .await
            .expect("base commit should succeed");
        repo.branch("test").await.expect("test branch should be created");

        repo.checkout("test")
            .await
            .expect("checkout test should succeed");
        write_file(&root.join("test.c"), b"tracked on test\n").await;
        repo.add_all()
            .await
            .expect("test branch file should be staged");
        repo.commit("branch commit")
            .await
            .expect("branch commit should succeed");
        repo.checkout("main")
            .await
            .expect("checkout main should succeed");

        write_file(&root.join("test.c"), b"local untracked\n").await;
        let error = repo
            .checkout("test")
            .await
            .expect_err("checkout should reject conflicting untracked file");
        assert!(matches!(error, crate::AuraError::UntrackedWouldBeOverwritten(_)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn status_keeps_untracked_files_out_of_unstaged_changes() {
        let root = temp_repo_path("status-untracked");
        let repo = Repository::init(&root).await.expect("repo should init");

        write_file(&root.join("base.txt"), b"base\n").await;
        repo.add_all().await.expect("base file should be staged");
        repo.commit("base commit")
            .await
            .expect("base commit should succeed");

        write_file(&root.join("new.txt"), b"untracked\n").await;
        let status = repo.status().await.expect("status should succeed");

        assert!(status.unstaged.is_empty());
        assert_eq!(status.untracked.len(), 1);
        assert_eq!(status.untracked[0], PathBuf::from("new.txt"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn commit_with_empty_file_does_not_panic() {
        let root = temp_repo_path("commit-empty-file");
        let repo = Repository::init(&root).await.expect("repo should init");

        write_file(&root.join("empty.txt"), b"").await;
        repo.add_all().await.expect("empty file should be staged");
        let summary = repo
            .commit_with_summary("empty file commit")
            .await
            .expect("commit should succeed");

        assert_eq!(summary.stats.files_changed, 1);
        assert_eq!(summary.stats.insertions, 0);
        assert_eq!(summary.stats.deletions, 0);
        assert_eq!(summary.stats.files.len(), 1);
    }
}
