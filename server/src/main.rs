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
use async_recursion::async_recursion;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::BTreeSet,
    env,
    path::{Path as StdPath, PathBuf},
    sync::Arc,
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
        Self::new(StatusCode::BAD_REQUEST, anyhow::anyhow!(message.to_string()))
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
    State(_state): State<Arc<AppState>>,
    Query(_query): Query<RepoQuery>,
    Path(_hash): Path<String>,
) -> Result<Response, AppError> {
    Ok(Json(Vec::<serde_json::Value>::new()).into_response())
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
        .ok_or_else(|| AppError::not_found(format!("В репозитории `{}` нет ветки `{branch}`", repo_record.id)))?;
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
        return Err(AppError::bad_request("repo id in URL and payload must match"));
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

async fn write_transport_object(repo: &Repository, packed: &TransportObject) -> Result<(), AppError> {
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

async fn resolve_repo_record(state: &AppState, repo_id: Option<&str>) -> Result<RepoRecord, AppError> {
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
    if let Some(existing) = registry.repos.iter().find(|repo| repo.id == repo_id).cloned() {
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
    registry.repos.sort_by(|left, right| left.name.cmp(&right.name));
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
    registry.repos.sort_by(|left, right| left.name.cmp(&right.name));
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
        .route("/api/repo/diff/:hash", get(get_diff))
        .route("/api/repo/commit", post(post_commit))
        .route("/api/repo/branches", get(get_branches).post(post_branch))
        .route("/api/repo/remotes", get(get_remotes).post(post_remote))
        .route("/api/repo/push", post(post_push))
        .route("/api/repo/pull", post(post_pull))
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
    println!("Aura server storage: {}", storage_root.display());
    let state = init_state(storage_root).await?;
    let app = build_app(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("Server running on http://0.0.0.0:3000");
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
            objects: collect_pack(&repo, &oid).await.expect("pack should be built"),
        };

        let app = build_app(init_state(&storage_root).await.expect("state should init"));
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/transport/repos/demo/push")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&pack).expect("pack should serialize")))
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
        let oid = repo.commit("export commit").await.expect("commit should succeed");

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
