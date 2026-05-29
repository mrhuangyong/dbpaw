# DynamoDB Datasource Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Amazon DynamoDB support as a datasource (like Redis/Elasticsearch), enabling read/write operations on DynamoDB tables.

**Architecture:** Follow the `datasources/` pattern with a dedicated `DynamoDbClient` in `src-tauri/src/datasources/dynamodb.rs` and Tauri commands in `src-tauri/src/commands/dynamodb.rs`. Frontend uses dedicated components in `src/components/business/DynamoDB/`.

**Tech Stack:** Rust (aws-sdk-dynamodb), TypeScript/React, Tauri v2

---

## File Structure

| File | Responsibility |
|------|----------------|
| `src-tauri/src/datasources/dynamodb.rs` | DynamoDB client, connection logic, data operations |
| `src-tauri/src/commands/dynamodb.rs` | Tauri command definitions |
| `src-tauri/src/state.rs` | Add `dynamodb_cache` field |
| `src-tauri/src/lib.rs` | Register DynamoDB commands |
| `src-tauri/Cargo.toml` | Add AWS SDK dependencies |
| `src/lib/driver-registry.tsx` | Add DynamoDB driver config |
| `src/services/api.ts` | Add DynamoDB API wrappers |
| `src/services/mocks.ts` | Add DynamoDB mock implementations |
| `src/components/business/DynamoDB/DynamoDBBrowserView.tsx` | Main view container |
| `src/components/business/DynamoDB/DynamoDBTableList.tsx` | Table list sidebar |
| `src/components/business/DynamoDB/DynamoDBItemViewer.tsx` | Item viewer |
| `src/components/business/DynamoDB/DynamoDBConsole.tsx` | Scan/Query console |
| `src/lib/tree-adapters/dynamodb-adapter.tsx` | Sidebar tree adapter |
| `src-tauri/tests/common/dynamodb_context.rs` | Test container config |
| `src-tauri/tests/dynamodb_integration.rs` | Integration tests |
| `src-tauri/tests/dynamodb_command_integration.rs` | Command tests |

---

## Task 1: Add AWS SDK Dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Add AWS SDK dependencies to Cargo.toml**

```toml
# Add to [dependencies] section
aws-sdk-dynamodb = "1"
aws-config = "1"
aws-credential-types = "1"
```

- [ ] **Step 2: Run cargo check to verify dependencies**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Dependencies download and compile successfully

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml
git commit -m "feat: add AWS SDK dependencies for DynamoDB"
```

---

## Task 2: Create DynamoDB Datasource Module

**Files:**
- Create: `src-tauri/src/datasources/dynamodb.rs`

- [ ] **Step 1: Create the DynamoDB client module**

```rust
use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;
use aws_sdk_dynamodb::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::models::ConnectionForm;

const DEFAULT_REGION: &str = "us-east-1";

#[derive(Clone)]
pub struct DynamoDbClient {
    client: Client,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DynamoDbTableDescription {
    pub table_name: String,
    pub key_schema: Vec<KeySchemaElement>,
    pub attribute_definitions: Vec<AttributeDefinition>,
    pub global_secondary_indexes: Vec<GlobalSecondaryIndexInfo>,
    pub local_secondary_indexes: Vec<LocalSecondaryIndexInfo>,
    pub item_count: i64,
    pub table_size_bytes: i64,
    pub table_status: String,
    pub creation_date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeySchemaElement {
    pub attribute_name: String,
    pub key_type: String, // "HASH" or "RANGE"
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AttributeDefinition {
    pub attribute_name: String,
    pub attribute_type: String, // "S", "N", "B"
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalSecondaryIndexInfo {
    pub index_name: String,
    pub key_schema: Vec<KeySchemaElement>,
    pub projection_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocalSecondaryIndexInfo {
    pub index_name: String,
    pub key_schema: Vec<KeySchemaElement>,
    pub projection_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanParams {
    pub table_name: String,
    pub filter_expression: Option<String>,
    pub expression_attribute_values: Option<HashMap<String, AttributeValue>>,
    pub limit: Option<i32>,
    pub exclusive_start_key: Option<HashMap<String, AttributeValue>>,
    pub consistent_read: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryParams {
    pub table_name: String,
    pub key_condition_expression: String,
    pub filter_expression: Option<String>,
    pub expression_attribute_values: Option<HashMap<String, AttributeValue>>,
    pub index_name: Option<String>,
    pub limit: Option<i32>,
    pub exclusive_start_key: Option<HashMap<String, AttributeValue>>,
    pub consistent_read: Option<bool>,
    pub scan_forward: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DynamoDbScanResult {
    pub items: Vec<HashMap<String, AttributeValue>>,
    pub count: i32,
    pub last_evaluated_key: Option<HashMap<String, AttributeValue>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateParams {
    pub table_name: String,
    pub key: HashMap<String, AttributeValue>,
    pub update_expression: String,
    pub expression_attribute_values: Option<HashMap<String, AttributeValue>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum AttributeValue {
    S(String),
    N(String),
    B(String),
    BOOL(bool),
    NULL,
    L(Vec<AttributeValue>),
    M(HashMap<String, AttributeValue>),
    SS(Vec<String>),
    NS(Vec<String>),
    BS(Vec<String>),
}

fn normalize_dynamodb_error(e: impl std::fmt::Display) -> String {
    let msg = e.to_string();
    if msg.contains("ResourceNotFoundException") {
        format!("[DYNAMODB_RESOURCE_ERROR] Table not found: {}", msg)
    } else if msg.contains("ValidationException") {
        format!("[DYNAMODB_VALIDATION_ERROR] Invalid request: {}", msg)
    } else if msg.contains("ProvisionedThroughputExceededException") {
        format!("[DYNAMODB_LIMIT_ERROR] Throughput exceeded: {}", msg)
    } else if msg.contains("RequestLimitExceeded") {
        format!("[DYNAMODB_LIMIT_ERROR] Request limit exceeded: {}", msg)
    } else if msg.contains("UnrecognizedClientException")
        || msg.contains("InvalidSignatureException")
        || msg.contains("AccessDeniedException")
    {
        format!("[DYNAMODB_AUTH_ERROR] Authentication failed: {}", msg)
    } else if msg.contains("timeout") || msg.contains("timed out") {
        format!("[DYNAMODB_NETWORK_ERROR] Connection timed out: {}", msg)
    } else if msg.contains("connection refused") || msg.contains("Connection refused") {
        format!("[DYNAMODB_NETWORK_ERROR] Connection refused: {}", msg)
    } else {
        format!("[DYNAMODB_ERROR] {}", msg)
    }
}

impl DynamoDbClient {
    pub async fn connect(form: &ConnectionForm) -> Result<Self, String> {
        let access_key = form
            .username
            .as_deref()
            .ok_or("[VALIDATION_ERROR] Access Key ID is required")?;
        let secret_key = form
            .password
            .as_deref()
            .ok_or("[VALIDATION_ERROR] Secret Access Key is required")?;
        let region = form
            .extra
            .as_ref()
            .and_then(|e| e.get("region"))
            .map(|s| s.as_str())
            .unwrap_or(DEFAULT_REGION);
        let endpoint_url = form
            .extra
            .as_ref()
            .and_then(|e| e.get("endpoint_url"))
            .filter(|s| !s.is_empty())
            .cloned();

        let credentials = Credentials::new(access_key, secret_key, None, None, "dbpaw");

        let mut config_loader = aws_config::defaults(BehaviorVersion::latest())
            .credentials_provider(credentials)
            .region(aws_config::Region::new(region.to_string()));

        if let Some(endpoint) = endpoint_url {
            config_loader = config_loader.endpoint_url(&endpoint);
        }

        let config = config_loader.load().await;
        let client = Client::new(&config);

        Ok(Self { client })
    }

    pub async fn list_tables(&self) -> Result<Vec<String>, String> {
        let mut tables = Vec::new();
        let mut last_evaluated_key = None;

        loop {
            let mut request = self.client.list_tables().limit(100);
            if let Some(key) = last_evaluated_key {
                request = request.exclusive_start_table_name(key);
            }

            let result = request.send().await.map_err(normalize_dynamodb_error)?;
            if let Some(names) = result.table_names() {
                tables.extend(names.iter().map(|s| s.to_string()));
            }

            last_evaluated_key = result
                .last_evaluated_table_name()
                .map(|s| s.to_string());

            if last_evaluated_key.is_none() {
                break;
            }
        }

        Ok(tables)
    }

    pub async fn describe_table(
        &self,
        table_name: &str,
    ) -> Result<DynamoDbTableDescription, String> {
        let result = self
            .client
            .describe_table()
            .table_name(table_name)
            .send()
            .await
            .map_err(normalize_dynamodb_error)?;

        let table = result
            .table()
            .ok_or("[DYNAMODB_ERROR] Table description not found")?;

        let key_schema = table
            .key_schema()
            .unwrap_or_default()
            .iter()
            .map(|k| KeySchemaElement {
                attribute_name: k.attribute_name().unwrap_or_default().to_string(),
                key_type: format!("{:?}", k.key_type().unwrap_or_default()),
            })
            .collect();

        let attribute_definitions = table
            .attribute_definitions()
            .unwrap_or_default()
            .iter()
            .map(|a| AttributeDefinition {
                attribute_name: a.attribute_name().unwrap_or_default().to_string(),
                attribute_type: format!("{:?}", a.attribute_type().unwrap_or_default()),
            })
            .collect();

        let global_secondary_indexes = table
            .global_secondary_indexes()
            .unwrap_or_default()
            .iter()
            .map(|gsi| GlobalSecondaryIndexInfo {
                index_name: gsi.index_name().unwrap_or_default().to_string(),
                key_schema: gsi
                    .key_schema()
                    .unwrap_or_default()
                    .iter()
                    .map(|k| KeySchemaElement {
                        attribute_name: k.attribute_name().unwrap_or_default().to_string(),
                        key_type: format!("{:?}", k.key_type().unwrap_or_default()),
                    })
                    .collect(),
                projection_type: format!(
                    "{:?}",
                    gsi
                        .projection()
                        .and_then(|p| p.projection_type())
                        .unwrap_or_default()
                ),
            })
            .collect();

        let local_secondary_indexes = table
            .local_secondary_indexes()
            .unwrap_or_default()
            .iter()
            .map(|lsi| LocalSecondaryIndexInfo {
                index_name: lsi.index_name().unwrap_or_default().to_string(),
                key_schema: lsi
                    .key_schema()
                    .unwrap_or_default()
                    .iter()
                    .map(|k| KeySchemaElement {
                        attribute_name: k.attribute_name().unwrap_or_default().to_string(),
                        key_type: format!("{:?}", k.key_type().unwrap_or_default()),
                    })
                    .collect(),
                projection_type: format!(
                    "{:?}",
                    lsi
                        .projection()
                        .and_then(|p| p.projection_type())
                        .unwrap_or_default()
                ),
            })
            .collect();

        Ok(DynamoDbTableDescription {
            table_name: table.table_name().unwrap_or_default().to_string(),
            key_schema,
            attribute_definitions,
            global_secondary_indexes,
            local_secondary_indexes,
            item_count: table.item_count().unwrap_or_default(),
            table_size_bytes: table.table_size_bytes().unwrap_or_default(),
            table_status: format!("{:?}", table.table_status().unwrap_or_default()),
            creation_date: table
                .creation_date_string()
                .map(|s| s.to_string()),
        })
    }

    pub async fn scan(&self, params: ScanParams) -> Result<DynamoDbScanResult, String> {
        let mut request = self.client.scan().table_name(&params.table_name);

        if let Some(filter) = &params.filter_expression {
            request = request.filter_expression(filter);
        }

        if let Some(values) = &params.expression_attribute_values {
            let attr_values: HashMap<String, aws_sdk_dynamodb::types::AttributeValue> = values
                .iter()
                .map(|(k, v)| (k.clone(), convert_to_sdk_attribute(v)))
                .collect();
            request = request.set_expression_attribute_values(Some(attr_values));
        }

        if let Some(limit) = params.limit {
            request = request.limit(limit);
        }

        if let Some(key) = &params.exclusive_start_key {
            let start_key: HashMap<String, aws_sdk_dynamodb::types::AttributeValue> = key
                .iter()
                .map(|(k, v)| (k.clone(), convert_to_sdk_attribute(v)))
                .collect();
            request = request.set_exclusive_start_key(Some(start_key));
        }

        if let Some(consistent) = params.consistent_read {
            request = request.consistent_read(consistent);
        }

        let result = request.send().await.map_err(normalize_dynamodb_error)?;

        let items = result
            .items()
            .iter()
            .map(|item| convert_from_sdk_item(item))
            .collect();

        let last_evaluated_key = result
            .last_evaluated_key()
            .map(|key| convert_from_sdk_item(key));

        Ok(DynamoDbScanResult {
            items,
            count: result.count(),
            last_evaluated_key,
        })
    }

    pub async fn query(&self, params: QueryParams) -> Result<DynamoDbScanResult, String> {
        let mut request = self
            .client
            .query()
            .table_name(&params.table_name)
            .key_condition_expression(&params.key_condition_expression);

        if let Some(filter) = &params.filter_expression {
            request = request.filter_expression(filter);
        }

        if let Some(values) = &params.expression_attribute_values {
            let attr_values: HashMap<String, aws_sdk_dynamodb::types::AttributeValue> = values
                .iter()
                .map(|(k, v)| (k.clone(), convert_to_sdk_attribute(v)))
                .collect();
            request = request.set_expression_attribute_values(Some(attr_values));
        }

        if let Some(index) = &params.index_name {
            request = request.index_name(index);
        }

        if let Some(limit) = params.limit {
            request = request.limit(limit);
        }

        if let Some(key) = &params.exclusive_start_key {
            let start_key: HashMap<String, aws_sdk_dynamodb::types::AttributeValue> = key
                .iter()
                .map(|(k, v)| (k.clone(), convert_to_sdk_attribute(v)))
                .collect();
            request = request.set_exclusive_start_key(Some(start_key));
        }

        if let Some(consistent) = params.consistent_read {
            request = request.consistent_read(consistent);
        }

        if let Some(forward) = params.scan_forward {
            request = request.scan_index_forward(forward);
        }

        let result = request.send().await.map_err(normalize_dynamodb_error)?;

        let items = result
            .items()
            .iter()
            .map(|item| convert_from_sdk_item(item))
            .collect();

        let last_evaluated_key = result
            .last_evaluated_key()
            .map(|key| convert_from_sdk_item(key));

        Ok(DynamoDbScanResult {
            items,
            count: result.count(),
            last_evaluated_key,
        })
    }

    pub async fn put_item(
        &self,
        table_name: &str,
        item: HashMap<String, AttributeValue>,
    ) -> Result<(), String> {
        let sdk_item: HashMap<String, aws_sdk_dynamodb::types::AttributeValue> = item
            .into_iter()
            .map(|(k, v)| (k, convert_to_sdk_attribute(&v)))
            .collect();

        self.client
            .put_item()
            .table_name(table_name)
            .set_item(Some(sdk_item))
            .send()
            .await
            .map_err(normalize_dynamodb_error)?;

        Ok(())
    }

    pub async fn delete_item(
        &self,
        table_name: &str,
        key: HashMap<String, AttributeValue>,
    ) -> Result<(), String> {
        let sdk_key: HashMap<String, aws_sdk_dynamodb::types::AttributeValue> = key
            .into_iter()
            .map(|(k, v)| (k, convert_to_sdk_attribute(&v)))
            .collect();

        self.client
            .delete_item()
            .table_name(table_name)
            .set_key(Some(sdk_key))
            .send()
            .await
            .map_err(normalize_dynamodb_error)?;

        Ok(())
    }

    pub async fn update_item(&self, params: UpdateParams) -> Result<(), String> {
        let sdk_key: HashMap<String, aws_sdk_dynamodb::types::AttributeValue> = params
            .key
            .into_iter()
            .map(|(k, v)| (k, convert_to_sdk_attribute(&v)))
            .collect();

        let mut request = self
            .client
            .update_item()
            .table_name(&params.table_name)
            .set_key(Some(sdk_key))
            .update_expression(&params.update_expression);

        if let Some(values) = &params.expression_attribute_values {
            let attr_values: HashMap<String, aws_sdk_dynamodb::types::AttributeValue> = values
                .iter()
                .map(|(k, v)| (k.clone(), convert_to_sdk_attribute(v)))
                .collect();
            request = request.set_expression_attribute_values(Some(attr_values));
        }

        request.send().await.map_err(normalize_dynamodb_error)?;

        Ok(())
    }
}

fn convert_to_sdk_attribute(value: &AttributeValue) -> aws_sdk_dynamodb::types::AttributeValue {
    match value {
        AttributeValue::S(s) => aws_sdk_dynamodb::types::AttributeValue::S(s.clone()),
        AttributeValue::N(n) => aws_sdk_dynamodb::types::AttributeValue::N(n.clone()),
        AttributeValue::B(b) => aws_sdk_dynamodb::types::AttributeValue::B(
            aws_sdk_dynamodb::types::Blob::new(b.as_bytes()),
        ),
        AttributeValue::BOOL(b) => aws_sdk_dynamodb::types::AttributeValue::Bool(*b),
        AttributeValue::NULL => aws_sdk_dynamodb::types::AttributeValue::Null(true),
        AttributeValue::L(list) => {
            let sdk_list: Vec<aws_sdk_dynamodb::types::AttributeValue> =
                list.iter().map(|v| convert_to_sdk_attribute(v)).collect();
            aws_sdk_dynamodb::types::AttributeValue::L(sdk_list)
        }
        AttributeValue::M(map) => {
            let sdk_map: HashMap<String, aws_sdk_dynamodb::types::AttributeValue> = map
                .iter()
                .map(|(k, v)| (k.clone(), convert_to_sdk_attribute(v)))
                .collect();
            aws_sdk_dynamodb::types::AttributeValue::M(sdk_map)
        }
        AttributeValue::SS(ss) => aws_sdk_dynamodb::types::AttributeValue::Ss(ss.clone()),
        AttributeValue::NS(ns) => aws_sdk_dynamodb::types::AttributeValue::Ns(ns.clone()),
        AttributeValue::BS(bs) => {
            let blobs: Vec<aws_sdk_dynamodb::types::Blob> = bs
                .iter()
                .map(|b| aws_sdk_dynamodb::types::Blob::new(b.as_bytes()))
                .collect();
            aws_sdk_dynamodb::types::AttributeValue::Bs(blobs)
        }
    }
}

fn convert_from_sdk_item(
    item: &HashMap<String, aws_sdk_dynamodb::types::AttributeValue>,
) -> HashMap<String, AttributeValue> {
    item.iter()
        .map(|(k, v)| (k.clone(), convert_from_sdk_attribute(v)))
        .collect()
}

fn convert_from_sdk_attribute(value: &aws_sdk_dynamodb::types::AttributeValue) -> AttributeValue {
    match value {
        aws_sdk_dynamodb::types::AttributeValue::S(s) => AttributeValue::S(s.clone()),
        aws_sdk_dynamodb::types::AttributeValue::N(n) => AttributeValue::N(n.clone()),
        aws_sdk_dynamodb::types::AttributeValue::B(b) => {
            AttributeValue::B(String::from_utf8_lossy(b.as_ref()).to_string())
        }
        aws_sdk_dynamodb::types::AttributeValue::Bool(b) => AttributeValue::BOOL(*b),
        aws_sdk_dynamodb::types::AttributeValue::Null(_) => AttributeValue::NULL,
        aws_sdk_dynamodb::types::AttributeValue::L(list) => {
            let converted: Vec<AttributeValue> =
                list.iter().map(|v| convert_from_sdk_attribute(v)).collect();
            AttributeValue::L(converted)
        }
        aws_sdk_dynamodb::types::AttributeValue::M(map) => {
            let converted: HashMap<String, AttributeValue> = map
                .iter()
                .map(|(k, v)| (k.clone(), convert_from_sdk_attribute(v)))
                .collect();
            AttributeValue::M(converted)
        }
        aws_sdk_dynamodb::types::AttributeValue::Ss(ss) => AttributeValue::SS(ss.clone()),
        aws_sdk_dynamodb::types::AttributeValue::Ns(ns) => {
            let converted: Vec<String> = ns.clone();
            AttributeValue::NS(converted)
        }
        aws_sdk_dynamodb::types::AttributeValue::Bs(bs) => {
            let converted: Vec<String> = bs
                .iter()
                .map(|b| String::from_utf8_lossy(b.as_ref()).to_string())
                .collect();
            AttributeValue::BS(converted)
        }
        _ => AttributeValue::NULL,
    }
}
```

- [ ] **Step 2: Run cargo check to verify compilation**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/datasources/dynamodb.rs
git commit -m "feat: add DynamoDB datasource client implementation"
```

---

## Task 3: Update AppState with DynamoDB Cache

**Files:**
- Modify: `src-tauri/src/state.rs`

- [ ] **Step 1: Add DynamoDB cache to AppState**

```rust
use crate::datasources::redis::RedisConnectionCache;
use crate::datasources::dynamodb::DynamoDbClient;
use crate::db::local::LocalDb;
use crate::db::pool_manager::PoolManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AppState {
    pub local_db: Mutex<Option<Arc<LocalDb>>>,
    pub pool_manager: Arc<PoolManager>,
    pub redis_cache: Mutex<RedisConnectionCache>,
    pub dynamodb_cache: Mutex<HashMap<String, DynamoDbClient>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            local_db: Mutex::new(None),
            pool_manager: Arc::new(PoolManager::new()),
            redis_cache: Mutex::new(RedisConnectionCache::new()),
            dynamodb_cache: Mutex::new(HashMap::new()),
        }
    }
}
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/state.rs
git commit -m "feat: add DynamoDB connection cache to AppState"
```

---

## Task 4: Add Datasources Module Declaration

**Files:**
- Modify: `src-tauri/src/datasources/mod.rs`

- [ ] **Step 1: Add dynamodb module declaration**

Check the current `mod.rs` file and add:

```rust
pub mod dynamodb;
pub mod elasticsearch;
pub mod redis;
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/datasources/mod.rs
git commit -m "feat: register DynamoDB datasource module"
```

---

## Task 5: Create DynamoDB Tauri Commands

**Files:**
- Create: `src-tauri/src/commands/dynamodb.rs`

- [ ] **Step 1: Create the commands file**

```rust
use crate::datasources::dynamodb::{
    AttributeValue, DynamoDbClient, DynamoDbScanResult, DynamoDbTableDescription, QueryParams,
    ScanParams, UpdateParams,
};
use crate::models::ConnectionForm;
use crate::state::AppState;
use std::collections::HashMap;
use tauri::State;

async fn get_or_create_client(
    state: &State<'_, AppState>,
    id: i64,
) -> Result<DynamoDbClient, String> {
    let cache_key = id.to_string();

    // Fast path: return cached client
    {
        let cache = state.dynamodb_cache.lock().await;
        if let Some(client) = cache.get(&cache_key) {
            return Ok(client.clone());
        }
    }

    // Slow path: create new client
    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };
    let db = local_db.ok_or("Local DB not initialized")?;
    let form = db.get_connection_form_by_id(id).await?;

    if form.driver != "dynamodb" {
        return Err(format!(
            "[UNSUPPORTED] Connection {} is not a DynamoDB connection",
            id
        ));
    }

    let client = DynamoDbClient::connect(&form).await?;

    // Cache the client
    {
        let mut cache = state.dynamodb_cache.lock().await;
        cache.insert(cache_key, client.clone());
    }

    Ok(client)
}

#[tauri::command]
pub async fn dynamodb_test_connection(
    state: State<'_, AppState>,
    id: i64,
) -> Result<String, String> {
    let client = get_or_create_client(&state, id).await?;
    client.list_tables().await?;
    Ok("Connection successful".to_string())
}

#[tauri::command]
pub async fn dynamodb_test_connection_ephemeral(
    form: ConnectionForm,
) -> Result<String, String> {
    let client = DynamoDbClient::connect(&form).await?;
    client.list_tables().await?;
    Ok("Connection successful".to_string())
}

#[tauri::command]
pub async fn dynamodb_list_tables(
    state: State<'_, AppState>,
    id: i64,
) -> Result<Vec<String>, String> {
    let client = get_or_create_client(&state, id).await?;
    client.list_tables().await
}

#[tauri::command]
pub async fn dynamodb_describe_table(
    state: State<'_, AppState>,
    id: i64,
    table_name: String,
) -> Result<DynamoDbTableDescription, String> {
    let client = get_or_create_client(&state, id).await?;
    client.describe_table(&table_name).await
}

#[tauri::command]
pub async fn dynamodb_scan(
    state: State<'_, AppState>,
    id: i64,
    table_name: String,
    filter_expression: Option<String>,
    expression_attribute_values: Option<HashMap<String, AttributeValue>>,
    limit: Option<i32>,
    exclusive_start_key: Option<HashMap<String, AttributeValue>>,
    consistent_read: Option<bool>,
) -> Result<DynamoDbScanResult, String> {
    let client = get_or_create_client(&state, id).await?;
    let params = ScanParams {
        table_name,
        filter_expression,
        expression_attribute_values,
        limit,
        exclusive_start_key,
        consistent_read,
    };
    client.scan(params).await
}

#[tauri::command]
pub async fn dynamodb_query(
    state: State<'_, AppState>,
    id: i64,
    table_name: String,
    key_condition_expression: String,
    filter_expression: Option<String>,
    expression_attribute_values: Option<HashMap<String, AttributeValue>>,
    index_name: Option<String>,
    limit: Option<i32>,
    exclusive_start_key: Option<HashMap<String, AttributeValue>>,
    consistent_read: Option<bool>,
    scan_forward: Option<bool>,
) -> Result<DynamoDbScanResult, String> {
    let client = get_or_create_client(&state, id).await?;
    let params = QueryParams {
        table_name,
        key_condition_expression,
        filter_expression,
        expression_attribute_values,
        index_name,
        limit,
        exclusive_start_key,
        consistent_read,
        scan_forward,
    };
    client.query(params).await
}

#[tauri::command]
pub async fn dynamodb_put_item(
    state: State<'_, AppState>,
    id: i64,
    table_name: String,
    item: HashMap<String, AttributeValue>,
) -> Result<(), String> {
    let client = get_or_create_client(&state, id).await?;
    client.put_item(&table_name, item).await
}

#[tauri::command]
pub async fn dynamodb_delete_item(
    state: State<'_, AppState>,
    id: i64,
    table_name: String,
    key: HashMap<String, AttributeValue>,
) -> Result<(), String> {
    let client = get_or_create_client(&state, id).await?;
    client.delete_item(&table_name, key).await
}

#[tauri::command]
pub async fn dynamodb_update_item(
    state: State<'_, AppState>,
    id: i64,
    table_name: String,
    key: HashMap<String, AttributeValue>,
    update_expression: String,
    expression_attribute_values: Option<HashMap<String, AttributeValue>>,
) -> Result<(), String> {
    let client = get_or_create_client(&state, id).await?;
    let params = UpdateParams {
        table_name,
        key,
        update_expression,
        expression_attribute_values,
    };
    client.update_item(params).await
}
```

- [ ] **Step 2: Register commands in lib.rs**

Add to the `invoke_handler` in `src-tauri/src/lib.rs`:

```rust
commands::dynamodb::dynamodb_test_connection,
commands::dynamodb::dynamodb_test_connection_ephemeral,
commands::dynamodb::dynamodb_list_tables,
commands::dynamodb::dynamodb_describe_table,
commands::dynamodb::dynamodb_scan,
commands::dynamodb::dynamodb_query,
commands::dynamodb::dynamodb_put_item,
commands::dynamodb::dynamodb_delete_item,
commands::dynamodb::dynamodb_update_item,
```

- [ ] **Step 3: Add commands module declaration**

In `src-tauri/src/commands/mod.rs`, add:

```rust
pub mod dynamodb;
```

- [ ] **Step 4: Run cargo check**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/dynamodb.rs src-tauri/src/lib.rs src-tauri/src/commands/mod.rs
git commit -m "feat: add DynamoDB Tauri commands"
```

---

## Task 6: Add DynamoDB to Frontend Driver Registry

**Files:**
- Modify: `src/lib/driver-registry.tsx`

- [ ] **Step 1: Add DynamoDB to DRIVER_IDS**

```typescript
const DRIVER_IDS = [
  "postgres",
  "mysql",
  "mariadb",
  "tidb",
  "starrocks",
  "doris",
  "sqlite",
  "duckdb",
  "clickhouse",
  "mssql",
  "oracle",
  "db2",
  "redis",
  "elasticsearch",
  "mongodb",
  "cassandra",
  "dynamodb",
] as const;
```

- [ ] **Step 2: Add DynamoDB configuration to DRIVER_REGISTRY**

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
},
```

- [ ] **Step 3: Add DynamoDB tree adapter import**

```typescript
import { createDynamoDBTreeConfig } from "./tree-adapters/dynamodb-adapter.tsx";
```

- [ ] **Step 4: Run typecheck**

Run: `bun run typecheck`
Expected: TypeScript compiles successfully

- [ ] **Step 5: Commit**

```bash
git add src/lib/driver-registry.tsx
git commit -m "feat: add DynamoDB driver to frontend registry"
```

---

## Task 7: Create DynamoDB Tree Adapter

**Files:**
- Create: `src/lib/tree-adapters/dynamodb-adapter.tsx`

- [ ] **Step 1: Create the tree adapter**

```tsx
import type { TreeConfig, TreeCallbacks } from "./types.tsx";
import { Database, Table } from "lucide-react";

export function createDynamoDBTreeConfig(
  callbacks: TreeCallbacks,
): TreeConfig {
  return {
    getRootNodes: async (connectionId: string) => {
      const tables = await window.__TAURI__.invoke("dynamodb_list_tables", {
        id: parseInt(connectionId),
      });
      return tables.map((name: string) => ({
        id: name,
        label: name,
        type: "table" as const,
        icon: <Table className="w-4 h-4" />,
        data: { connectionId, tableName: name },
      }));
    },

    getChildren: async (nodeId: string, data: any) => {
      return [];
    },

    onNodeClick: (nodeId: string, data: any) => {
      callbacks.onTableSelect?.(data.connectionId, data.tableName);
    },
  };
}
```

- [ ] **Step 2: Run typecheck**

Run: `bun run typecheck`
Expected: TypeScript compiles successfully

- [ ] **Step 3: Commit**

```bash
git add src/lib/tree-adapters/dynamodb-adapter.tsx
git commit -m "feat: add DynamoDB tree adapter for sidebar"
```

---

## Task 8: Add DynamoDB API Wrappers

**Files:**
- Modify: `src/services/api.ts`

- [ ] **Step 1: Add DynamoDB API methods**

```typescript
export const api = {
  // ... existing methods ...

  dynamodb: {
    testConnection: (id: number) =>
      invoke<string>("dynamodb_test_connection", { id }),

    testConnectionEphemeral: (form: ConnectionForm) =>
      invoke<string>("dynamodb_test_connection_ephemeral", { form }),

    listTables: (id: number) =>
      invoke<string[]>("dynamodb_list_tables", { id }),

    describeTable: (id: number, tableName: string) =>
      invoke<DynamoDbTableDescription>("dynamodb_describe_table", { id, tableName }),

    scan: (id: number, params: DynamoDbScanParams) =>
      invoke<DynamoDbScanResult>("dynamodb_scan", { id, ...params }),

    query: (id: number, params: DynamoDbQueryParams) =>
      invoke<DynamoDbScanResult>("dynamodb_query", { id, ...params }),

    putItem: (id: number, tableName: string, item: Record<string, DynamoDbAttributeValue>) =>
      invoke<void>("dynamodb_put_item", { id, tableName, item }),

    deleteItem: (id: number, tableName: string, key: Record<string, DynamoDbAttributeValue>) =>
      invoke<void>("dynamodb_delete_item", { id, tableName, key }),

    updateItem: (id: number, params: DynamoDbUpdateParams) =>
      invoke<void>("dynamodb_update_item", { id, ...params }),
  },
};

// Add types
export interface DynamoDbAttributeValue {
  type: "S" | "N" | "B" | "BOOL" | "NULL" | "L" | "M" | "SS" | "NS" | "BS";
  value: any;
}

export interface DynamoDbTableDescription {
  tableName: string;
  keySchema: Array<{ attributeName: string; keyType: string }>;
  attributeDefinitions: Array<{ attributeName: string; attributeType: string }>;
  globalSecondaryIndexes: Array<{
    indexName: string;
    keySchema: Array<{ attributeName: string; keyType: string }>;
    projectionType: string;
  }>;
  localSecondaryIndexes: Array<{
    indexName: string;
    keySchema: Array<{ attributeName: string; keyType: string }>;
    projectionType: string;
  }>;
  itemCount: number;
  tableSizeBytes: number;
  tableStatus: string;
  creationDate?: string;
}

export interface DynamoDbScanParams {
  tableName: string;
  filterExpression?: string;
  expressionAttributeValues?: Record<string, DynamoDbAttributeValue>;
  limit?: number;
  exclusiveStartKey?: Record<string, DynamoDbAttributeValue>;
  consistentRead?: boolean;
}

export interface DynamoDbQueryParams {
  tableName: string;
  keyConditionExpression: string;
  filterExpression?: string;
  expressionAttributeValues?: Record<string, DynamoDbAttributeValue>;
  indexName?: string;
  limit?: number;
  exclusiveStartKey?: Record<string, DynamoDbAttributeValue>;
  consistentRead?: boolean;
  scanForward?: boolean;
}

export interface DynamoDbScanResult {
  items: Array<Record<string, DynamoDbAttributeValue>>;
  count: number;
  lastEvaluatedKey?: Record<string, DynamoDbAttributeValue>;
}

export interface DynamoDbUpdateParams {
  tableName: string;
  key: Record<string, DynamoDbAttributeValue>;
  updateExpression: string;
  expressionAttributeValues?: Record<string, DynamoDbAttributeValue>;
}
```

- [ ] **Step 2: Run typecheck**

Run: `bun run typecheck`
Expected: TypeScript compiles successfully

- [ ] **Step 3: Commit**

```bash
git add src/services/api.ts
git commit -m "feat: add DynamoDB API wrappers"
```

---

## Task 9: Add DynamoDB Mock Implementations

**Files:**
- Modify: `src/services/mocks.ts`

- [ ] **Step 1: Add DynamoDB mock methods**

```typescript
export const mockApi = {
  // ... existing methods ...

  dynamodb: {
    testConnection: async () => "Connection successful",
    testConnectionEphemeral: async () => "Connection successful",
    listTables: async () => ["Users", "Orders", "Products"],
    describeTable: async () => ({
      tableName: "Users",
      keySchema: [
        { attributeName: "PK", keyType: "HASH" },
        { attributeName: "SK", keyType: "RANGE" },
      ],
      attributeDefinitions: [
        { attributeName: "PK", attributeType: "S" },
        { attributeName: "SK", attributeType: "S" },
      ],
      globalSecondaryIndexes: [],
      localSecondaryIndexes: [],
      itemCount: 100,
      tableSizeBytes: 10240,
      tableStatus: "ACTIVE",
      creationDate: "2026-01-01T00:00:00Z",
    }),
    scan: async () => ({
      items: [
        { PK: { type: "S", value: "USER#1" }, SK: { type: "S", value: "PROFILE" }, name: { type: "S", value: "John Doe" } },
        { PK: { type: "S", value: "USER#2" }, SK: { type: "S", value: "PROFILE" }, name: { type: "S", value: "Jane Doe" } },
      ],
      count: 2,
    }),
    query: async () => ({
      items: [
        { PK: { type: "S", value: "USER#1" }, SK: { type: "S", value: "PROFILE" }, name: { type: "S", value: "John Doe" } },
      ],
      count: 1,
    }),
    putItem: async () => {},
    deleteItem: async () => {},
    updateItem: async () => {},
  },
};
```

- [ ] **Step 2: Run typecheck**

Run: `bun run typecheck`
Expected: TypeScript compiles successfully

- [ ] **Step 3: Commit**

```bash
git add src/services/mocks.ts
git commit -m "feat: add DynamoDB mock implementations"
```

---

## Task 10: Create DynamoDB Frontend Components

**Files:**
- Create: `src/components/business/DynamoDB/DynamoDBBrowserView.tsx`
- Create: `src/components/business/DynamoDB/DynamoDBTableList.tsx`
- Create: `src/components/business/DynamoDB/DynamoDBItemViewer.tsx`
- Create: `src/components/business/DynamoDB/DynamoDBConsole.tsx`

- [ ] **Step 1: Create DynamoDBBrowserView.tsx**

```tsx
import { useState, useEffect } from "react";
import { api } from "@/services/api";
import { DynamoDBTableList } from "./DynamoDBTableList";
import { DynamoDBItemViewer } from "./DynamoDBItemViewer";
import { DynamoDBConsole } from "./DynamoDBConsole";

interface Props {
  connectionId: number;
}

export function DynamoDBBrowserView({ connectionId }: Props) {
  const [tables, setTables] = useState<string[]>([]);
  const [selectedTable, setSelectedTable] = useState<string | null>(null);
  const [items, setItems] = useState<Array<Record<string, any>>>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    loadTables();
  }, [connectionId]);

  const loadTables = async () => {
    try {
      const tableList = await api.dynamodb.listTables(connectionId);
      setTables(tableList);
    } catch (error) {
      console.error("Failed to load tables:", error);
    }
  };

  const handleTableSelect = (tableName: string) => {
    setSelectedTable(tableName);
    loadItems(tableName);
  };

  const loadItems = async (tableName: string) => {
    setLoading(true);
    try {
      const result = await api.dynamodb.scan(connectionId, {
        tableName,
        limit: 100,
      });
      setItems(result.items);
    } catch (error) {
      console.error("Failed to load items:", error);
    } finally {
      setLoading(false);
    }
  };

  const handleScan = async (params: any) => {
    setLoading(true);
    try {
      const result = await api.dynamodb.scan(connectionId, params);
      setItems(result.items);
    } catch (error) {
      console.error("Scan failed:", error);
    } finally {
      setLoading(false);
    }
  };

  const handleQuery = async (params: any) => {
    setLoading(true);
    try {
      const result = await api.dynamodb.query(connectionId, params);
      setItems(result.items);
    } catch (error) {
      console.error("Query failed:", error);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex h-full">
      <div className="w-64 border-r">
        <DynamoDBTableList
          tables={tables}
          selectedTable={selectedTable}
          onTableSelect={handleTableSelect}
        />
      </div>
      <div className="flex-1 flex flex-col">
        <div className="flex-1 overflow-auto p-4">
          <DynamoDBItemViewer items={items} loading={loading} />
        </div>
        <div className="border-t p-4">
          <DynamoDBConsole
            selectedTable={selectedTable}
            onScan={handleScan}
            onQuery={handleQuery}
          />
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Create DynamoDBTableList.tsx**

```tsx
import { Table } from "lucide-react";

interface Props {
  tables: string[];
  selectedTable: string | null;
  onTableSelect: (tableName: string) => void;
}

export function DynamoDBTableList({ tables, selectedTable, onTableSelect }: Props) {
  return (
    <div className="p-2">
      <h3 className="text-sm font-semibold mb-2 px-2">Tables</h3>
      <div className="space-y-1">
        {tables.map((table) => (
          <button
            key={table}
            className={`w-full flex items-center gap-2 px-2 py-1.5 rounded text-sm ${
              selectedTable === table
                ? "bg-accent text-accent-foreground"
                : "hover:bg-muted"
            }`}
            onClick={() => onTableSelect(table)}
          >
            <Table className="w-4 h-4" />
            {table}
          </button>
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Create DynamoDBItemViewer.tsx**

```tsx
import { Loader2 } from "lucide-react";

interface Props {
  items: Array<Record<string, any>>;
  loading: boolean;
}

export function DynamoDBItemViewer({ items, loading }: Props) {
  if (loading) {
    return (
      <div className="flex items-center justify-center h-32">
        <Loader2 className="w-6 h-6 animate-spin" />
      </div>
    );
  }

  if (items.length === 0) {
    return (
      <div className="text-center text-muted-foreground py-8">
        No items found
      </div>
    );
  }

  return (
    <div className="space-y-2">
      {items.map((item, index) => (
        <div key={index} className="border rounded p-3">
          <pre className="text-sm overflow-auto">
            {JSON.stringify(item, null, 2)}
          </pre>
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 4: Create DynamoDBConsole.tsx**

```tsx
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

interface Props {
  selectedTable: string | null;
  onScan: (params: any) => void;
  onQuery: (params: any) => void;
}

export function DynamoDBConsole({ selectedTable, onScan, onQuery }: Props) {
  const [mode, setMode] = useState<"scan" | "query">("scan");
  const [filterExpression, setFilterExpression] = useState("");
  const [keyCondition, setKeyCondition] = useState("");
  const [limit, setLimit] = useState("100");

  const handleExecute = () => {
    if (!selectedTable) return;

    const params: any = {
      tableName: selectedTable,
      limit: parseInt(limit) || 100,
    };

    if (filterExpression) {
      params.filterExpression = filterExpression;
    }

    if (mode === "query") {
      if (!keyCondition) return;
      params.keyConditionExpression = keyCondition;
      onQuery(params);
    } else {
      onScan(params);
    }
  };

  return (
    <div className="space-y-2">
      <div className="flex gap-2">
        <Button
          variant={mode === "scan" ? "default" : "outline"}
          size="sm"
          onClick={() => setMode("scan")}
        >
          Scan
        </Button>
        <Button
          variant={mode === "query" ? "default" : "outline"}
          size="sm"
          onClick={() => setMode("query")}
        >
          Query
        </Button>
      </div>

      {mode === "query" && (
        <Input
          placeholder="Key condition expression (e.g., PK = :pk)"
          value={keyCondition}
          onChange={(e) => setKeyCondition(e.target.value)}
        />
      )}

      <Input
        placeholder="Filter expression (optional)"
        value={filterExpression}
        onChange={(e) => setFilterExpression(e.target.value)}
      />

      <div className="flex gap-2">
        <Input
          placeholder="Limit"
          value={limit}
          onChange={(e) => setLimit(e.target.value)}
          type="number"
          className="w-24"
        />
        <Button onClick={handleExecute} disabled={!selectedTable}>
          Execute
        </Button>
      </div>
    </div>
  );
}
```

- [ ] **Step 5: Run typecheck**

Run: `bun run typecheck`
Expected: TypeScript compiles successfully

- [ ] **Step 6: Commit**

```bash
git add src/components/business/DynamoDB/
git commit -m "feat: add DynamoDB frontend components"
```

---

## Task 11: Add DynamoDB Connection Form Support

**Files:**
- Modify: `src/lib/connection-form/rules.ts`
- Modify: `src/components/business/ConnectionForm.tsx`

- [ ] **Step 1: Add DynamoDB to connection form rules**

Add DynamoDB-specific fields to the connection form:

```typescript
// In rules.ts, add DynamoDB to the driver list for custom fields
export const DYNAMODB_FIELDS = ["access_key_id", "secret_access_key", "region", "endpoint_url"];
```

- [ ] **Step 2: Update ConnectionForm to handle DynamoDB fields**

Add conditional rendering for DynamoDB-specific fields when `driver === "dynamodb"`.

- [ ] **Step 3: Run typecheck**

Run: `bun run typecheck`
Expected: TypeScript compiles successfully

- [ ] **Step 4: Commit**

```bash
git add src/lib/connection-form/rules.ts src/components/business/ConnectionForm.tsx
git commit -m "feat: add DynamoDB connection form fields"
```

---

## Task 12: Create Integration Tests

**Files:**
- Create: `src-tauri/tests/common/dynamodb_context.rs`
- Create: `src-tauri/tests/dynamodb_integration.rs`
- Create: `src-tauri/tests/dynamodb_command_integration.rs`

- [ ] **Step 1: Create test container context**

```rust
// src-tauri/tests/common/dynamodb_context.rs
use testcontainers::core::WaitFor;
use testcontainers::{GenericImage, Image};

#[derive(Debug)]
pub struct DynamoDb;

impl Image for DynamoDb {
    fn name(&self) -> &str {
        "amazon/dynamodb-local"
    }

    fn tag(&self) -> &str {
        "latest"
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::message_on_stdout("Initializing DynamoDB Local")]
    }
}

impl DynamoDb {
    pub fn endpoint(&self) -> String {
        "http://localhost:8000".to_string()
    }
}
```

- [ ] **Step 2: Add to common/mod.rs**

```rust
pub mod dynamodb_context;
```

- [ ] **Step 3: Create integration test**

```rust
// src-tauri/tests/dynamodb_integration.rs
use dbpaw::datasources::dynamodb::{AttributeValue, DynamoDbClient, ScanParams};
use dbpaw::models::ConnectionForm;
use std::collections::HashMap;

#[tokio::test]
#[ignore]
async fn test_dynamodb_connection() {
    // This test requires DynamoDB Local running
    let form = ConnectionForm {
        driver: "dynamodb".to_string(),
        username: Some("test".to_string()),
        password: Some("test".to_string()),
        extra: Some(HashMap::from([
            ("region".to_string(), "us-east-1".to_string()),
            ("endpoint_url".to_string(), "http://localhost:8000".to_string()),
        ])),
        ..Default::default()
    };

    let client = DynamoDbClient::connect(&form).await.unwrap();
    let tables = client.list_tables().await.unwrap();
    assert!(tables.is_empty() || !tables.is_empty()); // Just verify it doesn't error
}
```

- [ ] **Step 4: Update test-integration.sh**

Add DynamoDB test configuration to `scripts/test-integration.sh`.

- [ ] **Step 5: Run cargo check**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles successfully

- [ ] **Step 6: Commit**

```bash
git add src-tauri/tests/common/dynamodb_context.rs src-tauri/tests/common/mod.rs src-tauri/tests/dynamodb_integration.rs scripts/test-integration.sh
git commit -m "feat: add DynamoDB integration tests"
```

---

## Task 13: Final Verification

- [ ] **Step 1: Run full typecheck**

Run: `bun run typecheck`
Expected: No TypeScript errors

- [ ] **Step 2: Run lint**

Run: `bun run lint`
Expected: No lint errors

- [ ] **Step 3: Run cargo check**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: No Rust compilation errors

- [ ] **Step 4: Run smoke tests**

Run: `bun run test:smoke`
Expected: All tests pass

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat: complete DynamoDB datasource implementation"
```
