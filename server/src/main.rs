use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{env, sync::Arc};
use tokio::sync::{broadcast, Mutex};
use tower_http::cors::{Any, CorsLayer};

use aura_control::Repository;

// --- State & Events ---

pub struct AppState {
    pub repo: Arc<Mutex<Repository>>,
    pub tx: broadcast::Sender<Event>,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type")]
pub enum Event {
    #[serde(rename = "COMMIT_ADDED")]
    CommitAdded,
    #[serde(rename = "STATUS_CHANGED")]
    StatusChanged,
}

// --- Error Handling ---

pub struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let error_message = self.0.to_string();
        let body = Json(json!({
            "error": error_message,
        }));
        (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

// --- Requests ---

#[derive(Deserialize)]
struct CommitRequest {
    message: String,
}

#[derive(Deserialize)]
struct BranchRequest {
    name: String,
}

// --- Endpoints ---

async fn get_status(State(state): State<Arc<AppState>>) -> Result<Response, AppError> {
    let repo = state.repo.lock().await;
    let status = repo.status().await?;
    
    let staged: Vec<String> = status.staged.into_iter().map(|e| e.path.to_string_lossy().to_string()).collect();
    let unstaged: Vec<String> = status.unstaged.into_iter().map(|e| e.path.to_string_lossy().to_string()).collect();
    let untracked: Vec<String> = status.untracked.into_iter().map(|p| p.to_string_lossy().to_string()).collect();
    
    Ok(Json(json!({
        "staged": staged,
        "unstaged": unstaged,
        "untracked": untracked,
        "branch": status.branch.unwrap_or_else(|| "main".to_string()),
    })).into_response())
}

async fn get_log(
    State(state): State<Arc<AppState>>,
) -> Result<Response, AppError> {
    let repo = state.repo.lock().await;
    let log = repo.log().await.unwrap_or_default();
    
    let commits: Vec<_> = log.into_iter().map(|c| {
        json!({
            "hash": c.oid,
            "author": c.author,
            "message": c.message,
            "date": c.timestamp.to_rfc3339(),
        })
    }).collect();

    Ok(Json(commits).into_response())
}

async fn get_diff(
    State(_state): State<Arc<AppState>>,
    Path(_hash): Path<String>,
) -> Result<Response, AppError> {
    // aura_control currently doesn't expose a way to diff a specific commit easily.
    // Returning an empty array to satisfy the client for now.
    let diff: Vec<serde_json::Value> = vec![];
    Ok(Json(diff).into_response())
}

async fn post_commit(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CommitRequest>,
) -> Result<Response, AppError> {
    let repo = state.repo.lock().await;
    
    // Add all changes first
    let _ = repo.add_all().await?;
    
    // Then commit
    let hash = repo.commit(&payload.message).await?;

    let _ = state.tx.send(Event::CommitAdded);
    let _ = state.tx.send(Event::StatusChanged);

    Ok(Json(json!({ "hash": hash })).into_response())
}

async fn get_branches(
    State(state): State<Arc<AppState>>,
) -> Result<Response, AppError> {
    let repo = state.repo.lock().await;
    let mut branches = Vec::new();
    let heads_path = repo.path.join(".aura/refs/heads");
    
    if let Ok(mut dir) = tokio::fs::read_dir(heads_path).await {
        while let Ok(Some(entry)) = dir.next_entry().await {
            if let Ok(name) = entry.file_name().into_string() {
                branches.push(name);
            }
        }
    }
    Ok(Json(branches).into_response())
}

async fn post_branch(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<BranchRequest>,
) -> Result<Response, AppError> {
    let repo = state.repo.lock().await;
    repo.branch(&payload.name).await?;
    let _ = state.tx.send(Event::StatusChanged);
    Ok(Json(json!({ "status": "ok" })).into_response())
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.tx.subscribe();

    while let Ok(event) = rx.recv().await {
        if let Ok(msg) = serde_json::to_string(&event) {
            if socket.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    }
}

// --- Main ---

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Determine repository path from environment variable or command-line argument
    let repo_path = env::var("AURA_REPO_PATH").unwrap_or_else(|_| {
        env::args().nth(1).unwrap_or_else(|| ".".to_string())
    });

    println!("Opening repository at: {}", repo_path);
    let repo = Repository::new(&repo_path);
    
    // Ensure repo is initialized
    let _ = repo.init_repository().await;

    let (tx, _rx) = broadcast::channel(100);

    let state = Arc::new(AppState {
        repo: Arc::new(Mutex::new(repo)),
        tx,
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/repo/status", get(get_status))
        .route("/api/repo/log", get(get_log))
        .route("/api/repo/diff/:hash", get(get_diff))
        .route("/api/repo/commit", post(post_commit))
        .route("/api/repo/branches", get(get_branches).post(post_branch))
        .route("/api/events", get(ws_handler))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("Server running on http://0.0.0.0:3000");

    axum::serve(listener, app).await?;

    Ok(())
}
