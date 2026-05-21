use axum::{
    extract::{
        ws::{Message, WebSocket},
        Query, State, WebSocketUpgrade,
    },
    http::{HeaderValue, StatusCode},
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
    #[serde(rename = "commit")]
    Commit { hash: String },
    #[serde(rename = "checkout")]
    Checkout { target: String },
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
struct LogQuery {
    branch: String,
}

#[derive(Deserialize)]
struct FilesQuery {
    commit: String,
    path: String,
}

#[derive(Deserialize)]
struct DiffQuery {
    commit: String,
}

#[derive(Deserialize)]
struct AddRequest {
    paths: Vec<String>,
}

#[derive(Deserialize)]
struct CommitRequest {
    message: String,
}

#[derive(Deserialize)]
struct CheckoutRequest {
    target: String,
}

#[derive(Deserialize)]
struct BranchRequest {
    name: String,
}

// --- Endpoints ---

async fn get_info(State(state): State<Arc<AppState>>) -> Result<Response, AppError> {
    let repo = state.repo.lock().await;
    let info = repo.get_info()?;
    Ok(Json(info).into_response())
}

async fn get_log(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LogQuery>,
) -> Result<Response, AppError> {
    let repo = state.repo.lock().await;
    let log = repo.get_log(&query.branch)?;
    Ok(Json(log).into_response())
}

async fn get_files(
    State(state): State<Arc<AppState>>,
    Query(query): Query<FilesQuery>,
) -> Result<Response, AppError> {
    let repo = state.repo.lock().await;
    let files = repo.get_files(&query.commit, &query.path)?;
    Ok(Json(files).into_response())
}

async fn get_diff(
    State(state): State<Arc<AppState>>,
    Query(query): Query<DiffQuery>,
) -> Result<Response, AppError> {
    let repo = state.repo.lock().await;
    let diff = repo.get_diff(&query.commit)?;
    Ok(Json(json!({ "diff": diff })).into_response())
}

async fn post_add(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AddRequest>,
) -> Result<Response, AppError> {
    let mut repo = state.repo.lock().await;
    repo.add(&payload.paths)?;
    Ok(Json(json!({ "status": "ok" })).into_response())
}

async fn post_commit(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CommitRequest>,
) -> Result<Response, AppError> {
    let mut repo = state.repo.lock().await;
    let hash = repo.commit(&payload.message)?;

    let _ = state.tx.send(Event::Commit {
        hash: hash.clone(),
    });

    Ok(Json(json!({ "hash": hash })).into_response())
}

async fn post_checkout(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CheckoutRequest>,
) -> Result<Response, AppError> {
    let mut repo = state.repo.lock().await;
    repo.checkout(&payload.target)?;

    let _ = state.tx.send(Event::Checkout {
        target: payload.target.clone(),
    });

    Ok(Json(json!({ "status": "ok" })).into_response())
}

async fn post_branch(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<BranchRequest>,
) -> Result<Response, AppError> {
    let mut repo = state.repo.lock().await;
    repo.create_branch(&payload.name)?;
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
    let repo = Repository::open(&repo_path)?;

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
        .route("/api/repo/info", get(get_info))
        .route("/api/repo/log", get(get_log))
        .route("/api/repo/files", get(get_files))
        .route("/api/repo/diff", get(get_diff))
        .route("/api/repo/add", post(post_add))
        .route("/api/repo/commit", post(post_commit))
        .route("/api/repo/checkout", post(post_checkout))
        .route("/api/repo/branch", post(post_branch))
        .route("/api/events", get(ws_handler))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("Server running on http://0.0.0.0:3000");

    axum::serve(listener, app).await?;

    Ok(())
}
