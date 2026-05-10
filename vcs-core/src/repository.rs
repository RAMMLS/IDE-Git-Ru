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
        self.ensure_initialized().await?;

        let index_file = index::load_index(self.fs.as_ref(), &self.path).await?;
        let tree_oid = self.write_tree_from_index(&index_file).await?;
        let timestamp = Utc::now().timestamp();
        let parent = refs::resolve_head(self.fs.as_ref(), &self.path).await?;
        let commit = Commit {
            tree: tree_oid,
            parents: parent.into_iter().collect(),
            author: DEFAULT_AUTHOR.to_string(),
            committer: DEFAULT_AUTHOR.to_string(),
            timestamp,
            message: message.into(),
        };

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

        Ok(oid)
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
        let current_index = index::load_index(self.fs.as_ref(), &self.path).await?;

        for entry in &current_index.entries {
            if target.contains_key(&entry.path) {
                continue;
            }
            let path = self.path.join(repo_path_to_native(&entry.path));
            self.fs.remove_file(&path).await?;
        }

        let mut new_index = IndexFile::default();
        for (path, oid) in target {
            let absolute = self.path.join(repo_path_to_native(&path));
            if let Some(parent) = absolute.parent() {
                self.fs.create_dir_all(parent).await?;
            }

            let blob = object::read_blob(self.fs.as_ref(), &self.path, &oid).await?;
            self.fs.write(&absolute, &blob.content).await?;
            let metadata = self.fs.metadata(&absolute).await?;
            let entry = index::entry_from_metadata(path, FILE_MODE_INDEX, oid, &metadata);
            new_index.upsert(entry);
        }

        index::save_index(self.fs.as_ref(), &self.path, &new_index).await?;
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
        let unstaged = compare_maps(&index_map, &worktree_map);
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

fn myers_diff(old: &str, new: &str) -> Vec<DiffLine> {
    let a = split_lines(old);
    let b = split_lines(new);
    let n = a.len() as isize;
    let m = b.len() as isize;
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
