# MongoDB Driver Consolidation

## Problem

The project has two parallel MongoDB implementations that share significant duplicated code:

- `src-tauri/src/db/drivers/mongodb.rs` (599 lines) — implements the `DatabaseDriver` trait
- `src-tauri/src/datasources/mongodb.rs` (345 lines) — standalone client with richer return types

Duplicated code: `build_connection_uri()`, `normalize_mongo_error()`, `trim_to_option()`, `connect()` (SSH tunnel + TLS setup), constants `DEFAULT_MONGODB_PORT` and `DEFAULT_CONNECT_TIMEOUT_MS`.

## Solution

Consolidate into a single `MongoDBDriver` in `db/drivers/mongodb.rs`. Delete `datasources/mongodb.rs`.

## Design

### Part 1: Code Structure Changes

**Target state:**

```
src-tauri/src/db/drivers/mongodb.rs  ← sole MongoDB implementation (post-merge)
src-tauri/src/datasources/mongodb.rs ← deleted
src-tauri/src/commands/mongodb.rs    ← calls MongoDBDriver
src-tauri/src/commands/connection.rs ← calls MongoDBDriver
```

**New content in MongoDBDriver:**

1. Rich type definitions (moved from `datasources/mongodb.rs`):
   - `MongodbConnectionInfo` — `version`, `node_count`
   - `MongodbDatabaseInfo` — `name`, `size_on_disk`, `empty`
   - `MongodbCollectionInfo` — `name`, `database`, `document_count`, `size`

2. Additional methods (do not break `DatabaseDriver` trait):
   - `test_connection_info(&self) -> Result<MongodbConnectionInfo, String>` — runs `serverStatus`, returns version and connection count
   - `list_databases_info(&self) -> Result<Vec<MongodbDatabaseInfo>, String>` — returns full info with sizeOnDisk/empty
   - `list_collections_info(&self, database: &str) -> Result<Vec<MongodbCollectionInfo>, String>` — returns full info with documentCount/size

3. Deduplication: keep only one copy of `trim_to_option()`, `normalize_mongo_error()`, `build_connection_uri()`, `connect()`, and constants.

4. Unit tests: migrate all 8 tests from `datasources/mongodb.rs` `#[cfg(test)] mod tests` into `db/drivers/mongodb.rs`.

### Part 2: Commands Layer Changes

**`commands/mongodb.rs`:**

Replace `MongodbClient` with `MongoDBDriver`. Import concrete type (not trait object) to access rich methods:

```rust
// Before
use crate::datasources::mongodb::{MongodbClient, MongodbConnectionInfo, ...};
async fn client_from_id(...) -> Result<MongodbClient, String> {
    MongodbClient::connect(&connection_form(state, id).await?).await
}

// After
use crate::db::drivers::mongodb::{MongoDBDriver, MongodbConnectionInfo, ...};
async fn driver_from_id(...) -> Result<MongoDBDriver, String> {
    MongoDBDriver::connect(&connection_form(state, id).await?).await
}
```

4 Tauri commands change from `client.xxx()` to `driver.xxx_info()`:

| Command | Before | After |
|---|---|---|
| `mongodb_test_connection` | `client.test_connection()` | `driver.test_connection_info()` |
| `mongodb_test_connection_ephemeral` | `client.test_connection()` | `driver.test_connection_info()` |
| `mongodb_list_databases` | `client.list_databases()` | `driver.list_databases_info()` |
| `mongodb_list_collections` | `client.list_collections(&database)` | `driver.list_collections_info(&database)` |

**`commands/connection.rs`:**

`test_connection_ephemeral` MongoDB branch:

```rust
// Before
} else if form.driver == "mongodb" {
    let client = crate::datasources::mongodb::MongodbClient::connect(&form).await?;
    client.test_connection().await?;

// After
} else if form.driver == "mongodb" {
    let driver = crate::db::drivers::mongodb::MongoDBDriver::connect(&form).await?;
    driver.test_connection().await?;
```

### Part 3: Frontend Changes

None. Tauri command names and return types are unchanged. The `serde(rename_all = "camelCase")` attribute on rich types is preserved when moved to `db/drivers/mongodb.rs`.

### Part 4: Verification

1. `cargo check` — must pass with zero errors
2. Migrate 8 unit tests from `datasources/mongodb.rs` into `db/drivers/mongodb.rs`
3. Existing `tests/mongodb_integration.rs` (2 tests) unchanged — uses `db::drivers::connect()`
4. Manual frontend smoke test: MongoDB connection test (shows version), database list, collection list

## Files Modified

| File | Action |
|---|---|
| `src-tauri/src/db/drivers/mongodb.rs` | Edit: add rich types, extra methods, migrate tests |
| `src-tauri/src/datasources/mongodb.rs` | Delete |
| `src-tauri/src/datasources/mod.rs` | Edit: remove `pub mod mongodb;` |
| `src-tauri/src/commands/mongodb.rs` | Edit: switch from MongodbClient to MongoDBDriver |
| `src-tauri/src/commands/connection.rs` | Edit: MongoDB branch in test_connection_ephemeral |

## Risks

- **Downcast**: `commands/mongodb.rs` needs concrete `MongoDBDriver` type, not `Box<dyn DatabaseDriver>`. Solved by importing `MongoDBDriver::connect()` directly (same pattern already used for Redis/ES in `connection.rs`).
- **Module visibility**: `MongoDBDriver` struct and `connect()` method must be `pub` (they already are).
