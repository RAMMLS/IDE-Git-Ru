//! Reference management for Aura repositories.
//!
//! Branches are stored under `.aura/refs/heads/<name>`. `HEAD` usually contains
//! `ref: refs/heads/<name>`, but detached `HEAD` mode is also supported for
//! explicit commit checkout scenarios.

use std::path::{Path, PathBuf};

use crate::{AuraError, FileSystem, Result, AURA_DIR};

/// Parsed representation of the `.aura/HEAD` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadRef {
    /// Symbolic reference such as `refs/heads/main`.
    Symbolic(String),
    /// Detached head pointing directly to a commit id.
    Detached(String),
}

/// Returns the path to `.aura/HEAD`.
pub fn head_path(repo_path: &Path) -> PathBuf {
    repo_path.join(AURA_DIR).join("HEAD")
}

/// Returns the path to `.aura/refs/heads`.
pub fn heads_dir(repo_path: &Path) -> PathBuf {
    repo_path.join(AURA_DIR).join("refs").join("heads")
}

/// Returns the path to `.aura/remotes`.
pub fn remotes_dir(repo_path: &Path) -> PathBuf {
    repo_path.join(AURA_DIR).join("remotes")
}

/// Returns the full path of a branch reference file.
pub fn branch_path(repo_path: &Path, branch: &str) -> PathBuf {
    heads_dir(repo_path).join(branch)
}

/// Returns the full path of a remote definition file.
pub fn remote_path(repo_path: &Path, remote: &str) -> PathBuf {
    remotes_dir(repo_path).join(remote)
}

/// Creates the reference directory layout used by Aura.
pub async fn ensure_layout(fs: &dyn FileSystem, repo_path: &Path) -> Result<()> {
    fs.create_dir_all(&repo_path.join(AURA_DIR).join("objects")).await?;
    fs.create_dir_all(&heads_dir(repo_path)).await?;
    fs.create_dir_all(&remotes_dir(repo_path)).await?;
    Ok(())
}

/// Reads and parses the `HEAD` file.
pub async fn read_head(fs: &dyn FileSystem, repo_path: &Path) -> Result<HeadRef> {
    let path = head_path(repo_path);
    if !fs.exists(&path).await? {
        return Err(AuraError::InvalidHead);
    }

    let raw = fs.read_to_string(&path).await?;
    let trimmed = raw.trim();
    if let Some(reference) = trimmed.strip_prefix("ref: ") {
        return Ok(HeadRef::Symbolic(reference.to_string()));
    }

    if trimmed.len() == 40 {
        return Ok(HeadRef::Detached(trimmed.to_string()));
    }

    Err(AuraError::InvalidHead)
}

/// Writes `HEAD` as a symbolic branch reference.
pub async fn write_head_symbolic(fs: &dyn FileSystem, repo_path: &Path, branch: &str) -> Result<()> {
    let reference = normalize_branch_ref(branch);
    fs.write(&head_path(repo_path), format!("ref: {reference}\n").as_bytes())
        .await
}

/// Writes `HEAD` in detached mode.
pub async fn write_head_detached(fs: &dyn FileSystem, repo_path: &Path, oid: &str) -> Result<()> {
    fs.write(&head_path(repo_path), format!("{oid}\n").as_bytes())
        .await
}

/// Returns the current branch name when `HEAD` is symbolic.
pub async fn current_branch(fs: &dyn FileSystem, repo_path: &Path) -> Result<Option<String>> {
    match read_head(fs, repo_path).await? {
        HeadRef::Symbolic(reference) => Ok(reference
            .strip_prefix("refs/heads/")
            .map(|branch| branch.to_string())),
        HeadRef::Detached(_) => Ok(None),
    }
}

/// Resolves `HEAD` to a commit id.
pub async fn resolve_head(fs: &dyn FileSystem, repo_path: &Path) -> Result<Option<String>> {
    match read_head(fs, repo_path).await? {
        HeadRef::Symbolic(reference) => resolve_reference(fs, repo_path, &reference).await,
        HeadRef::Detached(oid) => Ok(Some(oid)),
    }
}

/// Reads a branch file and returns its commit id.
pub async fn read_branch(
    fs: &dyn FileSystem,
    repo_path: &Path,
    branch: &str,
) -> Result<Option<String>> {
    let path = branch_path(repo_path, branch);
    if !fs.exists(&path).await? {
        return Ok(None);
    }

    let oid = fs.read_to_string(&path).await?;
    Ok(Some(oid.trim().to_string()))
}

/// Writes a branch reference.
pub async fn write_branch(
    fs: &dyn FileSystem,
    repo_path: &Path,
    branch: &str,
    oid: &str,
) -> Result<()> {
    let path = branch_path(repo_path, branch);
    if let Some(parent) = path.parent() {
        fs.create_dir_all(parent).await?;
    }
    fs.write(&path, format!("{oid}\n").as_bytes()).await
}

/// Reads a configured remote path.
pub async fn read_remote(
    fs: &dyn FileSystem,
    repo_path: &Path,
    remote: &str,
) -> Result<Option<String>> {
    let path = remote_path(repo_path, remote);
    if !fs.exists(&path).await? {
        return Ok(None);
    }

    let target = fs.read_to_string(&path).await?;
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    Ok(Some(trimmed.to_string()))
}

/// Writes a remote configuration entry.
pub async fn write_remote(
    fs: &dyn FileSystem,
    repo_path: &Path,
    remote: &str,
    target: &str,
) -> Result<()> {
    let path = remote_path(repo_path, remote);
    if let Some(parent) = path.parent() {
        fs.create_dir_all(parent).await?;
    }
    fs.write(&path, format!("{target}\n").as_bytes()).await
}

/// Resolves a reference such as `refs/heads/main` to a commit id.
pub async fn resolve_reference(
    fs: &dyn FileSystem,
    repo_path: &Path,
    reference: &str,
) -> Result<Option<String>> {
    let path = repo_path.join(AURA_DIR).join(reference);
    if !fs.exists(&path).await? {
        return Ok(None);
    }

    let raw = fs.read_to_string(&path).await?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(trimmed.to_string()))
}

/// Lists all branches under `.aura/refs/heads`.
pub async fn list_branches(fs: &dyn FileSystem, repo_path: &Path) -> Result<Vec<String>> {
    let root = heads_dir(repo_path);
    if !fs.exists(&root).await? {
        return Ok(Vec::new());
    }

    let mut stack = vec![root.clone()];
    let mut branches = Vec::new();

    while let Some(dir) = stack.pop() {
        for child in fs.read_dir(&dir).await? {
            let metadata = fs.metadata(&child).await?;
            if metadata.is_dir {
                stack.push(child);
                continue;
            }
            if metadata.is_file {
                let relative = child
                    .strip_prefix(&root)
                    .map_err(|_| AuraError::InvalidReference(child.display().to_string()))?;
                branches.push(to_repo_relative(relative));
            }
        }
    }

    branches.sort();
    Ok(branches)
}

/// Updates `HEAD` to point to the provided branch.
pub async fn set_head_to_branch(
    fs: &dyn FileSystem,
    repo_path: &Path,
    branch: &str,
) -> Result<()> {
    write_head_symbolic(fs, repo_path, branch).await
}

fn normalize_branch_ref(branch: &str) -> String {
    if branch.starts_with("refs/heads/") {
        branch.to_string()
    } else {
        format!("refs/heads/{branch}")
    }
}

fn to_repo_relative(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}
