use async_recursion::async_recursion;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, Query, State, WebSocketUpgrade,
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    path::{Path as StdPath, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::{broadcast, Mutex};
use tower_http::cors::{Any, CorsLayer};

use aura_control::{
    object::{self, Blob, Commit, ObjectKind, Tree},
    refs, ChangeKind, Repository,
};

pub struct AppState {
    pub storage_root: PathBuf,
    pub registry_lock: Mutex<()>,
    pub repo_lock: Mutex<()>,
    pub tx: broadcast::Sender<Event>,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type")]
pub enum Event {
    #[serde(rename = "REPO_UPDATED")]
    RepoUpdated { repo_id: String },
    #[serde(rename = "REPO_LIST_CHANGED")]
    RepoListChanged,
}

#[derive(Debug)]
pub struct AppError {
    status: StatusCode,
    error: anyhow::Error,
}

impl AppError {
    fn new(status: StatusCode, error: impl Into<anyhow::Error>) -> Self {
        Self {
            status,
            error: error.into(),
        }
    }

    fn bad_request(message: impl std::fmt::Display) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            anyhow::anyhow!(message.to_string()),
        )
    }

    fn not_found(message: impl std::fmt::Display) -> Self {
        Self::new(StatusCode::NOT_FOUND, anyhow::anyhow!(message.to_string()))
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({
                "error": self.error.to_string(),
            })),
        )
            .into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, err)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RepoRecord {
    id: String,
    name: String,
    path: String,
    storage: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct RepoRegistry {
    repos: Vec<RepoRecord>,
}

#[derive(Deserialize)]
struct RepoQuery {
    repo: Option<String>,
}

#[derive(Deserialize)]
struct ExportQuery {
    branch: Option<String>,
}

#[derive(Deserialize)]
struct TreeQuery {
    repo: Option<String>,
    rev: Option<String>,
    path: Option<String>,
}

#[derive(Deserialize)]
struct FileQuery {
    repo: Option<String>,
    rev: Option<String>,
    path: String,
}

#[derive(Deserialize)]
struct CommitRequest {
    message: String,
}

#[derive(Deserialize)]
struct BranchRequest {
    name: String,
}

#[derive(Deserialize)]
struct RemoteRequest {
    name: String,
    target: String,
}

#[derive(Deserialize)]
struct SyncRequest {
    remote: Option<String>,
    branch: Option<String>,
}

#[derive(Deserialize)]
struct RepoUpsertRequest {
    name: String,
    path: Option<String>,
}

#[derive(Deserialize)]
struct HubItemRequest {
    title: String,
    body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TransportObject {
    oid: String,
    kind: String,
    body_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TransportPack {
    repo_id: String,
    branch: String,
    oid: String,
    objects: Vec<TransportObject>,
}

#[derive(Serialize)]
struct RemoteConfig {
    name: String,
    target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HubItem {
    id: u64,
    title: String,
    body: String,
    status: String,
    author: String,
    created_at: u64,
}

#[derive(Serialize)]
struct TreeEntryResponse {
    name: String,
    path: String,
    kind: &'static str,
    oid: String,
    size: Option<u64>,
}

#[derive(Serialize)]
struct FileResponse {
    path: String,
    oid: String,
    language: String,
    content: String,
}

#[derive(Serialize)]
struct ChangeStatsResponse {
    files_changed: usize,
    insertions: usize,
    deletions: usize,
    files: Vec<FileStatResponse>,
}

#[derive(Serialize)]
struct FileStatResponse {
    path: String,
    kind: &'static str,
    insertions: usize,
    deletions: usize,
}

async fn get_status(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RepoQuery>,
) -> Result<Response, AppError> {
    let repo_record = resolve_repo_record(&state, query.repo.as_deref()).await?;
    let repo = Repository::new(PathBuf::from(&repo_record.path));
    let status = repo.status().await?;

    let staged: Vec<String> = status
        .staged
        .into_iter()
        .map(|entry| entry.path.to_string_lossy().to_string())
        .collect();
    let unstaged: Vec<String> = status
        .unstaged
        .into_iter()
        .map(|entry| entry.path.to_string_lossy().to_string())
        .collect();
    let untracked: Vec<String> = status
        .untracked
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();

    Ok(Json(json!({
        "repo": repo_record.id,
        "staged": staged,
        "unstaged": unstaged,
        "untracked": untracked,
        "branch": status.branch.unwrap_or_else(|| "main".to_string()),
        "head": status.head,
    }))
    .into_response())
}

async fn get_log(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RepoQuery>,
) -> Result<Response, AppError> {
    let repo_record = resolve_repo_record(&state, query.repo.as_deref()).await?;
    let repo = Repository::new(PathBuf::from(&repo_record.path));
    let log = repo.log().await.unwrap_or_default();

    let commits: Vec<_> = log
        .into_iter()
        .map(|commit| {
            json!({
                "hash": commit.oid,
                "author": commit.author,
                "message": commit.message,
                "date": commit.timestamp.to_rfc3339(),
            })
        })
        .collect();

    Ok(Json(commits).into_response())
}

async fn get_diff(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RepoQuery>,
    Path(hash): Path<String>,
) -> Result<Response, AppError> {
    let repo_record = resolve_repo_record(&state, query.repo.as_deref()).await?;
    let repo = Repository::new(PathBuf::from(&repo_record.path));
    let commit = object::read_commit(repo.filesystem(), &repo.path, &hash).await?;
    let target = collect_tree_entries(&repo, &commit.tree).await?;
    let base = match commit.parents.first() {
        Some(parent) => {
            let parent_commit = object::read_commit(repo.filesystem(), &repo.path, parent).await?;
            collect_tree_entries(&repo, &parent_commit.tree).await?
        }
        None => BTreeMap::new(),
    };

    let mut paths = BTreeSet::new();
    paths.extend(base.keys().cloned());
    paths.extend(target.keys().cloned());

    let mut diffs = Vec::new();
    for path in paths {
        let status = match (base.get(&path), target.get(&path)) {
            (Some(left), Some(right)) if left == right => continue,
            (None, Some(_)) => "added",
            (Some(_), None) => "deleted",
            (Some(_), Some(_)) => "modified",
            (None, None) => continue,
        };
        let old_text = match base.get(&path) {
            Some(oid) => String::from_utf8_lossy(
                &object::read_blob(repo.filesystem(), &repo.path, oid)
                    .await?
                    .content,
            )
            .into_owned(),
            None => String::new(),
        };
        let new_text = match target.get(&path) {
            Some(oid) => String::from_utf8_lossy(
                &object::read_blob(repo.filesystem(), &repo.path, oid)
                    .await?
                    .content,
            )
            .into_owned(),
            None => String::new(),
        };
        diffs.push(json!({
            "path": path,
            "status": status,
            "diff": render_simple_diff(&old_text, &new_text),
        }));
    }

    Ok(Json(diffs).into_response())
}

async fn get_tree(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TreeQuery>,
) -> Result<Response, AppError> {
    let repo_record = resolve_repo_record(&state, query.repo.as_deref()).await?;
    let repo = Repository::new(PathBuf::from(&repo_record.path));
    let Some(commit_oid) = resolve_commit_oid(&repo, query.rev.as_deref()).await? else {
        return Ok(Json(Vec::<TreeEntryResponse>::new()).into_response());
    };
    let commit = object::read_commit(repo.filesystem(), &repo.path, &commit_oid).await?;
    let tree =
        resolve_tree_at_path(&repo, &commit.tree, query.path.as_deref().unwrap_or("")).await?;
    let prefix = normalize_repo_path(query.path.as_deref().unwrap_or(""));

    let mut entries = Vec::new();
    for entry in tree.entries {
        let path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            format!("{prefix}/{}", entry.name)
        };
        let (kind, size) = if entry.mode == "40000" {
            ("dir", None)
        } else {
            let blob = object::read_blob(repo.filesystem(), &repo.path, &entry.oid).await?;
            ("file", Some(blob.content.len() as u64))
        };
        entries.push(TreeEntryResponse {
            name: entry.name,
            path,
            kind,
            oid: entry.oid,
            size,
        });
    }
    entries.sort_by(|left, right| {
        let left_rank = if left.kind == "dir" { 0 } else { 1 };
        let right_rank = if right.kind == "dir" { 0 } else { 1 };
        (left_rank, &left.name).cmp(&(right_rank, &right.name))
    });

    Ok(Json(entries).into_response())
}

#[axum::debug_handler]
async fn get_file(
    State(state): State<Arc<AppState>>,
    Query(query): Query<FileQuery>,
) -> Result<Response, AppError> {
    let repo_record = resolve_repo_record(&state, query.repo.as_deref()).await?;
    let repo = Repository::new(PathBuf::from(&repo_record.path));
    let commit_oid = resolve_commit_oid(&repo, query.rev.as_deref())
        .await?
        .ok_or_else(|| AppError::not_found("Repository has no commits"))?;
    let commit = object::read_commit(repo.filesystem(), &repo.path, &commit_oid).await?;
    let path = normalize_repo_path(&query.path);
    let blob_oid = resolve_blob_at_path(&repo, &commit.tree, &path).await?;
    let blob = object::read_blob(repo.filesystem(), &repo.path, &blob_oid).await?;

    Ok(Json(FileResponse {
        path: path.clone(),
        oid: blob_oid,
        language: language_for_path(&path).to_string(),
        content: String::from_utf8_lossy(&blob.content).into_owned(),
    })
    .into_response())
}

async fn post_commit(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RepoQuery>,
    Json(payload): Json<CommitRequest>,
) -> Result<Response, AppError> {
    let repo_record = resolve_repo_record(&state, query.repo.as_deref()).await?;
    let _guard = state.repo_lock.lock().await;
    let repo = Repository::new(PathBuf::from(&repo_record.path));
    repo.add_all().await?;
    let hash = repo.commit(&payload.message).await?;

    let _ = state.tx.send(Event::RepoUpdated {
        repo_id: repo_record.id,
    });

    Ok(Json(json!({ "hash": hash })).into_response())
}

async fn get_branches(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RepoQuery>,
) -> Result<Response, AppError> {
    let repo_record = resolve_repo_record(&state, query.repo.as_deref()).await?;
    let repo = Repository::new(PathBuf::from(&repo_record.path));
    let branches = refs::list_branches(repo.filesystem(), &repo.path).await?;
    Ok(Json(branches).into_response())
}

async fn post_branch(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RepoQuery>,
    Json(payload): Json<BranchRequest>,
) -> Result<Response, AppError> {
    let repo_record = resolve_repo_record(&state, query.repo.as_deref()).await?;
    let _guard = state.repo_lock.lock().await;
    let repo = Repository::new(PathBuf::from(&repo_record.path));
    repo.branch(&payload.name).await?;
    let _ = state.tx.send(Event::RepoUpdated {
        repo_id: repo_record.id,
    });
    Ok(Json(json!({ "status": "ok" })).into_response())
}

async fn get_remotes(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RepoQuery>,
) -> Result<Response, AppError> {
    let repo_record = resolve_repo_record(&state, query.repo.as_deref()).await?;
    let repo = Repository::new(PathBuf::from(&repo_record.path));
    Ok(Json(list_remotes(&repo).await?).into_response())
}

async fn post_remote(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RepoQuery>,
    Json(payload): Json<RemoteRequest>,
) -> Result<Response, AppError> {
    let repo_record = resolve_repo_record(&state, query.repo.as_deref()).await?;
    let _guard = state.repo_lock.lock().await;
    let repo = Repository::new(PathBuf::from(&repo_record.path));
    let target = repo.add_remote(&payload.name, &payload.target).await?;
    let _ = state.tx.send(Event::RepoUpdated {
        repo_id: repo_record.id,
    });

    Ok(Json(RemoteConfig {
        name: payload.name,
        target: target.to_string_lossy().to_string(),
    })
    .into_response())
}

async fn post_push(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RepoQuery>,
    Json(payload): Json<SyncRequest>,
) -> Result<Response, AppError> {
    let repo_record = resolve_repo_record(&state, query.repo.as_deref()).await?;
    let _guard = state.repo_lock.lock().await;
    let repo = Repository::new(PathBuf::from(&repo_record.path));
    let summary = repo
        .push(payload.remote.as_deref(), payload.branch.as_deref())
        .await?;

    let _ = state.tx.send(Event::RepoUpdated {
        repo_id: repo_record.id,
    });

    Ok(Json(json!({
        "remote": summary.remote,
        "branch": summary.branch,
        "hash": summary.oid,
        "target": summary.target.to_string_lossy().to_string(),
    }))
    .into_response())
}

async fn post_pull(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RepoQuery>,
    Json(payload): Json<SyncRequest>,
) -> Result<Response, AppError> {
    let repo_record = resolve_repo_record(&state, query.repo.as_deref()).await?;
    let _guard = state.repo_lock.lock().await;
    let repo = Repository::new(PathBuf::from(&repo_record.path));
    let summary = repo
        .pull(payload.remote.as_deref(), payload.branch.as_deref())
        .await?;

    let _ = state.tx.send(Event::RepoUpdated {
        repo_id: repo_record.id,
    });

    Ok(Json(json!({
        "remote": summary.remote,
        "source_branch": summary.source_branch,
        "local_branch": summary.local_branch,
        "hash": summary.oid,
        "previous_hash": summary.previous_oid,
        "status": pull_status_to_str(summary.status),
        "target": summary.target.to_string_lossy().to_string(),
        "stats": summary.stats.map(map_change_stats),
    }))
    .into_response())
}

async fn list_repositories(State(state): State<Arc<AppState>>) -> Result<Response, AppError> {
    Ok(Json(load_registry(&state).await?.repos).into_response())
}

async fn upsert_repository(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RepoUpsertRequest>,
) -> Result<Response, AppError> {
    let record = if let Some(path) = payload.path {
        register_existing_repo(&state, &payload.name, &path).await?
    } else {
        ensure_hosted_repo(&state, &slugify(&payload.name), Some(payload.name.clone())).await?
    };
    let _ = state.tx.send(Event::RepoListChanged);
    Ok(Json(record).into_response())
}

async fn list_issues(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RepoQuery>,
) -> Result<Response, AppError> {
    let repo_record = resolve_repo_record(&state, query.repo.as_deref()).await?;
    let repo = Repository::new(PathBuf::from(&repo_record.path));
    Ok(Json(load_hub_items(&repo, "issues").await?).into_response())
}

async fn create_issue(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RepoQuery>,
    Json(payload): Json<HubItemRequest>,
) -> Result<Response, AppError> {
    let repo_record = resolve_repo_record(&state, query.repo.as_deref()).await?;
    let repo = Repository::new(PathBuf::from(&repo_record.path));
    let item = append_hub_item(&repo, "issues", payload).await?;
    let _ = state.tx.send(Event::RepoUpdated {
        repo_id: repo_record.id,
    });
    Ok(Json(item).into_response())
}

async fn list_pull_requests(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RepoQuery>,
) -> Result<Response, AppError> {
    let repo_record = resolve_repo_record(&state, query.repo.as_deref()).await?;
    let repo = Repository::new(PathBuf::from(&repo_record.path));
    Ok(Json(load_hub_items(&repo, "pull_requests").await?).into_response())
}

async fn create_pull_request(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RepoQuery>,
    Json(payload): Json<HubItemRequest>,
) -> Result<Response, AppError> {
    let repo_record = resolve_repo_record(&state, query.repo.as_deref()).await?;
    let repo = Repository::new(PathBuf::from(&repo_record.path));
    let item = append_hub_item(&repo, "pull_requests", payload).await?;
    let _ = state.tx.send(Event::RepoUpdated {
        repo_id: repo_record.id,
    });
    Ok(Json(item).into_response())
}

async fn export_pack(
    State(state): State<Arc<AppState>>,
    Path(repo_id): Path<String>,
    Query(query): Query<ExportQuery>,
) -> Result<Response, AppError> {
    let repo_record = resolve_repo_record(&state, Some(repo_id.as_str())).await?;
    let repo = Repository::new(PathBuf::from(&repo_record.path));
    let branch = match query.branch {
        Some(branch) => branch,
        None => refs::current_branch(repo.filesystem(), &repo.path)
            .await?
            .unwrap_or_else(|| "main".to_string()),
    };
    let oid = refs::read_branch(repo.filesystem(), &repo.path, &branch)
        .await?
        .ok_or_else(|| {
            AppError::not_found(format!(
                "В репозитории `{}` нет ветки `{branch}`",
                repo_record.id
            ))
        })?;
    let objects = collect_pack(&repo, &oid).await?;

    Ok(Json(TransportPack {
        repo_id: repo_record.id,
        branch,
        oid,
        objects,
    })
    .into_response())
}

async fn import_pack(
    State(state): State<Arc<AppState>>,
    Path(repo_id): Path<String>,
    Json(payload): Json<TransportPack>,
) -> Result<Response, AppError> {
    if repo_id != payload.repo_id {
        return Err(AppError::bad_request(
            "repo id in URL and payload must match",
        ));
    }

    let _guard = state.repo_lock.lock().await;
    let repo_record = ensure_hosted_repo(&state, &repo_id, None).await?;
    let repo = Repository::new(PathBuf::from(&repo_record.path));
    repo.init_repository().await?;

    for packed in &payload.objects {
        write_transport_object(&repo, packed).await?;
    }

    refs::write_branch(repo.filesystem(), &repo.path, &payload.branch, &payload.oid).await?;
    repo.materialize_branch(&payload.branch).await?;

    let _ = state.tx.send(Event::RepoUpdated {
        repo_id: repo_record.id.clone(),
    });
    let _ = state.tx.send(Event::RepoListChanged);

    Ok(Json(json!({
        "repo_id": repo_record.id,
        "branch": payload.branch,
        "hash": payload.oid,
        "object_count": payload.objects.len(),
        "path": repo_record.path,
    }))
    .into_response())
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.tx.subscribe();
    while let Ok(event) = rx.recv().await {
        if let Ok(message) = serde_json::to_string(&event) {
            if socket.send(Message::Text(message)).await.is_err() {
                break;
            }
        }
    }
}

async fn list_remotes(repo: &Repository) -> Result<Vec<RemoteConfig>, AppError> {
    let root = repo.path.join(".aura/remotes");
    if !repo.filesystem().exists(&root).await? {
        return Ok(Vec::new());
    }

    let mut entries = repo.filesystem().read_dir(&root).await?;
    entries.sort();

    let mut remotes = Vec::new();
    for path in entries {
        let metadata = repo.filesystem().metadata(&path).await?;
        if !metadata.is_file {
            continue;
        }

        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };

        if let Some(target) = refs::read_remote(repo.filesystem(), &repo.path, name).await? {
            remotes.push(RemoteConfig {
                name: name.to_string(),
                target,
            });
        }
    }

    Ok(remotes)
}

async fn collect_pack(repo: &Repository, oid: &str) -> Result<Vec<TransportObject>, AppError> {
    let mut seen = BTreeSet::new();
    let mut objects = Vec::new();
    collect_commit_objects(repo, oid, &mut seen, &mut objects).await?;
    Ok(objects)
}

#[async_recursion]
async fn collect_commit_objects(
    repo: &Repository,
    oid: &str,
    seen: &mut BTreeSet<String>,
    objects: &mut Vec<TransportObject>,
) -> Result<(), AppError> {
    if !seen.insert(oid.to_string()) {
        return Ok(());
    }

    let commit = object::read_commit(repo.filesystem(), &repo.path, oid).await?;
    objects.push(TransportObject {
        oid: oid.to_string(),
        kind: "commit".to_string(),
        body_base64: STANDARD.encode(commit.serialize()),
    });

    collect_tree_objects(repo, &commit.tree, seen, objects).await?;
    for parent in &commit.parents {
        collect_commit_objects(repo, parent, seen, objects).await?;
    }

    Ok(())
}

#[async_recursion]
async fn collect_tree_objects(
    repo: &Repository,
    oid: &str,
    seen: &mut BTreeSet<String>,
    objects: &mut Vec<TransportObject>,
) -> Result<(), AppError> {
    if !seen.insert(oid.to_string()) {
        return Ok(());
    }

    let tree = object::read_tree(repo.filesystem(), &repo.path, oid).await?;
    objects.push(TransportObject {
        oid: oid.to_string(),
        kind: "tree".to_string(),
        body_base64: STANDARD.encode(tree.serialize()?),
    });

    for entry in &tree.entries {
        if entry.mode == "40000" {
            collect_tree_objects(repo, &entry.oid, seen, objects).await?;
        } else {
            collect_blob_object(repo, &entry.oid, seen, objects).await?;
        }
    }

    Ok(())
}

async fn collect_blob_object(
    repo: &Repository,
    oid: &str,
    seen: &mut BTreeSet<String>,
    objects: &mut Vec<TransportObject>,
) -> Result<(), AppError> {
    if !seen.insert(oid.to_string()) {
        return Ok(());
    }

    let blob = object::read_blob(repo.filesystem(), &repo.path, oid).await?;
    objects.push(TransportObject {
        oid: oid.to_string(),
        kind: "blob".to_string(),
        body_base64: STANDARD.encode(blob.serialize()),
    });

    Ok(())
}

async fn write_transport_object(
    repo: &Repository,
    packed: &TransportObject,
) -> Result<(), AppError> {
    let body = STANDARD
        .decode(&packed.body_base64)
        .map_err(|error| AppError::bad_request(error.to_string()))?;
    let kind = ObjectKind::from_str(&packed.kind)
        .map_err(|error| AppError::bad_request(error.to_string()))?;

    match kind {
        ObjectKind::Blob => {
            object::write_blob(repo.filesystem(), &repo.path, &Blob::deserialize(&body)).await?;
        }
        ObjectKind::Tree => {
            let tree = Tree::deserialize(&body)?;
            object::write_tree(repo.filesystem(), &repo.path, &tree).await?;
        }
        ObjectKind::Commit => {
            let commit = Commit::deserialize(&body)?;
            object::write_commit(repo.filesystem(), &repo.path, &commit).await?;
        }
    }

    Ok(())
}

async fn resolve_commit_oid(
    repo: &Repository,
    rev: Option<&str>,
) -> Result<Option<String>, AppError> {
    let rev = match rev {
        Some(value) if !value.trim().is_empty() => value,
        _ => return Ok(refs::resolve_head(repo.filesystem(), &repo.path).await?),
    };

    if let Some(oid) = refs::resolve_reference(repo.filesystem(), &repo.path, rev).await? {
        return Ok(Some(oid));
    }
    if let Some(oid) = refs::read_branch(repo.filesystem(), &repo.path, rev).await? {
        return Ok(Some(oid));
    }
    if let Some((remote, branch)) = rev.split_once('/') {
        if let Some(oid) =
            refs::read_remote_branch(repo.filesystem(), &repo.path, remote, branch).await?
        {
            return Ok(Some(oid));
        }
    }
    if object::read_commit(repo.filesystem(), &repo.path, rev)
        .await
        .is_ok()
    {
        return Ok(Some(rev.to_string()));
    }
    Ok(None)
}

#[async_recursion]
async fn collect_tree_entries(
    repo: &Repository,
    tree_oid: &str,
) -> Result<BTreeMap<String, String>, AppError> {
    collect_tree_entries_with_prefix(repo, tree_oid, "").await
}

#[async_recursion]
async fn collect_tree_entries_with_prefix(
    repo: &Repository,
    tree_oid: &str,
    prefix: &str,
) -> Result<BTreeMap<String, String>, AppError> {
    let tree = object::read_tree(repo.filesystem(), &repo.path, tree_oid).await?;
    let mut entries = BTreeMap::new();
    for entry in tree.entries {
        let path = if prefix.is_empty() {
            entry.name
        } else {
            format!("{prefix}/{}", entry.name)
        };
        if entry.mode == "40000" {
            entries.extend(collect_tree_entries_with_prefix(repo, &entry.oid, &path).await?);
        } else {
            entries.insert(path, entry.oid);
        }
    }
    Ok(entries)
}

#[async_recursion]
async fn resolve_tree_at_path(
    repo: &Repository,
    tree_oid: &str,
    path: &str,
) -> Result<Tree, AppError> {
    let normalized = normalize_repo_path(path);
    if normalized.is_empty() {
        return Ok(object::read_tree(repo.filesystem(), &repo.path, tree_oid).await?);
    }

    let mut current_tree = object::read_tree(repo.filesystem(), &repo.path, tree_oid).await?;
    for segment in normalized.split('/') {
        let entry = current_tree
            .entries
            .iter()
            .find(|entry| entry.name == segment && entry.mode == "40000")
            .cloned()
            .ok_or_else(|| AppError::not_found(format!("Directory `{normalized}` not found")))?;
        current_tree = object::read_tree(repo.filesystem(), &repo.path, &entry.oid).await?;
    }
    Ok(current_tree)
}

async fn resolve_blob_at_path(
    repo: &Repository,
    tree_oid: &str,
    path: &str,
) -> Result<String, AppError> {
    let normalized = normalize_repo_path(path);
    let segments = normalized
        .split('/')
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut current_tree = object::read_tree(repo.filesystem(), &repo.path, tree_oid).await?;

    for (index, segment) in segments.iter().enumerate() {
        let is_leaf = index + 1 == segments.len();
        let entry = current_tree
            .entries
            .iter()
            .find(|entry| entry.name == *segment)
            .cloned()
            .ok_or_else(|| AppError::not_found(format!("Path `{normalized}` not found")))?;

        if is_leaf && entry.mode != "40000" {
            return Ok(entry.oid);
        }
        if entry.mode != "40000" {
            return Err(AppError::bad_request(format!(
                "`{normalized}` is not a file"
            )));
        }
        current_tree = object::read_tree(repo.filesystem(), &repo.path, &entry.oid).await?;
    }

    Err(AppError::bad_request("file path is required"))
}

fn normalize_repo_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    normalized
        .split('/')
        .filter(|part| !part.is_empty() && *part != "." && *part != "..")
        .collect::<Vec<_>>()
        .join("/")
}

fn language_for_path(path: &str) -> &'static str {
    match StdPath::new(path)
        .extension()
        .and_then(|value| value.to_str())
    {
        Some("rs") => "rust",
        Some("ts") | Some("tsx") => "typescript",
        Some("js") | Some("jsx") => "javascript",
        Some("json") => "json",
        Some("md") => "markdown",
        Some("toml") => "toml",
        Some("css") => "css",
        Some("html") => "html",
        Some("sh") => "bash",
        _ => "text",
    }
}

fn render_simple_diff(old_text: &str, new_text: &str) -> String {
    if old_text == new_text {
        return String::new();
    }

    let mut output = String::new();
    for line in old_text.lines() {
        output.push('-');
        output.push_str(line);
        output.push('\n');
    }
    for line in new_text.lines() {
        output.push('+');
        output.push_str(line);
        output.push('\n');
    }
    output
}

fn hub_items_path(repo: &Repository, collection: &str) -> PathBuf {
    repo.path
        .join(".aura")
        .join("hub")
        .join(format!("{collection}.json"))
}

async fn load_hub_items(repo: &Repository, collection: &str) -> Result<Vec<HubItem>, AppError> {
    let path = hub_items_path(repo, collection);
    if !tokio::fs::try_exists(&path).await? {
        return Ok(Vec::new());
    }
    let raw = tokio::fs::read_to_string(path).await?;
    Ok(serde_json::from_str(&raw)?)
}

async fn save_hub_items(
    repo: &Repository,
    collection: &str,
    items: &[HubItem],
) -> Result<(), AppError> {
    let path = hub_items_path(repo, collection);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(path, serde_json::to_string_pretty(items)?).await?;
    Ok(())
}

async fn append_hub_item(
    repo: &Repository,
    collection: &str,
    payload: HubItemRequest,
) -> Result<HubItem, AppError> {
    let title = payload.title.trim();
    if title.is_empty() {
        return Err(AppError::bad_request("title is required"));
    }

    let mut items = load_hub_items(repo, collection).await?;
    let id = items.iter().map(|item| item.id).max().unwrap_or(0) + 1;
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let item = HubItem {
        id,
        title: title.to_string(),
        body: payload.body.unwrap_or_default(),
        status: "open".to_string(),
        author: "Aura User".to_string(),
        created_at,
    };
    items.push(item.clone());
    save_hub_items(repo, collection, &items).await?;
    Ok(item)
}

fn map_change_stats(stats: aura_control::ChangeStats) -> ChangeStatsResponse {
    ChangeStatsResponse {
        files_changed: stats.files_changed,
        insertions: stats.insertions,
        deletions: stats.deletions,
        files: stats
            .files
            .into_iter()
            .map(|file| FileStatResponse {
                path: file.path.to_string_lossy().to_string(),
                kind: change_kind_to_str(file.kind),
                insertions: file.insertions,
                deletions: file.deletions,
            })
            .collect(),
    }
}

fn change_kind_to_str(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Added => "added",
        ChangeKind::Modified => "modified",
        ChangeKind::Deleted => "deleted",
    }
}

fn pull_status_to_str(status: aura_control::PullStatus) -> &'static str {
    match status {
        aura_control::PullStatus::AlreadyUpToDate => "already_up_to_date",
        aura_control::PullStatus::FastForward => "fast_forward",
    }
}

fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        format!("repo-{}", uuid::Uuid::new_v4().simple())
    } else {
        slug
    }
}

fn registry_file(storage_root: &StdPath) -> PathBuf {
    storage_root.join("registry.json")
}

fn repos_root(storage_root: &StdPath) -> PathBuf {
    storage_root.join("repos")
}

async fn ensure_storage_layout(state: &AppState) -> Result<(), AppError> {
    tokio::fs::create_dir_all(repos_root(&state.storage_root)).await?;
    Ok(())
}

async fn load_registry(state: &AppState) -> Result<RepoRegistry, AppError> {
    let _guard = state.registry_lock.lock().await;
    ensure_storage_layout(state).await?;
    let path = registry_file(&state.storage_root);
    if !tokio::fs::try_exists(&path).await? {
        return Ok(RepoRegistry::default());
    }

    let raw = tokio::fs::read_to_string(path).await?;
    Ok(serde_json::from_str(&raw)?)
}

async fn save_registry(state: &AppState, registry: &RepoRegistry) -> Result<(), AppError> {
    let _guard = state.registry_lock.lock().await;
    ensure_storage_layout(state).await?;
    let path = registry_file(&state.storage_root);
    let raw = serde_json::to_string_pretty(registry)?;
    tokio::fs::write(path, raw).await?;
    Ok(())
}

async fn resolve_repo_record(
    state: &AppState,
    repo_id: Option<&str>,
) -> Result<RepoRecord, AppError> {
    let registry = load_registry(state).await?;
    if let Some(repo_id) = repo_id {
        return registry
            .repos
            .into_iter()
            .find(|repo| repo.id == repo_id)
            .ok_or_else(|| AppError::not_found(format!("Репозиторий `{repo_id}` не найден")));
    }

    match registry.repos.len() {
        0 => Err(AppError::not_found(
            "На сервере нет зарегистрированных репозиториев. Создай hosted repo или зарегистрируй существующий путь.",
        )),
        1 => Ok(registry.repos.into_iter().next().expect("one repo exists")),
        _ => Err(AppError::bad_request(
            "Нужно выбрать репозиторий: передай query `?repo=<id>`.",
        )),
    }
}

async fn ensure_hosted_repo(
    state: &AppState,
    repo_id: &str,
    name: Option<String>,
) -> Result<RepoRecord, AppError> {
    let mut registry = load_registry(state).await?;
    if let Some(existing) = registry
        .repos
        .iter()
        .find(|repo| repo.id == repo_id)
        .cloned()
    {
        let repo = Repository::new(PathBuf::from(&existing.path));
        repo.init_repository().await?;
        return Ok(existing);
    }

    ensure_storage_layout(state).await?;
    let path = repos_root(&state.storage_root).join(repo_id);
    let repo = Repository::new(&path);
    repo.init_repository().await?;

    let record = RepoRecord {
        id: repo_id.to_string(),
        name: name.unwrap_or_else(|| repo_id.to_string()),
        path: path.to_string_lossy().to_string(),
        storage: "hosted".to_string(),
    };
    registry.repos.push(record.clone());
    registry
        .repos
        .sort_by(|left, right| left.name.cmp(&right.name));
    save_registry(state, &registry).await?;
    Ok(record)
}

async fn register_existing_repo(
    state: &AppState,
    name: &str,
    path: &str,
) -> Result<RepoRecord, AppError> {
    let repo_path = PathBuf::from(path);
    if !tokio::fs::try_exists(repo_path.join(".aura")).await? {
        return Err(AppError::bad_request(format!(
            "Путь `{path}` не содержит Aura-репозиторий (.aura не найден)."
        )));
    }

    let mut registry = load_registry(state).await?;
    let repo_id = slugify(name);
    let record = RepoRecord {
        id: repo_id.clone(),
        name: name.to_string(),
        path: repo_path.to_string_lossy().to_string(),
        storage: "linked".to_string(),
    };

    if let Some(existing) = registry.repos.iter_mut().find(|repo| repo.id == repo_id) {
        *existing = record.clone();
    } else {
        registry.repos.push(record.clone());
    }
    registry
        .repos
        .sort_by(|left, right| left.name.cmp(&right.name));
    save_registry(state, &registry).await?;
    Ok(record)
}

async fn init_state(storage_root: impl Into<PathBuf>) -> anyhow::Result<Arc<AppState>> {
    let (tx, _rx) = broadcast::channel(100);
    let state = Arc::new(AppState {
        storage_root: storage_root.into(),
        registry_lock: Mutex::new(()),
        repo_lock: Mutex::new(()),
        tx,
    });
    ensure_storage_layout(&state)
        .await
        .map_err(|error| anyhow::anyhow!(error.error.to_string()))?;
    Ok(state)
}

fn build_app(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/repos", get(list_repositories).post(upsert_repository))
        .route("/api/repo/status", get(get_status))
        .route("/api/repo/log", get(get_log))
        .route("/api/repo/tree", get(get_tree))
        .route("/api/repo/file", get(get_file))
        .route("/api/repo/diff/:hash", get(get_diff))
        .route("/api/repo/commit", post(post_commit))
        .route("/api/repo/branches", get(get_branches).post(post_branch))
        .route("/api/repo/remotes", get(get_remotes).post(post_remote))
        .route("/api/repo/push", post(post_push))
        .route("/api/repo/pull", post(post_pull))
        .route("/api/repo/issues", get(list_issues).post(create_issue))
        .route(
            "/api/repo/pull-requests",
            get(list_pull_requests).post(create_pull_request),
        )
        .route("/api/transport/repos/:repo_id/push", post(import_pack))
        .route("/api/transport/repos/:repo_id/export", get(export_pack))
        .route("/api/events", get(ws_handler))
        .layer(cors)
        .with_state(state)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let storage_root = env::var("AURA_STORAGE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".aura-server"));
    let api_port = env::var("AURA_API_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(3000);
    println!("Aura server storage: {}", storage_root.display());
    let state = init_state(storage_root).await?;
    let app = build_app(state);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", api_port)).await?;
    println!("Server running on http://0.0.0.0:{api_port}");
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    fn temp_path(name: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("aura-server-{name}-{unique}"))
    }

    async fn json_body(response: axum::response::Response) -> serde_json::Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should be readable");
        serde_json::from_slice(&bytes).expect("response should be valid json")
    }

    async fn create_commit(repo_path: &PathBuf, file_name: &str, message: &str) -> Repository {
        let repo = Repository::init(repo_path).await.expect("repo should init");
        tokio::fs::write(repo_path.join(file_name), format!("{message}\n"))
            .await
            .expect("file should be written");
        repo.add_all().await.expect("files should be staged");
        repo.commit(message).await.expect("commit should succeed");
        repo
    }

    #[tokio::test(flavor = "current_thread")]
    async fn repositories_can_be_registered_and_listed() {
        let storage_root = temp_path("registry");
        let linked_repo = temp_path("linked-repo");
        let _repo = create_commit(&linked_repo, "main.txt", "linked repo").await;
        let app = build_app(init_state(&storage_root).await.expect("state should init"));

        let response = app
            .clone()
            .oneshot(
                Request::post("/api/repos")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "test",
                            "path": linked_repo.to_string_lossy(),
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        let list_response = app
            .oneshot(
                Request::get("/api/repos")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let body = json_body(list_response).await;
        let repos = body.as_array().expect("repos should be an array");
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0]["id"], "test");
        assert_eq!(repos[0]["storage"], "linked");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn transport_push_creates_hosted_repo_and_updates_log() {
        let storage_root = temp_path("transport-storage");
        let local_repo = temp_path("transport-local");
        let repo = create_commit(&local_repo, "hello.txt", "first hosted commit").await;
        let oid = refs::read_branch(repo.filesystem(), &repo.path, "main")
            .await
            .expect("main should be readable")
            .expect("main should exist");
        let pack = TransportPack {
            repo_id: "demo".to_string(),
            branch: "main".to_string(),
            oid: oid.clone(),
            objects: collect_pack(&repo, &oid)
                .await
                .expect("pack should be built"),
        };

        let app = build_app(init_state(&storage_root).await.expect("state should init"));
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/transport/repos/demo/push")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_string(&pack).expect("pack should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        let repos_response = app
            .clone()
            .oneshot(
                Request::get("/api/repos")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let repos = json_body(repos_response).await;
        assert_eq!(repos.as_array().expect("repos should be array").len(), 1);

        let log_response = app
            .oneshot(
                Request::get("/api/repo/log?repo=demo")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let log = json_body(log_response).await;
        let entries = log.as_array().expect("log should be an array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["message"], "first hosted commit");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn transport_export_returns_branch_objects() {
        let storage_root = temp_path("export-storage");
        let repo = ensure_hosted_repo(
            &init_state(&storage_root).await.expect("state should init"),
            "export-demo",
            Some("Export Demo".to_string()),
        )
        .await
        .expect("hosted repo should be created");
        let repo = Repository::new(PathBuf::from(repo.path));
        tokio::fs::write(repo.path.join("lib.txt"), "exported\n")
            .await
            .expect("file should be written");
        repo.add_all().await.expect("files should be staged");
        let oid = repo
            .commit("export commit")
            .await
            .expect("commit should succeed");

        let app = build_app(init_state(&storage_root).await.expect("state should init"));
        let response = app
            .oneshot(
                Request::get("/api/transport/repos/export-demo/export?branch=main")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["oid"], oid);
        assert!(body["objects"]
            .as_array()
            .expect("objects should be array")
            .iter()
            .any(|value| value["kind"] == "commit"));
    }
}
