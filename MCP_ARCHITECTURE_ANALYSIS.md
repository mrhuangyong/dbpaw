# DBX MCP 支持架构深度分析

> 基于源码解析，供学习/二次开发参考。

---

## 1. 总体架构

```
┌─────────────────────────────────────────────────────────────────────┐
│                        AI Agent (Claude / Cursor)                    │
│                              ▲                                       │
│                              │ stdio (JSON-RPC)                       │
│                              ▼                                       │
│                   ┌─────────────────────┐                           │
│                   │  @dbx-app/mcp-server │  ← Node.js 独立进程        │
│                   │   (StdioTransport)   │                           │
│                   └──────────┬──────────┘                           │
│                              │                                       │
│           ┌──────────────────┼──────────────────┐                   │
│           ▼                  ▼                  ▼                   │
│    ┌─────────────┐   ┌──────────────┐   ┌─────────────┐            │
│    │  直连模式    │   │  桌面桥接模式 │   │  Web 后端模式 │            │
│    │ (Node.js驱动)│   │ (Rust HTTP)  │   │ (Axum HTTP) │            │
│    └─────────────┘   └──────────────┘   └─────────────┘            │
└─────────────────────────────────────────────────────────────────────┘
```

**核心设计思想**：
- MCP Server 是一个独立的 Node.js CLI，通过标准输入输出与 AI Agent 通信
- 查询尽可能在 Node.js 层直接完成（PG/MySQL/SQLite），不依赖桌面端
- 不支持直连的数据库，通过本地 HTTP 桥接调用 Rust 核心
- 连接配置从 DBX 本地 SQLite 数据库零配置读取

---

## 2. 代码目录结构

```
packages/
├── mcp-server/
│   ├── src/index.ts          # ← MCP Server 主入口：工具注册、stdio 传输
│   ├── server.json           # MCP 服务器注册清单
│   └── tests/
│
├── node-core/                # ← MCP Server 的底层依赖
│   ├── src/backend.ts        # Backend 工厂：选择直连 / Web 模式
│   ├── src/bridge.ts         # 桌面桥接客户端（HTTP → Tauri）
│   ├── src/connections.ts    # SQLite 连接配置读写 + 密码解密
│   ├── src/database.ts       # 直连查询引擎（pg/mysql2/sqlite3 + 连接池）
│   ├── src/schema-context.ts # Schema 上下文构建（给 AI 的表结构字典）
│   ├── src/sql-safety.ts     # SQL 安全检查（只读/危险词/WHERE强制）
│   ├── src/web-backend.ts    # Web API 后端实现
│   └── src/paths.ts          # 跨平台路径（dbx.db / mcp-bridge-port）
│
└── cli/                      # CLI 工具，与 MCP Server 共享 node-core
    └── src/cli.ts

src-tauri/src/commands/
└── mcp_bridge.rs             # ← Rust 端：TCP HTTP 桥接服务器 + Tauri Event
```

---

## 3. MCP Server 入口详解

### 3.1 启动流程

**文件**：`packages/mcp-server/src/index.ts`

```typescript
// 1. 创建 Backend（根据环境变量选择模式）
const backend = await createBackend();

// 2. 构建 MCP Server 实例
const server = new McpServer({ name: "dbx", version: "0.4.2" });

// 3. 注册 6~8 个 tool
server.tool("dbx_list_connections", ...);
server.tool("dbx_list_tables", ...);
server.tool("dbx_describe_table", ...);
server.tool("dbx_execute_query", ...);
server.tool("dbx_get_schema_context", ...);
server.tool("dbx_add_connection", ...);
server.tool("dbx_remove_connection", ...);

// 桌面端额外注册 2 个 UI 联动工具（仅在 !isWebMode 时）
server.tool("dbx_open_table", ...);
server.tool("dbx_execute_and_show", ...);

// 4. 连接 stdio 传输层
const transport = new StdioServerTransport();
await server.connect(transport);
```

### 3.2 工具参数定义

所有工具参数使用 **Zod** 做运行时校验和类型描述，AI 模型在调用前能看到字段说明：

```typescript
{
  connection_name: z.string().describe("Name of the DBX connection"),
  database: z.string().optional().describe("Database name"),
  schema: z.string().optional().describe("Schema name (default: public for PostgreSQL)"),
}
```

### 3.3 响应格式

统一返回 Markdown 表格文本，方便 AI 阅读：

```typescript
function text(s: string) {
  return { content: [{ type: "text" as const, text: s }] };
}

// 查询结果 → Markdown 表格
`${mdTable(result.columns, rows)}\n\n${result.row_count} row(s)`
```

---

## 4. 三层后端模式实现

### 4.1 Backend 工厂

**文件**：`packages/node-core/src/backend.ts`

```typescript
export async function createBackend(env = process.env): Promise<Backend> {
  if (env.DBX_WEB_URL) {
    return await import("./web-backend.js");   // ← 模式 3：Web API
  }
  // ← 模式 1+2：本地直连 + 桌面桥接混合
  return {
    loadConnections: desktopLoadConnections,
    findConnection: desktopFindConnection,
    ...,
    executeQuery: desktopExecuteQuery,
  };
}
```

### 4.2 模式一：本地直连（Node.js 驱动）

**文件**：`packages/node-core/src/database.ts`

#### 支持的数据库类型

| 类型 | 驱动库 | 连接方式 |
|------|--------|----------|
| PostgreSQL / Redshift | `pg` | Pool (max 3) |
| MySQL / Doris / StarRocks | `mysql2/promise` | Pool (connectionLimit 3) |
| SQLite | `better-sqlite3` | 文件直接打开 |

#### 连接池管理

```typescript
const pools = new Map<string, PoolEntry>();
const IDLE_TIMEOUT_MS = 5 * 60 * 1000;  // 5 分钟空闲自动释放

function poolKey(config: ConnectionConfig): string {
  return `${config.id}:${config.database || ""}`;
}

// 每次取用/归还时重置定时器
function resetIdleTimer(key: string, entry: PoolEntry) {
  clearTimeout(entry.timer);
  entry.timer = setTimeout(() => evictPool(key, entry), IDLE_TIMEOUT_MS);
}
```

#### 代理隧道

如果连接启用了 SOCKS5 或 HTTP 代理，**在 Node.js 层自建本地 TCP 隧道**：

```typescript
// 1. 在 127.0.0.1:0 启动一个 net.Server
const server = createServer((inbound) => {
  connectViaProxy(config).then((outbound) => {
    inbound.pipe(outbound);
    outbound.pipe(inbound);   // 双向透传
  });
});

// 2. 数据库驱动实际连接的是这个本地端口
const endpoint = { host: "127.0.0.1", port: <随机端口> };
```

实现了完整的 **SOCKS5 握手 + 用户名密码认证** 以及 **HTTP CONNECT 代理**。

#### 直连判定逻辑

```typescript
function isDirectType(dbType: string): boolean {
  switch (dbType) {
    case "postgres":
    case "redshift":
    case "mysql":
    case "doris":
    case "starrocks":
    case "sqlite":
      return true;      // ← Node.js 层直接查
    default:
      return false;     // ← 走 Bridge 或 Web
  }
}
```

### 4.3 模式二：桌面桥接（Rust TCP HTTP）

**Node.js 端文件**：`packages/node-core/src/database.ts`（bridgeDataRequest）  
**Rust 端文件**：`src-tauri/src/commands/mcp_bridge.rs`

#### 通信机制

```
Node.js (fetch POST)
  → http://127.0.0.1:<随机端口>/data/execute-query
      ↑
      └─ 端口从 ~/Library/Application Support/com.dbx.app/mcp-bridge-port 读取
```

#### Rust 端启动时写端口文件

```rust
// src-tauri/src/commands/mcp_bridge.rs
let listener = TcpListener::bind("127.0.0.1:0").await?;
let actual_port = listener.local_addr()?.port();

// 写入端口文件，供 Node.js MCP Server 发现
let dir = app_handle.path().app_data_dir()?;
std::fs::write(dir.join("mcp-bridge-port"), actual_port.to_string())?;
```

#### 桥接路由表

| HTTP 路径 | 处理函数 | 说明 |
|-----------|----------|------|
| `POST /open-table` | `handle_open_table` | 触发 Tauri `mcp-open-table` Event |
| `POST /execute-query` | `handle_execute_query` | 触发 Tauri `mcp-execute-query` Event |
| `POST /data/execute-query` | `handle_execute_query_data` | 直接执行 SQL 并返回 JSON |
| `POST /data/list-tables` | `handle_list_tables_data` | 返回表列表 JSON |
| `POST /data/describe-table` | `handle_describe_table_data` | 返回字段定义 JSON |
| `POST /reload-connections` | - | 触发 `mcp-reload-connections` Event |

#### UI 联动实现（Tauri Event）

```rust
// 收到 /open-table 请求
let event = McpOpenTableEvent {
    connection_id: config.id.clone(),
    database: req.database.unwrap_or_default(),
    schema: req.schema,
    table: req.table,
};
app.emit("mcp-open-table", &event)?;   // ← Vue 前端监听此事件自动打开表页
```

### 4.4 模式三：Web 后端（Axum HTTP API）

**文件**：`packages/node-core/src/web-backend.ts`

通过环境变量 `DBX_WEB_URL` 启用：

```typescript
const baseUrl = process.env.DBX_WEB_URL!.replace(/\/+$/, "");
const password = process.env.DBX_WEB_PASSWORD || "";

// 1. 先登录获取 Session Cookie
const res = await fetch(`${baseUrl}/api/auth/login`, {
  method: "POST",
  body: JSON.stringify({ password }),
});

// 2. 后续 API 调用携带 Cookie
// /api/connection/list
// /api/connection/connect
// /api/schema/tables
// /api/schema/columns
// /api/query/execute
```

---

## 5. 连接配置与凭证管理

### 5.1 数据源

**文件**：`packages/node-core/src/connections.ts`

DBX 在本地使用 SQLite 存储所有连接配置：

| 平台 | 路径 |
|------|------|
| macOS | `~/Library/Application Support/com.dbx.app/dbx.db` |
| Linux | `~/.config/com.dbx.app/dbx.db` |
| Windows | `%APPDATA%\com.dbx.app\dbx.db` |

### 5.2 表结构

```sql
-- 连接主表
CREATE TABLE connections (
    id TEXT PRIMARY KEY,
    config_json TEXT        -- 序列化的 ConnectionConfig（不含密码）
);

-- 凭证密文表
CREATE TABLE connection_secrets (
    connection_id TEXT,
    key TEXT,               -- "password" / "proxy_password"
    secret TEXT             -- 明文存储（因为 dbx.db 在本地且受系统权限保护）
);
```

### 5.3 加载流程

```typescript
export async function loadConnections(): Promise<ConnectionConfig[]> {
  const db = openDb(true);   // readonly
  const rows = db.prepare("SELECT id, config_json FROM connections").all();

  for (const row of rows) {
    const config = JSON.parse(row.config_json);
    config.id = row.id;

    // 如果 config_json 里没有密码，从 secrets 表补回
    if (!config.password) {
      config.password = getSecret(db, row.id, "password");
    }
  }
}
```

> **设计要点**：密码不直接存 `config_json`，而是分离到 `connection_secrets`，但两者都在同一个本地 SQLite 中，依赖操作系统文件权限保护。

---

## 6. SQL 安全机制

**文件**：`packages/node-core/src/sql-safety.ts`

### 6.1 四层拦截

```
SQL 输入
   │
   ├──► [Layer 1] 是否为空？ ──► 拒绝
   │
   ├──► [Layer 2] 是否只有 1 条语句？ ──► 拒绝（防 ; 注入多语句）
   │
   ├──► [Layer 3] 是否包含 DROP/TRUNCATE/ALTER？
   │      └──► 是且未设置 ALLOW_DANGEROUS ──► 拒绝
   │
   ├──► [Layer 4] 首关键字是否只读？
   │      └──► 否且未设置 ALLOW_WRITES ──► 拒绝
   │
   └──► [Layer 5] UPDATE/DELETE 是否带 WHERE？
          └──► 否 ──► 拒绝
```

### 6.2 解析实现

不是简单的正则，而是**手写状态机解析 SQL**，能正确处理注释和字符串：

```typescript
function splitSqlStatements(sql: string): string[] {
  // 状态：quote / lineComment / blockComment
  // 按分号分割，但忽略字符串和注释内的分号
}

function stripSqlCommentsAndStrings(sql: string): string {
  // 去掉 -- 行注释、/* */ 块注释、'...' 字符串
  // 然后用正则提取首关键字
}
```

### 6.3 环境变量开关

| 变量 | 作用 |
|------|------|
| `DBX_MCP_ALLOW_WRITES=1` | 允许 INSERT / UPDATE / DELETE |
| `DBX_MCP_ALLOW_DANGEROUS_SQL=1` | 允许 DROP / TRUNCATE / ALTER |

---

## 7. Schema 上下文（AI 写 SQL 的"字典"）

**文件**：`packages/node-core/src/schema-context.ts`

### 7.1 为什么需要

AI 大模型在写 SQL 时经常**虚构表名或字段类型**。Schema Context 给 AI 提供精确的表结构信息。

### 7.2 构建流程

```typescript
export async function buildSchemaContext(backend, config, options) {
  const maxTables = Math.max(1, Math.min(options.maxTables ?? 8, 20));

  // 1. 拉取所有表
  const availableTables = await backend.listTables(config, options.schema);

  // 2. 如果 AI 指定了表名，精确匹配；否则取前 N 个
  const selected = requested.size
    ? availableTables.filter(t => requested.has(t.name.toLowerCase()))
    : availableTables.slice(0, maxTables);

  // 3. 并发拉取每张表的字段定义
  const tables = await Promise.all(
    limited.map(async (table) => ({
      name: table.name,
      type: table.type,
      columns: await backend.describeTable(config, table.name, options.schema),
    }))
  );

  return { connection, database, schema, truncated, tables };
}
```

### 7.3 输出格式

紧凑的 Markdown 格式，包含字段名、类型、是否 NULL、是否主键、注释：

```markdown
Connection: local-pg
Database: mydb
Schema: public

## users
Type: TABLE
- id bigint NOT NULL PK
- email varchar(255) NOT NULL
- created_at timestamp NULL

## orders
Type: TABLE
- order_id bigint NOT NULL PK
- user_id bigint NOT NULL -- FK to users.id
```

---

## 8. 关键数据流时序图

### 8.1 AI 查询 PG 数据库（直连）

```
AI Agent
   │ "dbx_execute_query(connection_name='prod-pg', sql='SELECT * FROM users')"
   ▼
MCP Server (index.ts)
   │ 1. backend.findConnection('prod-pg')
   ▼
connections.ts ──► 读 dbx.db ──► 返回 ConnectionConfig（含密码）
   ▼
database.ts ──► isDirectType('postgres') === true
   │ 2. getPgPool(config) ──► 复用或新建 pg.Pool
   ▼
pg (node-postgres)
   │ 3. pool.query('SELECT * FROM users')
   ▼
PostgreSQL Server
   │ 结果
   ▼
pg ──► database.ts ──► 截断 100 行 ──► 格式化为 Markdown 表格
   ▼
MCP Server ──► StdioTransport ──► AI Agent
```

### 8.2 AI 查询 MongoDB（桥接）

```
AI Agent
   │ "dbx_execute_query(connection_name='prod-mongo', sql='...')"
   ▼
MCP Server (index.ts)
   │ 1. backend.findConnection('prod-mongo')
   ▼
connections.ts ──► 读 dbx.db
   ▼
database.ts ──► isDirectType('mongodb') === false
   │ 2. bridgeDataRequest('/data/execute-query', {...})
   ▼
bridge.ts ──► fetch("http://127.0.0.1:<port>/data/execute-query")
   ▼
mcp_bridge.rs (Tauri Rust)
   │ 3. resolve_connection() ──► 从 AppState 找配置
   ▼
dbx-core (Rust)
   │ 4. dbx_core::query::execute_sql_statement(...)
   ▼
MongoDB Server
   │ 结果
   ▼
dbx-core ──► mcp_bridge.rs ──► JSON HTTP 响应
   ▼
Node.js fetch ──► database.ts ──► Markdown 表格
   ▼
MCP Server ──► AI Agent
```

### 8.3 AI 要求"打开 orders 表"

```
AI Agent
   │ "dbx_open_table(connection_name='prod-pg', table='orders')"
   ▼
MCP Server (index.ts)
   │ 仅桌面端注册此 tool
   ▼
postBridge('/open-table', {...})
   ▼
mcp_bridge.rs
   │ handle_open_table()
   │ app.emit("mcp-open-table", { connection_id, database, schema, table })
   ▼
Vue 3 Frontend (Tauri Event Listener)
   │ 收到 mcp-open-table
   │ router.push('/connection/' + id + '/table/' + table)
   ▼
DBX 桌面端 UI 自动跳转到 orders 表的数据页
```

---

## 9. 二次开发扩展建议

| 扩展点 | 文件 | 说明 |
|--------|------|------|
| **新增 MCP 工具** | `mcp-server/src/index.ts` | 参考现有 `server.tool(...)` 模式 |
| **支持新直连数据库** | `node-core/src/database.ts` | 在 `isDirectType` + `query()` 中添加驱动 |
| **修改 SQL 安全策略** | `node-core/src/sql-safety.ts` | 调整 `READ_KEYWORDS` / `DANGEROUS_KEYWORDS` |
| **修改 Schema 输出格式** | `node-core/src/schema-context.ts` | 调整 `formatSchemaContext()` |
| **新增桥接路由** | `src-tauri/src/commands/mcp_bridge.rs` | 在 `start()` 的 match 中添加新路径 |
| **Web 后端新增 API** | `crates/dbx-web/src/` | Axum Router 中添加对应端点 |

---

## 10. 依赖版本快照

| 组件 | 关键依赖 | 版本 |
|------|----------|------|
| MCP SDK | `@modelcontextprotocol/sdk` | `^1.12.1` |
| 参数校验 | `zod` | `^3.25.20` |
| PG 驱动 | `pg` | `^8.16.0` |
| MySQL 驱动 | `mysql2` | `^3.14.1` |
| SQLite 驱动 | `better-sqlite3` | `^12.9.0` |
| Tauri | `tauri` | `^2.10.3` |
| Web 框架 | `axum` | `^0.8` |

---

*分析完成。如需深入某个具体模块，可继续追问。*
