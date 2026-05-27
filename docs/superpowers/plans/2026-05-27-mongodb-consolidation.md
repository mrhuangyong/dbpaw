# MongoDB Driver Consolidation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate the duplicate `MongodbClient` in `datasources/mongodb.rs` by merging its rich types and methods into `MongoDBDriver` in `db/drivers/mongodb.rs`.

**Architecture:** `MongoDBDriver` gains `test_connection_info()`, `list_databases_info()`, `list_collections_info()` methods that return richer types (version, size, count). The `datasources/mongodb.rs` file is deleted. Commands switch to using `MongoDBDriver` directly.

**Tech Stack:** Rust, mongodb 3.6 crate, Tauri commands, serde

---

### Task 1: Add rich types and extra methods to MongoDBDriver

**Files:**
- Modify: `src-tauri/src/db/drivers/mongodb.rs:1-10` (add imports)
- Modify: `src-tauri/src/db/drivers/mongodb.rs:160-219` (add types and methods after struct)

- [ ] **Step 1: Add `use serde::{Deserialize, Serialize};` to imports**

At `src-tauri/src/db/drivers/mongodb.rs:6`, after the existing `use` block, add the serde import. The file's current imports (lines 1-12):

```rust
use super::DatabaseDriver;
use crate::models::{
    ColumnInfo, ColumnSchema, ConnectionForm, IndexInfo, QueryColumn, QueryResult, SchemaOverview,
    SingleResultSet, TableDataResponse, TableInfo, TableMetadata, TableSchema, TableStructure,
};
use async_trait::async_trait;
use mongodb::bson::{doc, Bson, Document};
use mongodb::options::{ClientOptions, Tls, TlsOptions};
use mongodb::Client;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
```

Add after line 6 (`use async_trait::async_trait;`):

```rust
use serde::{Deserialize, Serialize};
```

- [ ] **Step 2: Add rich type definitions**

Insert after the `MongoDBDriver` struct definition (after line 169), before `impl MongoDBDriver`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MongodbConnectionInfo {
    pub version: Option<String>,
    pub node_count: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MongodbDatabaseInfo {
    pub name: String,
    pub size_on_disk: Option<i64>,
    pub empty: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MongodbCollectionInfo {
    pub name: String,
    pub database: String,
    pub document_count: Option<i64>,
    pub size: Option<i64>,
}
```

- [ ] **Step 3: Add `test_connection_info()` method**

Inside the existing `impl MongoDBDriver` block, after the `collect_cursor` method (after line 331), add:

```rust
pub async fn test_connection_info(&self) -> Result<MongodbConnectionInfo, String> {
    let db = self.get_database("admin");
    let result = db
        .run_command(doc! { "serverStatus": 1 })
        .await
        .map_err(normalize_mongo_error)?;

    let version = result.get_str("version").ok().map(|s| s.to_string());
    let node_count = result
        .get_document("connections")
        .ok()
        .and_then(|c| c.get_i32("current").ok());

    Ok(MongodbConnectionInfo {
        version,
        node_count,
    })
}
```

- [ ] **Step 4: Add `list_databases_info()` method**

After `test_connection_info()`:

```rust
pub async fn list_databases_info(&self) -> Result<Vec<MongodbDatabaseInfo>, String> {
    let databases = self.client.list_databases().await.map_err(normalize_mongo_error)?;
    Ok(databases
        .into_iter()
        .map(|db| MongodbDatabaseInfo {
            name: db.name,
            size_on_disk: Some(db.size_on_disk as i64),
            empty: Some(db.empty),
        })
        .collect())
}
```

- [ ] **Step 5: Add `list_collections_info()` method**

After `list_databases_info()`:

```rust
pub async fn list_collections_info(
    &self,
    database: &str,
) -> Result<Vec<MongodbCollectionInfo>, String> {
    let db = self.client.database(database);
    let mut cursor = db
        .list_collections()
        .await
        .map_err(normalize_mongo_error)?;

    let mut result = Vec::new();
    while cursor.advance().await.map_err(normalize_mongo_error)? {
        let collection = cursor.deserialize_current().map_err(normalize_mongo_error)?;
        result.push(MongodbCollectionInfo {
            name: collection.name,
            database: database.to_string(),
            document_count: None,
            size: None,
        });
    }

    Ok(result)
}
```

- [ ] **Step 6: Run `cargo check` to verify compilation**

Run: `cargo check`
Expected: no errors

---

### Task 2: Migrate unit tests from datasources/mongodb.rs

**Files:**
- Modify: `src-tauri/src/db/drivers/mongodb.rs` (add `#[cfg(test)] mod tests` at end)

- [ ] **Step 1: Add test module at end of file**

Append to the end of `src-tauri/src/db/drivers/mongodb.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_form(driver: &str, host: Option<&str>, port: Option<i64>) -> ConnectionForm {
        ConnectionForm {
            driver: driver.to_string(),
            host: host.map(|s| s.to_string()),
            port,
            ..Default::default()
        }
    }

    #[test]
    fn build_uri_basic() {
        let form = make_form("mongodb", Some("localhost"), Some(27017));
        let uri = build_connection_uri(&form).unwrap();
        assert_eq!(uri, "mongodb://localhost:27017");
    }

    #[test]
    fn build_uri_with_auth() {
        let form = ConnectionForm {
            driver: "mongodb".to_string(),
            host: Some("localhost".to_string()),
            port: Some(27017),
            username: Some("admin".to_string()),
            password: Some("pass word".to_string()),
            ..Default::default()
        };
        let uri = build_connection_uri(&form).unwrap();
        assert!(uri.starts_with("mongodb://admin:pass%20word@localhost:27017"));
    }

    #[test]
    fn build_uri_with_database() {
        let form = ConnectionForm {
            driver: "mongodb".to_string(),
            host: Some("localhost".to_string()),
            port: Some(27017),
            database: Some("mydb".to_string()),
            ..Default::default()
        };
        let uri = build_connection_uri(&form).unwrap();
        assert_eq!(uri, "mongodb://localhost:27017/mydb");
    }

    #[test]
    fn build_uri_with_auth_source() {
        let form = ConnectionForm {
            driver: "mongodb".to_string(),
            host: Some("localhost".to_string()),
            port: Some(27017),
            username: Some("admin".to_string()),
            password: Some("pass".to_string()),
            auth_source: Some("admin".to_string()),
            ..Default::default()
        };
        let uri = build_connection_uri(&form).unwrap();
        assert!(uri.contains("authSource=admin"));
    }

    #[test]
    fn build_uri_with_ssl() {
        let form = ConnectionForm {
            driver: "mongodb".to_string(),
            host: Some("localhost".to_string()),
            port: Some(27017),
            ssl: Some(true),
            ..Default::default()
        };
        let uri = build_connection_uri(&form).unwrap();
        assert!(uri.contains("ssl=true"));
    }

    #[test]
    fn build_uri_default_port() {
        let form = make_form("mongodb", Some("localhost"), None);
        let uri = build_connection_uri(&form).unwrap();
        assert!(uri.contains("localhost:27017"));
    }

    #[test]
    fn build_uri_missing_host() {
        let form = make_form("mongodb", None, None);
        assert!(build_connection_uri(&form).is_err());
    }

    #[test]
    fn build_uri_invalid_port() {
        let form = make_form("mongodb", Some("localhost"), Some(99999));
        assert!(build_connection_uri(&form).is_err());
    }

    #[test]
    fn normalize_error_categorization() {
        assert!(normalize_mongo_error("authentication failed").contains("Authentication failed"));
        assert!(normalize_mongo_error("dns resolve error").contains("DNS resolution failed"));
        assert!(normalize_mongo_error("connection timed out").contains("Connection timed out"));
        assert!(normalize_mongo_error("connection refused").contains("Connection refused"));
        assert!(normalize_mongo_error("some other error").starts_with("[MONGODB_ERROR]"));
    }
}
```

- [ ] **Step 2: Run `cargo test` to verify tests pass**

Run: `cargo test --lib db::drivers::mongodb::tests`
Expected: 9 tests pass (8 migrated + any existing)

---

### Task 3: Update commands/mongodb.rs to use MongoDBDriver

**Files:**
- Modify: `src-tauri/src/commands/mongodb.rs` (full rewrite)

- [ ] **Step 1: Replace file contents**

Replace the entire `src-tauri/src/commands/mongodb.rs` with:

```rust
use crate::db::drivers::mongodb::{
    MongodbCollectionInfo, MongodbConnectionInfo, MongodbDatabaseInfo, MongoDBDriver,
};
use crate::models::TestConnectionResult;
use crate::state::AppState;
use std::time::Instant;
use tauri::State;

async fn connection_form(
    state: &State<'_, AppState>,
    id: i64,
) -> Result<crate::models::ConnectionForm, String> {
    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };
    let db = local_db.ok_or("Local DB not initialized")?;
    let form = db.get_connection_form_by_id(id).await?;
    if form.driver != "mongodb" {
        return Err(format!(
            "[UNSUPPORTED] Connection {} is not a MongoDB connection",
            id
        ));
    }
    Ok(form)
}

async fn driver_from_id(state: &State<'_, AppState>, id: i64) -> Result<MongoDBDriver, String> {
    MongoDBDriver::connect(&connection_form(state, id).await?).await
}

#[tauri::command]
pub async fn mongodb_test_connection(
    state: State<'_, AppState>,
    id: i64,
) -> Result<MongodbConnectionInfo, String> {
    driver_from_id(&state, id).await?.test_connection_info().await
}

#[tauri::command]
pub async fn mongodb_test_connection_ephemeral(
    form: crate::models::ConnectionForm,
) -> Result<TestConnectionResult, String> {
    let started = Instant::now();
    let driver = MongoDBDriver::connect(&form).await?;
    match driver.test_connection_info().await {
        Ok(info) => Ok(TestConnectionResult {
            success: true,
            message: format!(
                "Connected to MongoDB {}",
                info.version.unwrap_or_else(|| "server".to_string())
            ),
            latency_ms: Some(started.elapsed().as_millis() as i64),
        }),
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn mongodb_list_databases(
    state: State<'_, AppState>,
    id: i64,
) -> Result<Vec<MongodbDatabaseInfo>, String> {
    driver_from_id(&state, id).await?.list_databases_info().await
}

#[tauri::command]
pub async fn mongodb_list_collections(
    state: State<'_, AppState>,
    id: i64,
    database: String,
) -> Result<Vec<MongodbCollectionInfo>, String> {
    driver_from_id(&state, id)
        .await?
        .list_collections_info(&database)
        .await
}
```

- [ ] **Step 2: Run `cargo check` to verify compilation**

Run: `cargo check`
Expected: no errors

---

### Task 4: Update commands/connection.rs MongoDB branch

**Files:**
- Modify: `src-tauri/src/commands/connection.rs:481-483`

- [ ] **Step 1: Replace the MongoDB branch in `test_connection_ephemeral`**

Change lines 481-483 from:

```rust
    } else if form.driver == "mongodb" {
        let client = crate::datasources::mongodb::MongodbClient::connect(&form).await?;
        client.test_connection().await?;
```

To:

```rust
    } else if form.driver == "mongodb" {
        let driver = crate::db::drivers::mongodb::MongoDBDriver::connect(&form).await?;
        driver.test_connection().await?;
```

- [ ] **Step 2: Run `cargo check` to verify compilation**

Run: `cargo check`
Expected: no errors

---

### Task 5: Delete datasources/mongodb.rs and update mod.rs

**Files:**
- Delete: `src-tauri/src/datasources/mongodb.rs`
- Modify: `src-tauri/src/datasources/mod.rs`

- [ ] **Step 1: Delete the datasource file**

Run: `rm src-tauri/src/datasources/mongodb.rs`

- [ ] **Step 2: Remove mongodb from datasources/mod.rs**

Change `src-tauri/src/datasources/mod.rs` from:

```rust
pub mod elasticsearch;
pub mod mongodb;
pub mod redis;
```

To:

```rust
pub mod elasticsearch;
pub mod redis;
```

- [ ] **Step 3: Run `cargo check` to verify no remaining references**

Run: `cargo check`
Expected: no errors (all references to `datasources::mongodb` should already be gone from Tasks 3-4)

---

### Task 6: Final verification

**Files:**
- None (verification only)

- [ ] **Step 1: Run `cargo check`**

Run: `cargo check`
Expected: zero errors, zero warnings related to mongodb

- [ ] **Step 2: Run unit tests**

Run: `cargo test --lib db::drivers::mongodb::tests`
Expected: 9 tests pass

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: all tests pass
