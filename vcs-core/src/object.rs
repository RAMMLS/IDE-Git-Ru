//! Aura object model and object database helpers.
//!
//! Aura stores three object kinds: blob, tree and commit. Every object is
//! serialized as `"<type> <len>\0<body>"`, hashed with SHA-1 and then compressed
//! with zlib before it is written under `.aura/objects/`.

use std::fmt::{Display, Formatter};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::{AuraError, FileSystem, Result, AURA_DIR};

/// Supported Aura object kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    /// Raw file contents.
    Blob,
    /// Directory listing that references blobs or nested trees.
    Tree,
    /// Commit metadata pointing to a root tree and zero or more parents.
    Commit,
}

impl ObjectKind {
    /// Returns the textual header name used by the object storage format.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Blob => "blob",
            Self::Tree => "tree",
            Self::Commit => "commit",
        }
    }

    /// Parses an object kind from its serialized header name.
    pub fn from_str(value: &str) -> Result<Self> {
        match value {
            "blob" => Ok(Self::Blob),
            "tree" => Ok(Self::Tree),
            "commit" => Ok(Self::Commit),
            other => Err(AuraError::InvalidReference(other.to_string())),
        }
    }
}

impl Display for ObjectKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// In-memory representation of a blob object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blob {
    /// Raw file bytes.
    pub content: Vec<u8>,
}

impl Blob {
    /// Creates a blob from raw bytes.
    pub fn new(content: Vec<u8>) -> Self {
        Self { content }
    }

    /// Returns the object body used for hashing and storage.
    pub fn serialize(&self) -> Vec<u8> {
        self.content.clone()
    }

    /// Builds a blob from its stored body.
    pub fn deserialize(raw: &[u8]) -> Self {
        Self::new(raw.to_vec())
    }
}

/// A single tree entry.
///
/// Aura uses Git-compatible directory encoding. File modes are simplified:
/// regular files are written as `100644`, directories as `40000`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeEntry {
    /// Mode written to the tree object, usually `100644` or `40000`.
    pub mode: String,
    /// Entry basename relative to the containing tree.
    pub name: String,
    /// SHA-1 of the referenced blob or subtree.
    pub oid: String,
}

/// Directory object stored in the Aura object database.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tree {
    /// Entries contained in the directory.
    pub entries: Vec<TreeEntry>,
}

impl Tree {
    /// Serializes the tree into Git-compatible binary tree format.
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut entries = self.entries.clone();
        entries.sort_by(|left, right| left.name.cmp(&right.name));

        let mut raw = Vec::new();
        for entry in entries {
            raw.extend_from_slice(entry.mode.as_bytes());
            raw.push(b' ');
            raw.extend_from_slice(entry.name.as_bytes());
            raw.push(0);
            raw.extend_from_slice(&decode_oid(&entry.oid)?);
        }
        Ok(raw)
    }

    /// Parses a binary tree body into a [`Tree`].
    pub fn deserialize(raw: &[u8]) -> Result<Self> {
        let mut cursor = 0usize;
        let mut entries = Vec::new();

        while cursor < raw.len() {
            let mode_end = raw[cursor..]
                .iter()
                .position(|byte| *byte == b' ')
                .ok_or_else(|| AuraError::CorruptObject("tree".to_string()))?;
            let mode = String::from_utf8(raw[cursor..cursor + mode_end].to_vec())?;
            cursor += mode_end + 1;

            let name_end = raw[cursor..]
                .iter()
                .position(|byte| *byte == 0)
                .ok_or_else(|| AuraError::CorruptObject("tree".to_string()))?;
            let name = String::from_utf8(raw[cursor..cursor + name_end].to_vec())?;
            cursor += name_end + 1;

            if cursor + 20 > raw.len() {
                return Err(AuraError::CorruptObject("tree".to_string()));
            }

            let oid = hex::encode(&raw[cursor..cursor + 20]);
            cursor += 20;

            entries.push(TreeEntry { mode, name, oid });
        }

        Ok(Self { entries })
    }
}

/// Commit object stored in Aura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    /// Root tree hash.
    pub tree: String,
    /// Parent commit hashes. Aura follows the first parent in `log()`.
    pub parents: Vec<String>,
    /// Commit author, fixed by the project requirements.
    pub author: String,
    /// Commit committer, also fixed by the project requirements.
    pub committer: String,
    /// Commit timestamp in UTC seconds since the Unix epoch.
    pub timestamp: i64,
    /// Commit message body.
    pub message: String,
}

impl Commit {
    /// Serializes the commit into a Git-like textual commit body.
    pub fn serialize(&self) -> Vec<u8> {
        let mut text = String::new();
        text.push_str(&format!("tree {}\n", self.tree));
        for parent in &self.parents {
            text.push_str(&format!("parent {}\n", parent));
        }
        text.push_str(&format!(
            "author {} {} +0000\n",
            self.author, self.timestamp
        ));
        text.push_str(&format!(
            "committer {} {} +0000\n",
            self.committer, self.timestamp
        ));
        text.push('\n');
        text.push_str(&self.message);
        text.into_bytes()
    }

    /// Parses a commit body from the object database.
    pub fn deserialize(raw: &[u8]) -> Result<Self> {
        let text = String::from_utf8(raw.to_vec())?;
        let mut parts = text.splitn(2, "\n\n");
        let headers = parts
            .next()
            .ok_or_else(|| AuraError::CorruptObject("commit".to_string()))?;
        let message = parts.next().unwrap_or_default().to_string();

        let mut tree = None::<String>;
        let mut parents = Vec::new();
        let mut author = None::<String>;
        let mut committer = None::<String>;
        let mut timestamp = None::<i64>;

        for line in headers.lines() {
            if let Some(value) = line.strip_prefix("tree ") {
                tree = Some(value.to_string());
                continue;
            }
            if let Some(value) = line.strip_prefix("parent ") {
                parents.push(value.to_string());
                continue;
            }
            if let Some(value) = line.strip_prefix("author ") {
                let (name, time) = parse_signature(value)?;
                author = Some(name);
                timestamp = Some(time);
                continue;
            }
            if let Some(value) = line.strip_prefix("committer ") {
                let (name, _) = parse_signature(value)?;
                committer = Some(name);
            }
        }

        Ok(Self {
            tree: tree.ok_or_else(|| AuraError::CorruptObject("commit".to_string()))?,
            parents,
            author: author.ok_or_else(|| AuraError::CorruptObject("commit".to_string()))?,
            committer: committer.ok_or_else(|| AuraError::CorruptObject("commit".to_string()))?,
            timestamp: timestamp.ok_or_else(|| AuraError::CorruptObject("commit".to_string()))?,
            message,
        })
    }
}

/// Raw object read from `.aura/objects`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredObject {
    /// Parsed object kind.
    pub kind: ObjectKind,
    /// Uncompressed object body.
    pub body: Vec<u8>,
}

/// Returns the object header and body concatenated as stored for hashing.
pub fn serialize_loose_object(kind: ObjectKind, body: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(kind.as_str().len() + body.len() + 32);
    raw.extend_from_slice(kind.as_str().as_bytes());
    raw.push(b' ');
    raw.extend_from_slice(body.len().to_string().as_bytes());
    raw.push(0);
    raw.extend_from_slice(body);
    raw
}

/// Computes the Aura object id for the provided body.
pub fn hash_bytes(kind: ObjectKind, body: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(serialize_loose_object(kind, body));
    hex::encode(hasher.finalize())
}

/// Resolves the loose-object path for a given object id.
pub fn object_path(repo_path: &Path, oid: &str) -> Result<PathBuf> {
    if oid.len() != 40 {
        return Err(AuraError::ObjectNotFound(oid.to_string()));
    }

    let (dir, file) = oid.split_at(2);
    Ok(repo_path
        .join(AURA_DIR)
        .join("objects")
        .join(dir)
        .join(file))
}

/// Writes a raw object body to the object database and returns its SHA-1 id.
pub async fn write_object(
    fs: &dyn FileSystem,
    repo_path: &Path,
    kind: ObjectKind,
    body: &[u8],
) -> Result<String> {
    let oid = hash_bytes(kind, body);
    let path = object_path(repo_path, &oid)?;

    if fs.exists(&path).await? {
        return Ok(oid);
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&serialize_loose_object(kind, body))?;
    let compressed = encoder.finish()?;

    if let Some(parent) = path.parent() {
        fs.create_dir_all(parent).await?;
    }
    fs.write(&path, &compressed).await?;
    Ok(oid)
}

/// Reads and decompresses an object from the database.
pub async fn read_object(fs: &dyn FileSystem, repo_path: &Path, oid: &str) -> Result<StoredObject> {
    let path = object_path(repo_path, oid)?;
    if !fs.exists(&path).await? {
        return Err(AuraError::ObjectNotFound(oid.to_string()));
    }

    let compressed = fs.read(&path).await?;
    let mut decoder = ZlibDecoder::new(&compressed[..]);
    let mut raw = Vec::new();
    decoder.read_to_end(&mut raw)?;

    let separator = raw
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| AuraError::CorruptObject(oid.to_string()))?;
    let header = String::from_utf8(raw[..separator].to_vec())?;
    let mut header_parts = header.splitn(2, ' ');
    let kind = ObjectKind::from_str(
        header_parts
            .next()
            .ok_or_else(|| AuraError::CorruptObject(oid.to_string()))?,
    )?;
    let declared_len = header_parts
        .next()
        .ok_or_else(|| AuraError::CorruptObject(oid.to_string()))?
        .parse::<usize>()
        .map_err(|_| AuraError::CorruptObject(oid.to_string()))?;

    let body = raw[separator + 1..].to_vec();
    if body.len() != declared_len {
        return Err(AuraError::CorruptObject(oid.to_string()));
    }

    Ok(StoredObject { kind, body })
}

/// Writes a blob object and returns its object id.
pub async fn write_blob(fs: &dyn FileSystem, repo_path: &Path, blob: &Blob) -> Result<String> {
    write_object(fs, repo_path, ObjectKind::Blob, &blob.serialize()).await
}

/// Reads a blob object from the database.
pub async fn read_blob(fs: &dyn FileSystem, repo_path: &Path, oid: &str) -> Result<Blob> {
    let object = read_object(fs, repo_path, oid).await?;
    ensure_kind(oid, object.kind, ObjectKind::Blob)?;
    Ok(Blob::deserialize(&object.body))
}

/// Writes a tree object and returns its object id.
pub async fn write_tree(fs: &dyn FileSystem, repo_path: &Path, tree: &Tree) -> Result<String> {
    write_object(fs, repo_path, ObjectKind::Tree, &tree.serialize()?).await
}

/// Reads and parses a tree object.
pub async fn read_tree(fs: &dyn FileSystem, repo_path: &Path, oid: &str) -> Result<Tree> {
    let object = read_object(fs, repo_path, oid).await?;
    ensure_kind(oid, object.kind, ObjectKind::Tree)?;
    Tree::deserialize(&object.body)
}

/// Writes a commit object and returns its object id.
pub async fn write_commit(
    fs: &dyn FileSystem,
    repo_path: &Path,
    commit: &Commit,
) -> Result<String> {
    write_object(fs, repo_path, ObjectKind::Commit, &commit.serialize()).await
}

/// Reads and parses a commit object.
pub async fn read_commit(fs: &dyn FileSystem, repo_path: &Path, oid: &str) -> Result<Commit> {
    let object = read_object(fs, repo_path, oid).await?;
    ensure_kind(oid, object.kind, ObjectKind::Commit)?;
    Commit::deserialize(&object.body)
}

fn ensure_kind(oid: &str, actual: ObjectKind, expected: ObjectKind) -> Result<()> {
    if actual == expected {
        return Ok(());
    }

    Err(AuraError::UnexpectedObjectType {
        oid: oid.to_string(),
        actual: actual.as_str().to_string(),
        expected: expected.as_str().to_string(),
    })
}

fn decode_oid(oid: &str) -> Result<[u8; 20]> {
    let bytes = hex::decode(oid)?;
    if bytes.len() != 20 {
        return Err(AuraError::CorruptObject(oid.to_string()));
    }

    let mut out = [0u8; 20];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn parse_signature(raw: &str) -> Result<(String, i64)> {
    let mut parts = raw.rsplitn(3, ' ');
    let _timezone = parts
        .next()
        .ok_or_else(|| AuraError::CorruptObject("commit".to_string()))?;
    let timestamp = parts
        .next()
        .ok_or_else(|| AuraError::CorruptObject("commit".to_string()))?
        .parse::<i64>()
        .map_err(|_| AuraError::CorruptObject("commit".to_string()))?;
    let name = parts
        .next()
        .ok_or_else(|| AuraError::CorruptObject("commit".to_string()))?
        .to_string();
    Ok((name, timestamp))
}
