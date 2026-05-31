use super::super::types::{JsonRpcRequest, JsonRpcResponse};
use super::{Transport, TransportError};
use async_trait::async_trait;
use axum::extract::State as AxumState;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

type SessionId = String;

struct Session {
    response_tx: mpsc::Sender<JsonRpcResponse>,
}

struct AppState {
    sessions: Arc<Mutex<HashMap<SessionId, Arc<Session>>>>,
    request_tx: mpsc::Sender<(SessionId, JsonRpcRequest)>,
}

pub struct HttpTransport {
    request_rx: mpsc::Receiver<(SessionId, JsonRpcRequest)>,
    request_tx: mpsc::Sender<(SessionId, JsonRpcRequest)>,
    sessions: Arc<Mutex<HashMap<SessionId, Arc<Session>>>>,
    pending_session: Option<SessionId>,
}

impl HttpTransport {
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::channel(32);
        Self {
            request_rx,
            request_tx,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            pending_session: None,
        }
    }

    pub async fn start_server(&self, addr: SocketAddr) -> Result<(), String> {
        let state = AppState {
            sessions: self.sessions.clone(),
            request_tx: self.request_tx.clone(),
        };
        let shared = Arc::new(state);

        let app = Router::new()
            .route("/mcp", post(handle_post))
            .route("/mcp", get(handle_sse))
            .route("/mcp", delete(handle_delete))
            .with_state(shared);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| format!("Failed to bind: {}", e))?;

        eprintln!("MCP HTTP server listening on {}", addr);

        axum::serve(listener, app)
            .await
            .map_err(|e| format!("Server error: {}", e))
    }
}

async fn handle_post(
    AxumState(state): AxumState<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<JsonRpcRequest>,
) -> Response {
    let session_id = headers
        .get("Mcp-Session-Id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let (resp_tx, mut resp_rx) = mpsc::channel::<JsonRpcResponse>(1);

    {
        let mut sessions = state.sessions.lock().await;
        sessions.insert(session_id.clone(), Arc::new(Session { response_tx: resp_tx }));
    }

    if let Err(_) = state.request_tx.send((session_id, request)).await {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    match resp_rx.recv().await {
        Some(response) => Json(response).into_response(),
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn handle_sse(
    AxumState(_state): AxumState<Arc<AppState>>,
) -> impl IntoResponse {
    Sse::new(async_stream::stream! {
        yield Ok::<Event, std::convert::Infallible>(Event::default().data("connected"))
    })
}

async fn handle_delete() -> impl IntoResponse {
    StatusCode::OK
}

#[async_trait]
impl Transport for HttpTransport {
    async fn receive(&mut self) -> Result<Option<JsonRpcRequest>, TransportError> {
        match self.request_rx.recv().await {
            Some((session_id, request)) => {
                self.pending_session = Some(session_id);
                Ok(Some(request))
            }
            None => Ok(None),
        }
    }

    async fn send(&mut self, response: &JsonRpcResponse) -> Result<(), TransportError> {
        if let Some(session_id) = self.pending_session.take() {
            let sessions = self.sessions.lock().await;
            if let Some(session) = sessions.get(&session_id) {
                session
                    .response_tx
                    .send(response.clone())
                    .await
                    .map_err(|_| TransportError::Other("Failed to send response".to_string()))?;
            }
        }
        Ok(())
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        Ok(())
    }
}
