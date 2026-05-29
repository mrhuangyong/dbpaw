# DynamoDB Datasource Design

日期：2026-05-27

## 概述

为 DbPaw 添加 Amazon DynamoDB 支持，采用 `datasources/` 数据源能力模型（与 Redis、Elasticsearch 相同），而非 `DatabaseDriver` trait。

## 设计决策

### 为什么用 datasources 模式？

- `ADD_NEW_DB.md` 明确规定：非 SQL 数据源不应实现 `DatabaseDriver`
- DynamoDB 是 key-value/document 模型，不适合伪装成表结构
- Redis 模式已有成熟的连接缓存、命令注册、前端组件架构

### 功能范围

**支持的操作：**
- ListTables — 列出所有表
- DescribeTable — 获取表结构（主键、GSI、LSI、属性定义）
- Scan — 全表扫描（支持 filter、limit、分页）
- Query — 按主键/索引查询
- PutItem — 插入/更新单条记录
- DeleteItem — 删除单条记录
- UpdateItem — 更新单条记录

**不支持的操作：**
- CreateTable / DeleteTable — 表管理
- BatchWriteItem — 批量写入
- TransactWriteItems — 事务写入

### 认证方式

- Access Key ID + Secret Key
- 支持 DynamoDB Local（自定义 endpoint URL）

---

## 后端架构

### 核心文件

| 文件 | 职责 |
|------|------|
| `src-tauri/src/datasources/dynamodb.rs` | DynamoDB 连接、数据操作封装 |
| `src-tauri/src/commands/dynamodb.rs` | Tauri command 定义 |

### DynamoDB 数据源能力模型

```rust
pub struct DynamoDbClient {
    client: aws_sdk_dynamodb::Client,
    // 连接配置
}

impl DynamoDbClient {
    pub async fn connect(form: &ConnectionForm) -> Result<Self, String>;
    pub async fn list_tables(&self) -> Result<Vec<String>, String>;
    pub async fn describe_table(&self, table_name: &str) -> Result<TableDescription, String>;
    pub async fn scan(&self, params: ScanParams) -> Result<ScanResult, String>;
    pub async fn query(&self, params: QueryParams) -> Result<QueryResult, String>;
    pub async fn put_item(&self, table_name: &str, item: Item) -> Result<(), String>;
    pub async fn delete_item(&self, table_name: &str, key: Key) -> Result<(), String>;
    pub async fn update_item(&self, params: UpdateParams) -> Result<(), String>;
    pub async fn close(&self);
}
```

### 连接缓存

- 使用 `HashMap<String, DynamoDbClient>` 缓存连接
- 缓存 key 格式：`"{connection_id}"`
- 放置在 `AppState` 中，类似 `redis_cache`

### Tauri Commands

```rust
#[tauri::command]
pub async fn dynamodb_list_tables(state: State<'_, AppState>, id: i64) -> Result<Vec<String>, String>;

#[tauri::command]
pub async fn dynamodb_describe_table(state: State<'_, AppState>, id: i64, table_name: String) -> Result<TableDescription, String>;

#[tauri::command]
pub async fn dynamodb_scan(state: State<'_, AppState>, id: i64, params: ScanParams) -> Result<ScanResult, String>;

#[tauri::command]
pub async fn dynamodb_query(state: State<'_, AppState>, id: i64, params: QueryParams) -> Result<QueryResult, String>;

#[tauri::command]
pub async fn dynamodb_put_item(state: State<'_, AppState>, id: i64, table_name: String, item: Item) -> Result<(), String>;

#[tauri::command]
pub async fn dynamodb_delete_item(state: State<'_, AppState>, id: i64, table_name: String, key: Key) -> Result<(), String>;

#[tauri::command]
pub async fn dynamodb_update_item(state: State<'_, AppState>, id: i64, params: UpdateParams) -> Result<(), String>;
```

### 依赖 (`src-tauri/Cargo.toml`)

```toml
aws-sdk-dynamodb = "1"
aws-config = "1"
aws-credential-types = "1"
```

---

## 前端架构

### 核心文件

| 文件 | 职责 |
|------|------|
| `src/components/business/DynamoDB/DynamoDBBrowserView.tsx` | 主视图容器 |
| `src/components/business/DynamoDB/DynamoDBTableList.tsx` | 左侧表列表 |
| `src/components/business/DynamoDB/DynamoDBItemViewer.tsx` | 右侧数据查看器 |
| `src/components/business/DynamoDB/DynamoDBConsole.tsx` | Scan/Query 控制台 |
| `src/lib/tree-adapters/dynamodb-adapter.tsx` | 侧边栏树结构适配 |

### UI 布局

```
┌─────────────────────────────────────────────────────┐
│ DynamoDB Browser                          [Console] │
├──────────────┬──────────────────────────────────────┤
│ Tables       │ Item Viewer / Scan Results           │
│ ├─ table1    │ ┌──────────────────────────────────┐ │
│ ├─ table2    │ │ { "id": "123", "name": "test" } │ │
│ └─ table3    │ │ { "id": "456", "name": "demo" } │ │
│              │ └──────────────────────────────────┘ │
├──────────────┴──────────────────────────────────────┤
│ Scan/Query Console                                  │
│ [Table: __] [Filter: __] [Limit: 100] [Execute]    │
└─────────────────────────────────────────────────────┘
```

### 数据展示

- Item 以 JSON 格式显示（类似 MongoDB）
- 支持分页（Next/Previous）
- 支持按主键排序

---

## 驱动注册

### `src/lib/driver-registry.tsx`

```typescript
{
  id: "dynamodb",
  label: "DynamoDB",
  kind: "document",
  defaultPort: null,
  isFileBased: false,
  isMysqlFamily: false,
  supportsSSLCA: false,
  supportsSchemaBrowsing: false,
  supportsCreateDatabase: false,
  supportsRoutines: false,
  importCapability: "unsupported",
  icon: () => <Server className="w-4 h-4" />,
  treeConfig: (callbacks) => createDynamoDBTreeConfig(callbacks),
}
```

### 连接表单字段

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `access_key_id` | string | ✓ | AWS Access Key ID |
| `secret_access_key` | string | ✓ | AWS Secret Access Key |
| `region` | string | ✓ | AWS 区域（如 `us-east-1`） |
| `endpoint_url` | string | ✗ | DynamoDB Local 地址 |

---

## 错误处理

| 错误类型 | 说明 |
|----------|------|
| `DYNAMODB_AUTH_ERROR` | 认证失败（Access Key/Secret Key 无效） |
| `DYNAMODB_NETWORK_ERROR` | 网络连接失败 |
| `DYNAMODB_RESOURCE_ERROR` | 表不存在、资源未找到 |
| `DYNAMODB_LIMIT_ERROR` | 速率限制、配额超限 |

复用 `conn_failed_error()` 模式，提供上下文感知的错误提示。

---

## 测试策略

| 测试层级 | 文件 | 说明 |
|----------|------|------|
| 单元测试 | `dynamodb.rs` 内 `#[cfg(test)]` | 连接逻辑、错误处理 |
| 集成测试 | `tests/dynamodb_integration.rs` | 使用 DynamoDB Local 容器 |
| Command 测试 | `tests/dynamodb_command_integration.rs` | Tauri command 层 |

**Docker 容器：** 使用 `amazon/dynamodb-local` 镜像，通过 `testcontainers` 自动启动。

---

## 文件改动汇总

| 文件 | 类型 | 说明 |
|------|------|------|
| `src-tauri/src/datasources/dynamodb.rs` | 新建 | DynamoDB 数据源实现 |
| `src-tauri/src/commands/dynamodb.rs` | 新建 | Tauri commands |
| `src-tauri/src/lib.rs` | 改 | 注册 commands |
| `src-tauri/src/state.rs` | 改 | 添加 dynamodb_cache |
| `src-tauri/Cargo.toml` | 改 | 添加 AWS SDK 依赖 |
| `src/lib/driver-registry.tsx` | 改 | 添加 DynamoDB 驱动配置 |
| `src/services/api.ts` | 改 | 添加 DynamoDB API 封装 |
| `src/services/mocks.ts` | 改 | 添加 DynamoDB mock |
| `src/components/business/DynamoDB/*` | 新建 | 前端组件 |
| `src/lib/tree-adapters/dynamodb-adapter.tsx` | 新建 | 树结构适配 |
| `src-tauri/tests/dynamodb_integration.rs` | 新建 | 集成测试 |
| `src-tauri/tests/dynamodb_command_integration.rs` | 新建 | Command 测试 |
| `src-tauri/tests/common/dynamodb_context.rs` | 新建 | 测试容器配置 |
| `scripts/test-integration.sh` | 改 | 添加 DynamoDB 测试 |
