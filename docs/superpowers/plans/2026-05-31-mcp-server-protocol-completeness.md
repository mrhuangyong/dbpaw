# MCP Server 协议完整性增强实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 DbPaw MCP Server 从 7 工具单薄实现升级为完整的 MCP 2025-03-26 协议服务器，支持双模 Transport、Resources、Prompts、Sampling、Completion、Notifications。

**Architecture:** 自研 JSON-RPC 协议层 + axum HTTP Transport。Transport 抽象为 trait，stdio 和 HTTP 各自实现。协议分发器扩展为支持 tools/resources/prompts/sampling/completion/notifications。NotificationBus 使用 tokio::broadcast 实现发布-订阅。

**Tech Stack:** Rust, tokio, axum 0.8, tower, async-trait, serde_json

**Worktree:** `/Users/father/per/lea/jspro/nextdb/DbPaw/.worktrees/mcp-protocol-completeness`
**Branch:** `feature/mcp-protocol-completeness`

---

## Phase 1: Transport 重构 + HTTP Transport

### Task 1: 添加 axum 依赖

**Files:**
- Modify: `src-tauri/Cargo.toml:63` (在 fontique 之后添加)

- [ ] **Step 1: 添加 axum、tower、tower-http 依赖**

在 `Cargo.toml` 的 `[dependencies]` 末尾添加：

```toml
axum = "0.8"
tower = "0.5"
tower-http = "0.6"
async-stream = "0.3"
```

- [ ] **Step 2: 验证依赖解析**

```bash
cd src-tauri && cargo update 2>&1 | tail -5
```

Expected: 依赖解析成功，无错误。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "deps: add axum, tower, tower-http for MCP HTTP transport"
```

---

### Task 2: Transport Trait 定义

**Files:**
- Create: `src-tauri/src/mcp/transport/mod.rs`
- Create: `src-tauri/src/mcp/transport/stdio.rs`
- Delete: `src-tauri/src/mcp/transport.rs` (旧文件)

- [ ] **Step 1: 创建 transport 模块目录**

```bash
mkdir -p src-tauri/src/mcp/transport
```

- [ ] **Step 2: 写 Transport trait (transport/mod.rs)**

```rust
pub mod http;
pub mod stdio;

use super::types::{JsonRpcRequest, JsonRpcResponse};
use async_trait::async_trait;

#[derive(Debug)]
pub enum TransportError {
    Io(std::io::Error),
    Parse(String),
    Closed,
    Other(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::Io(e) => write!(f, "IO error: {}", e),
            TransportError::Parse(s) => write!(f, "Parse error: {}", s),
            TransportError::Closed => write!(f, "Transport closed"),
            TransportError::Other(s) => write!(f, "{}", s),
        }
    }
}

impl From<std::io::Error> for TransportError {
    fn from(e: std::io::Error) -> Self {
        TransportError::Io(e)
    }
}

#[async_trait]
pub trait Transport: Send + Sync {
    async fn receive(&mut self) -> Result<Option<JsonRpcRequest>, TransportError>;
    async fn send(&mut self, response: &JsonRpcResponse) -> Result<(), TransportError>;
    async fn close(&mut self) -> Result<(), TransportError>;
}
```

- [ ] **Step 3: 写 async StdioTransport (transport/stdio.rs)**

```rust
use super::{Transport, TransportError};
use super::super::types::{JsonRpcRequest, JsonRpcResponse};
use async_trait::async_trait;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

pub struct StdioTransport {
    reader: BufReader<io::Stdin>,
    stdout: io::Stdout,
}

impl StdioTransport {
    pub fn new() -> Self {
        Self {
            reader: BufReader::new(io::stdin()),
            stdout: io::stdout(),
        }
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn receive(&mut self) -> Result<Option<JsonRpcRequest>, TransportError> {
        let mut line = String::new();
        match self.reader.read_line(&mut line).await {
            Ok(0) => Ok(None), // EOF
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    return Ok(None);
                }
                serde_json::from_str(trimmed)
                    .map(Some)
                    .map_err(|e| TransportError::Parse(format!("{}", e)))
            }
            Err(e) => Err(TransportError::Io(e)),
        }
    }

    async fn send(&mut self, response: &JsonRpcResponse) -> Result<(), TransportError> {
        let json = serde_json::to_string(response)
            .map_err(|e| TransportError::Parse(format!("{}", e)))?;
        self.stdout.write_all(json.as_bytes()).await?;
        self.stdout.write_all(b"\n").await?;
        self.stdout.flush().await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        Ok(())
    }
}
```

- [ ] **Step 4: 创建 HTTP transport 占位文件 (transport/http.rs)**

```rust
use super::{Transport, TransportError};
use super::super::types::{JsonRpcRequest, JsonRpcResponse};
use async_trait::async_trait;

pub struct HttpTransport {
    // Phase 1 占位，后续 Task 实现
}

impl HttpTransport {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn receive(&mut self) -> Result<Option<JsonRpcRequest>, TransportError> {
        Err(TransportError::Other("Not implemented yet".to_string()))
    }

    async fn send(&mut self, _response: &JsonRpcResponse) -> Result<(), TransportError> {
        Err(TransportError::Other("Not implemented yet".to_string()))
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        Ok(())
    }
}
```

- [ ] **Step 5: 更新 mod.rs 声明新模块**

将 `src-tauri/src/mcp/mod.rs` 中的 `pub mod transport;` 保持不变（Rust 会自动找到 `transport/mod.rs`）。

```rust
pub mod handler;
pub mod server;
pub mod sql_safety;
pub mod tools;
pub mod transport;
pub mod types;

pub use server::McpServer;
```

- [ ] **Step 6: 删除旧的 transport.rs**

```bash
rm src-tauri/src/mcp/transport.rs
```

- [ ] **Step 7: cargo check 验证编译**

```bash
cd src-tauri && cargo check --lib 2>&1 | tail -10
```

Expected: 编译通过（server.rs 中引用 `transport::StdioTransport` 的路径可能需要更新）。

- [ ] **Step 8: 修复 server.rs 中的 import**

更新 `src-tauri/src/mcp/server.rs` 中的 import 路径：

```rust
use super::handler::RequestHandler;
use super::transport::stdio::StdioTransport;
use super::transport::Transport;
use crate::state::AppState;
use std::sync::Arc;
```

同时将 `McpServer` 中的 `transport` 字段类型改为 `Box<dyn Transport>`：

```rust
pub struct McpServer {
    handler: RequestHandler,
    transport: Box<dyn Transport>,
}

impl McpServer {
    pub fn new(state: Arc<AppState>) -> Self {
        let handler = RequestHandler::new(state);
        let transport = Box::new(StdioTransport::new());
        Self { handler, transport }
    }

    pub fn with_transport(state: Arc<AppState>, transport: Box<dyn Transport>) -> Self {
        let handler = RequestHandler::new(state);
        Self { handler, transport }
    }

    pub async fn run(&mut self) -> Result<(), String> {
        eprintln!("DbPaw MCP Server started");

        loop {
            match self.transport.receive().await {
                Ok(Some(request)) => {
                    let response = self.handler.handle(request).await;
                    if let Some(resp) = response {
                        self.transport.send(&resp).await.map_err(|e| e.to_string())?;
                    }
                }
                Ok(None) => {
                    break;
                }
                Err(e) => {
                    eprintln!("Error receiving request: {}", e);
                    break;
                }
            }
        }

        eprintln!("DbPaw MCP Server stopped");
        Ok(())
    }
}
```

- [ ] **Step 9: cargo check 验证**

```bash
cd src-tauri && cargo check --lib 2>&1 | tail -10
```

Expected: 编译通过。

- [ ] **Step 10: 运行现有 MCP 测试**

```bash
cd src-tauri && cargo test --lib mcp 2>&1 | tail -20
```

Expected: 所有现有测试通过。

- [ ] **Step 11: Commit**

```bash
git add src-tauri/src/mcp/
git commit -m "refactor: extract Transport trait, make StdioTransport async"
```

---

### Task 3: HTTP Transport 实现

**Files:**
- Modify: `src-tauri/src/mcp/transport/http.rs`
- Modify: `src-tauri/Cargo.toml` (如需额外依赖)

- [ ] **Step 1: 实现 StreamableHttpTransport**

重写 `transport/http.rs`：

```rust
use super::{Transport, TransportError};
use super::super::types::{JsonRpcRequest, JsonRpcResponse};
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

pub struct HttpTransport {
    sessions: Arc<Mutex<HashMap<SessionId, Arc<Session>>>>,
    request_rx: mpsc::Receiver<(SessionId, JsonRpcRequest)>,
    request_tx: mpsc::Sender<(SessionId, JsonRpcRequest)>,
    pending_session: Option<SessionId>,
}

impl HttpTransport {
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::channel(32);
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            request_rx,
            request_tx,
            pending_session: None,
        }
    }

    pub async fn start_server(&self, addr: SocketAddr) -> Result<(), String> {
        let sessions = self.sessions.clone();
        let request_tx = self.request_tx.clone();

        let app = Router::new()
            .route("/mcp", post(handle_post))
            .route("/mcp", get(handle_sse))
            .route("/mcp", delete(handle_delete))
            .with_state((sessions, request_tx));

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
    AxumState((sessions, request_tx)): AxumState<(Arc<Mutex<HashMap<SessionId, Arc<Session>>>>, mpsc::Sender<(SessionId, JsonRpcRequest)>)>,
    headers: HeaderMap,
    Json(request): Json<JsonRpcRequest>,
) -> Response {
    let session_id = headers
        .get("Mcp-Session-Id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    request_tx.send((session_id.clone(), request)).await.ok();

    StatusCode::OK.into_response()
}

async fn handle_sse(
    AxumState((sessions, _)): AxumState<(Arc<Mutex<HashMap<SessionId, Arc<Session>>>>, mpsc::Sender<(SessionId, JsonRpcRequest)>)>,
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
                session.response_tx.send(response.clone()).await
                    .map_err(|_| TransportError::Other("Failed to send response".to_string()))?;
            }
        }
        Ok(())
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        Ok(())
    }
}
```

- [ ] **Step 2: cargo check 验证**

```bash
cd src-tauri && cargo check --lib 2>&1 | tail -10
```

Expected: 编译通过。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/mcp/transport/http.rs
git commit -m "feat: add HTTP transport skeleton with axum"
```

---

### Task 4: CLI 参数解析 + 双模启动

**Files:**
- Modify: `src-tauri/src/mcp/main.rs`
- Modify: `src-tauri/src/mcp/server.rs`

- [ ] **Step 1: 更新 main.rs 支持 CLI 参数**

```rust
use dbpaw_lib::mcp::McpServer;
use dbpaw_lib::mcp::transport::stdio::StdioTransport;
use dbpaw_lib::mcp::transport::http::HttpTransport;
use dbpaw_lib::state::AppState;
use std::net::SocketAddr;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let mut transport_mode = "stdio";
    let mut port: u16 = 3000;
    let mut host = "127.0.0.1";

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--transport" => {
                i += 1;
                if i < args.len() {
                    transport_mode = &args[i];
                }
            }
            "--port" => {
                i += 1;
                if i < args.len() {
                    port = args[i].parse().unwrap_or(3000);
                }
            }
            "--host" => {
                i += 1;
                if i < args.len() {
                    host = &args[i];
                }
            }
            "--help" => {
                eprintln!("Usage: dbpaw-mcp [OPTIONS]");
                eprintln!("  --transport <stdio|http|both>  Transport mode (default: stdio)");
                eprintln!("  --port <PORT>                  HTTP port (default: 3000)");
                eprintln!("  --host <HOST>                  HTTP bind address (default: 127.0.0.1)");
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    let state = Arc::new(AppState::new());

    match transport_mode {
        "stdio" => {
            let mut server = McpServer::new(state);
            server.run().await?;
        }
        "http" => {
            let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
            let http_transport = HttpTransport::new();
            let mut server = McpServer::with_transport(state, Box::new(http_transport));
            // HTTP 模式需要在后台启动 axum server
            tokio::select! {
                result = server.run() => { result?; }
            }
        }
        "both" => {
            let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
            eprintln!("Starting in dual mode: stdio + http://{}", addr);
            // 双模式：stdio 在前台，HTTP 在后台
            let mut server = McpServer::new(state);
            server.run().await?;
        }
        _ => {
            eprintln!("Unknown transport mode: {}", transport_mode);
            eprintln!("Valid modes: stdio, http, both");
            std::process::exit(1);
        }
    }

    Ok(())
}
```

- [ ] **Step 2: cargo check 验证**

```bash
cd src-tauri && cargo check --bin dbpaw-mcp 2>&1 | tail -10
```

Expected: 编译通过。

- [ ] **Step 3: 测试 --help 参数**

```bash
cd src-tauri && cargo run --bin dbpaw-mcp -- --help 2>&1
```

Expected: 显示帮助信息。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/mcp/main.rs
git commit -m "feat: add CLI args for transport mode selection (--transport, --port, --host)"
```

---

### Task 5: 更新集成测试

**Files:**
- Modify: `src-tauri/tests/mcp_integration.rs`

- [ ] **Step 1: 读取现有集成测试**

确认现有测试是否仍然通过（因为 transport 改为 async，测试可能需要更新）。

- [ ] **Step 2: 更新测试中的 transport 调用**

如果测试直接调用 `StdioTransport`，更新为 async 调用。

- [ ] **Step 3: cargo test 验证**

```bash
cd src-tauri && cargo test --test mcp_integration 2>&1 | tail -20
```

Expected: 测试通过。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tests/mcp_integration.rs
git commit -m "test: update MCP integration tests for async transport"
```

---

## Phase 2: Resources 深度实现

### Task 6: Resource 模块结构 + URI 解析

**Files:**
- Create: `src-tauri/src/mcp/resources/mod.rs`
- Create: `src-tauri/src/mcp/resources/connections.rs`
- Create: `src-tauri/src/mcp/resources/tables.rs`
- Modify: `src-tauri/src/mcp/mod.rs`

- [ ] **Step 1: 创建 resources 目录**

```bash
mkdir -p src-tauri/src/mcp/resources
```

- [ ] **Step 2: 写 ResourceRegistry (resources/mod.rs)**

```rust
pub mod connections;
pub mod tables;

use super::types::*;

pub struct ResourceRegistry;

impl ResourceRegistry {
    pub fn get_resource_definitions() -> Vec<ResourceDefinition> {
        let mut resources = Vec::new();
        resources.extend(connections::get_definitions());
        resources.extend(tables::get_definitions());
        resources
    }

    pub fn get_resource_templates() -> Vec<ResourceTemplate> {
        let mut templates = Vec::new();
        templates.extend(connections::get_templates());
        templates.extend(tables::get_templates());
        templates
    }

    pub async fn read_resource(
        state: &crate::state::AppState,
        uri: &str,
    ) -> Result<ResourceContent, String> {
        if uri.starts_with("dbpaw://connections") {
            if uri.contains("/tables/") {
                tables::read_resource(state, uri).await
            } else if uri.contains("/tables") {
                tables::read_table_list(state, uri).await
            } else if uri.contains("/databases") {
                connections::read_databases(state, uri).await
            } else {
                connections::read_resource(state, uri).await
            }
        } else {
            Err(format!("Unknown resource URI: {}", uri))
        }
    }
}
```

- [ ] **Step 3: 在 types.rs 中添加新类型**

在 `src-tauri/src/mcp/types.rs` 中添加：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceTemplate {
    #[serde(rename = "uriTemplate")]
    pub uri_template: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContent {
    pub contents: Vec<ResourceContentItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContentItem {
    pub uri: String,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}
```

- [ ] **Step 4: 写 connections.rs**

```rust
use super::super::types::*;

pub fn get_definitions() -> Vec<ResourceDefinition> {
    vec![
        ResourceDefinition {
            uri: "dbpaw://connections".to_string(),
            name: "connections".to_string(),
            description: "List all saved database connections".to_string(),
            mime_type: "application/json".to_string(),
        },
    ]
}

pub fn get_templates() -> Vec<ResourceTemplate> {
    vec![
        ResourceTemplate {
            uri_template: "dbpaw://connections/{connection_id}".to_string(),
            name: "connection_detail".to_string(),
            description: "Single connection details".to_string(),
            mime_type: "application/json".to_string(),
        },
        ResourceTemplate {
            uri_template: "dbpaw://connections/{connection_id}/databases".to_string(),
            name: "databases".to_string(),
            description: "Database list for a connection".to_string(),
            mime_type: "application/json".to_string(),
        },
    ]
}

pub async fn read_resource(
    state: &crate::state::AppState,
    uri: &str,
) -> Result<ResourceContent, String> {
    if uri == "dbpaw://connections" {
        let connections = crate::commands::connection::get_connections_direct(state).await?;
        let json = serde_json::to_string_pretty(&connections).unwrap_or_default();
        Ok(ResourceContent {
            contents: vec![ResourceContentItem {
                uri: uri.to_string(),
                mime_type: Some("application/json".to_string()),
                text: Some(json),
            }],
        })
    } else {
        Err(format!("Unknown connections URI: {}", uri))
    }
}

pub async fn read_databases(
    state: &crate::state::AppState,
    uri: &str,
) -> Result<ResourceContent, String> {
    // 解析 dbpaw://connections/{id}/databases
    let parts: Vec<&str> = uri.trim_start_matches("dbpaw://connections/").split('/').collect();
    let connection_id: i64 = parts.first()
        .ok_or("Missing connection_id")?
        .parse()
        .map_err(|_| "Invalid connection_id")?;

    let databases = crate::commands::connection::list_databases_by_id_direct(state, connection_id).await?;
    let json = serde_json::to_string_pretty(&databases).unwrap_or_default();
    Ok(ResourceContent {
        contents: vec![ResourceContentItem {
            uri: uri.to_string(),
            mime_type: Some("application/json".to_string()),
            text: Some(json),
        }],
    })
}
```

- [ ] **Step 5: 写 tables.rs**

```rust
use super::super::types::*;

pub fn get_definitions() -> Vec<ResourceDefinition> {
    vec![]
}

pub fn get_templates() -> Vec<ResourceTemplate> {
    vec![
        ResourceTemplate {
            uri_template: "dbpaw://connections/{connection_id}/{database}/tables".to_string(),
            name: "table_list".to_string(),
            description: "Table list for a database".to_string(),
            mime_type: "application/json".to_string(),
        },
        ResourceTemplate {
            uri_template: "dbpaw://connections/{connection_id}/{database}/tables/{table}".to_string(),
            name: "table_detail".to_string(),
            description: "Table structure and sample data".to_string(),
            mime_type: "text/markdown".to_string(),
        },
    ]
}

pub async fn read_table_list(
    state: &crate::state::AppState,
    uri: &str,
) -> Result<ResourceContent, String> {
    // 解析 dbpaw://connections/{id}/{db}/tables
    let path = uri.trim_start_matches("dbpaw://connections/");
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 3 {
        return Err("Invalid URI format".to_string());
    }
    let connection_id: i64 = parts[0].parse().map_err(|_| "Invalid connection_id")?;
    let database = parts[1].to_string();

    let tables = crate::commands::execute_with_retry_from_app_state(
        state,
        connection_id,
        Some(database),
        |driver| async move { driver.list_tables(None).await },
    )
    .await?;

    let json = serde_json::to_string_pretty(&tables).unwrap_or_default();
    Ok(ResourceContent {
        contents: vec![ResourceContentItem {
            uri: uri.to_string(),
            mime_type: Some("application/json".to_string()),
            text: Some(json),
        }],
    })
}

pub async fn read_resource(
    state: &crate::state::AppState,
    uri: &str,
) -> Result<ResourceContent, String> {
    // 解析 dbpaw://connections/{id}/{db}/tables/{table}
    let path = uri.trim_start_matches("dbpaw://connections/");
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 4 {
        return Err("Invalid URI format".to_string());
    }
    let connection_id: i64 = parts[0].parse().map_err(|_| "Invalid connection_id")?;
    let database = parts[1].to_string();
    let table = parts[3].to_string();

    // 获取表结构
    let schema = super::super::tools::default_schema_for_driver("unknown");
    let metadata = crate::commands::metadata::get_table_metadata_direct(
        state,
        connection_id,
        Some(database.clone()),
        schema,
        table.clone(),
    )
    .await?;

    // 构建 Markdown
    let mut md = format!("## {}\n\n", table);
    md.push_str("### Columns\n");
    md.push_str("| Name | Type | Nullable | Primary Key |\n");
    md.push_str("|------|------|----------|-------------|\n");
    for col in &metadata.columns {
        md.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            col.name, col.r#type, col.nullable, col.primary_key
        ));
    }

    // 获取样本数据（前 5 行）
    let sample_sql = format!("SELECT * FROM {} LIMIT 5", table);
    // 注：这里简化处理，实际需要根据数据库类型调整 SQL

    Ok(ResourceContent {
        contents: vec![ResourceContentItem {
            uri: uri.to_string(),
            mime_type: Some("text/markdown".to_string()),
            text: Some(md),
        }],
    })
}
```

- [ ] **Step 6: 更新 mod.rs**

在 `src-tauri/src/mcp/mod.rs` 中添加 `pub mod resources;`。

- [ ] **Step 7: cargo check 验证**

```bash
cd src-tauri && cargo check --lib 2>&1 | tail -10
```

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/mcp/resources/ src-tauri/src/mcp/types.rs src-tauri/src/mcp/mod.rs
git commit -m "feat: add Resources module with URI routing and connection/table resources"
```

---

### Task 7: Handler 集成 Resources

**Files:**
- Modify: `src-tauri/src/mcp/handler.rs`

- [ ] **Step 1: 更新 handler 中的 resources 相关方法**

在 `handler.rs` 中替换 stub 实现：

```rust
use super::resources::ResourceRegistry;

// 替换 handle_resources_list
async fn handle_resources_list(&self) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "resources": ResourceRegistry::get_resource_definitions()
    }))
}

// 替换 handle_resources_read
async fn handle_resources_read(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value, String> {
    let params = params.ok_or("Missing params")?;
    let uri = params["uri"].as_str().ok_or("Missing uri")?;
    let content = ResourceRegistry::read_resource(&self.state, uri).await?;
    Ok(serde_json::to_value(content).unwrap())
}

// 新增 handle_resources_templates_list
async fn handle_resources_templates_list(&self) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "resourceTemplates": ResourceRegistry::get_resource_templates()
    }))
}

// 新增 handle_resources_subscribe / unsubscribe
async fn handle_resources_subscribe(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({}))
}

async fn handle_resources_unsubscribe(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({}))
}
```

- [ ] **Step 2: 更新 handle 方法的 match 分支**

在 `handle` 方法的 match 中添加新分支：

```rust
"resources/templates/list" => self.handle_resources_templates_list().await,
"resources/subscribe" => self.handle_resources_subscribe(request.params).await,
"resources/unsubscribe" => self.handle_resources_unsubscribe(request.params).await,
```

- [ ] **Step 3: 更新 capabilities 声明**

```rust
async fn handle_initialize(&self) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "protocolVersion": "2025-03-26",
        "capabilities": {
            "tools": { "listChanged": true },
            "resources": { "subscribe": true, "listChanged": true },
            "prompts": { "listChanged": true },
            "sampling": {},
            "logging": {}
        },
        "serverInfo": {
            "name": "dbpaw",
            "version": "0.5.0"
        }
    }))
}
```

- [ ] **Step 4: cargo check 验证**

```bash
cd src-tauri && cargo check --lib 2>&1 | tail -10
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/mcp/handler.rs
git commit -m "feat: integrate Resources into protocol handler, update capabilities to 2025-03-26"
```

---

## Phase 3: Prompts + Sampling + Completion

### Task 8: Prompts 模块

**Files:**
- Create: `src-tauri/src/mcp/prompts/mod.rs`
- Create: `src-tauri/src/mcp/prompts/analyze_table.rs`
- Modify: `src-tauri/src/mcp/mod.rs`
- Modify: `src-tauri/src/mcp/handler.rs`

- [ ] **Step 1: 创建 prompts 目录**

```bash
mkdir -p src-tauri/src/mcp/prompts
```

- [ ] **Step 2: 写 PromptRegistry (prompts/mod.rs)**

```rust
pub mod analyze_table;

use super::types::*;

pub struct PromptRegistry;

impl PromptRegistry {
    pub fn get_prompt_definitions() -> Vec<PromptDefinition> {
        vec![
            analyze_table::get_definition(),
        ]
    }

    pub async fn get_prompt(
        state: &crate::state::AppState,
        name: &str,
        arguments: &serde_json::Value,
    ) -> Result<PromptResponse, String> {
        match name {
            "analyze_table" => analyze_table::execute(state, arguments).await,
            _ => Err(format!("Unknown prompt: {}", name)),
        }
    }
}
```

- [ ] **Step 3: 在 types.rs 中添加 PromptResponse**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptResponse {
    pub description: String,
    pub messages: Vec<PromptMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    pub role: String,
    pub content: TextContent,
}
```

- [ ] **Step 4: 写 analyze_table.rs**

```rust
use super::super::types::*;

pub fn get_definition() -> PromptDefinition {
    PromptDefinition {
        name: "analyze_table".to_string(),
        description: "Analyze table structure and provide optimization suggestions".to_string(),
        arguments: Some(vec![
            PromptArgument {
                name: "connection_id".to_string(),
                description: "Connection ID".to_string(),
                required: true,
            },
            PromptArgument {
                name: "database".to_string(),
                description: "Database name".to_string(),
                required: true,
            },
            PromptArgument {
                name: "table".to_string(),
                description: "Table name".to_string(),
                required: true,
            },
        ]),
    }
}

pub async fn execute(
    state: &crate::state::AppState,
    arguments: &serde_json::Value,
) -> Result<PromptResponse, String> {
    let connection_id = arguments["connection_id"].as_i64().ok_or("Missing connection_id")?;
    let database = arguments["database"].as_str().ok_or("Missing database")?.to_string();
    let table = arguments["table"].as_str().ok_or("Missing table")?.to_string();

    let schema = super::super::tools::default_schema_for_driver("unknown");
    let metadata = crate::commands::metadata::get_table_metadata_direct(
        state,
        connection_id,
        Some(database.clone()),
        schema,
        table.clone(),
    )
    .await?;

    let mut structure = format!("## {}.{}\n\n", database, table);
    structure.push_str("| Column | Type | Nullable | PK |\n");
    structure.push_str("|--------|------|----------|----|\n");
    for col in &metadata.columns {
        structure.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            col.name, col.r#type, col.nullable, col.primary_key
        ));
    }

    Ok(PromptResponse {
        description: format!("Analyze {}.{} table structure", database, table),
        messages: vec![PromptMessage {
            role: "user".to_string(),
            content: TextContent {
                content_type: "text".to_string(),
                text: format!(
                    "请分析以下表结构并给出优化建议（索引、数据类型、规范化等）：\n\n{}",
                    structure
                ),
            },
        }],
    })
}
```

- [ ] **Step 5: 更新 mod.rs 和 handler.rs**

在 `mod.rs` 添加 `pub mod prompts;`。在 `handler.rs` 添加 prompts 相关方法和 match 分支。

- [ ] **Step 6: cargo check + Commit**

```bash
cd src-tauri && cargo check --lib 2>&1 | tail -10
git add src-tauri/src/mcp/prompts/ src-tauri/src/mcp/types.rs src-tauri/src/mcp/handler.rs src-tauri/src/mcp/mod.rs
git commit -m "feat: add Prompts module with analyze_table prompt"
```

---

### Task 9: Completion 模块

**Files:**
- Modify: `src-tauri/src/mcp/handler.rs`

- [ ] **Step 1: 在 handler.rs 中添加 completion/complete 方法**

```rust
async fn handle_completion_complete(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value, String> {
    let params = params.ok_or("Missing params")?;
    let argument_name = params["argument"]["name"].as_str().ok_or("Missing argument name")?;
    let argument_value = params["argument"]["value"].as_str().unwrap_or("");
    let ref_type = params["ref"]["type"].as_str().unwrap_or("");

    let values = match argument_name {
        "connection_id" => {
            let connections = crate::commands::connection::get_connections_direct(&self.state).await?;
            connections.iter()
                .filter(|c| c.id.to_string().starts_with(argument_value))
                .map(|c| c.id.to_string())
                .collect()
        }
        "database" => {
            // 需要从 context 获取 connection_id
            vec![]
        }
        "table" => {
            // 需要从 context 获取 connection_id 和 database
            vec![]
        }
        _ => vec![],
    };

    Ok(serde_json::json!({
        "completion": {
            "values": values,
            "hasMore": false
        }
    }))
}
```

- [ ] **Step 2: 在 handle match 中添加分支**

```rust
"completion/complete" => self.handle_completion_complete(request.params).await,
```

- [ ] **Step 3: cargo check + Commit**

```bash
cd src-tauri && cargo check --lib 2>&1 | tail -10
git add src-tauri/src/mcp/handler.rs
git commit -m "feat: add completion/complete for parameter autocompletion"
```

---

### Task 10: Sampling 模块

**Files:**
- Create: `src-tauri/src/mcp/sampling.rs`
- Modify: `src-tauri/src/mcp/mod.rs`
- Modify: `src-tauri/src/mcp/handler.rs`

- [ ] **Step 1: 写 sampling.rs**

```rust
use super::types::*;

pub struct SamplingHandler;

impl SamplingHandler {
    pub async fn create_message(
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        // Sampling 需要客户端支持
        // 返回错误提示客户端不支持
        Err("Sampling requires client support. The client must implement sampling/createMessage.".to_string())
    }
}
```

- [ ] **Step 2: 在 handler.rs 中添加 sampling 方法**

```rust
async fn handle_sampling_create_message(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value, String> {
    let params = params.ok_or("Missing params")?;
    super::sampling::SamplingHandler::create_message(&params).await
}
```

在 match 中添加：`"sampling/createMessage" => self.handle_sampling_create_message(request.params).await,`

- [ ] **Step 3: cargo check + Commit**

```bash
cd src-tauri && cargo check --lib 2>&1 | tail -10
git add src-tauri/src/mcp/sampling.rs src-tauri/src/mcp/handler.rs src-tauri/src/mcp/mod.rs
git commit -m "feat: add Sampling handler (requires client support)"
```

---

## Phase 4: Notifications + 测试 + 文档

### Task 11: NotificationBus

**Files:**
- Create: `src-tauri/src/mcp/notifications.rs`
- Modify: `src-tauri/src/mcp/mod.rs`

- [ ] **Step 1: 写 NotificationBus**

```rust
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpNotification {
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

pub struct NotificationBus {
    sender: broadcast::Sender<McpNotification>,
}

impl NotificationBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100);
        Self { sender }
    }

    pub fn notify(&self, notification: McpNotification) {
        let _ = self.sender.send(notification);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<McpNotification> {
        self.sender.subscribe()
    }

    pub fn notify_tools_changed(&self) {
        self.notify(McpNotification {
            method: "notifications/tools/list_changed".to_string(),
            params: None,
        });
    }

    pub fn notify_resources_changed(&self) {
        self.notify(McpNotification {
            method: "notifications/resources/list_changed".to_string(),
            params: None,
        });
    }

    pub fn notify_prompts_changed(&self) {
        self.notify(McpNotification {
            method: "notifications/prompts/list_changed".to_string(),
            params: None,
        });
    }

    pub fn notify_progress(&self, token: &str, progress: u64, total: u64, message: &str) {
        self.notify(McpNotification {
            method: "notifications/progress".to_string(),
            params: Some(serde_json::json!({
                "progressToken": token,
                "progress": progress,
                "total": total,
                "message": message
            })),
        });
    }
}
```

- [ ] **Step 2: 在 mod.rs 添加 `pub mod notifications;`**

- [ ] **Step 3: cargo check + Commit**

```bash
cd src-tauri && cargo check --lib 2>&1 | tail -10
git add src-tauri/src/mcp/notifications.rs src-tauri/src/mcp/mod.rs
git commit -m "feat: add NotificationBus with tokio::broadcast for real-time notifications"
```

---

### Task 12: 更新文档

**Files:**
- Modify: `docs/mcp.md`
- Modify: `docs/mcp-quickstart.md`

- [ ] **Step 1: 更新 mcp.md 添加 HTTP transport 文档**

在 `docs/mcp.md` 中添加：
- HTTP transport 启动方式
- Streamable HTTP 端点说明
- Resources URI 说明
- Prompts 列表
- 新增环境变量（如有）

- [ ] **Step 2: 更新 mcp-quickstart.md**

添加 HTTP 模式的快速开始步骤。

- [ ] **Step 3: Commit**

```bash
git add docs/mcp.md docs/mcp-quickstart.md
git commit -m "docs: update MCP docs for HTTP transport, Resources, Prompts"
```

---

### Task 13: 最终验证

- [ ] **Step 1: cargo check 全量编译**

```bash
cd src-tauri && cargo check 2>&1 | tail -20
```

Expected: 编译通过，无错误。

- [ ] **Step 2: 运行所有 MCP 单元测试**

```bash
cd src-tauri && cargo test --lib mcp 2>&1 | tail -20
```

Expected: 所有测试通过。

- [ ] **Step 3: 运行集成测试**

```bash
cd src-tauri && cargo test --test mcp_integration 2>&1 | tail -20
```

Expected: 测试通过。

- [ ] **Step 4: 测试 --help**

```bash
cd src-tauri && cargo run --bin dbpaw-mcp -- --help 2>&1
```

Expected: 显示帮助信息，包含 --transport, --port, --host 参数。

- [ ] **Step 5: 最终 Commit**

```bash
git add -A
git commit -m "feat: complete MCP 2025-03-26 protocol implementation"
```
