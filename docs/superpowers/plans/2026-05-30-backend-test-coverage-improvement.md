# Backend Test Coverage Improvement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix integration test script gaps and add ~77 unit tests for untested backend modules (Cassandra, MCP, Redis, AI, metadata).

**Architecture:** Add `#[cfg(test)] mod tests` blocks to existing source files containing untested pure functions. Deduplicate `default_schema_for_driver` in MCP tools. Extract `normalize_role` from `openai_compat.rs`.

**Tech Stack:** Rust, `cargo test`, `serde_json`

**Working directory:** `.worktrees/test-coverage-improvement`

---

## File Summary

| File | Action | Tests |
|------|--------|-------|
| `scripts/test-integration.sh` | Modify | 0 (activates 31 existing tests) |
| `src-tauri/src/db/drivers/cassandra.rs` | Modify | ~18 |
| `src-tauri/src/mcp/types.rs` | Modify | ~17 |
| `src-tauri/src/mcp/tools/mod.rs` | Modify | ~8 |
| `src-tauri/src/mcp/tools/connection.rs` | Modify | 0 |
| `src-tauri/src/mcp/tools/schema.rs` | Modify | 0 |
| `src-tauri/src/mcp/tools/sql.rs` | Modify | ~9 |
| `src-tauri/src/commands/redis.rs` | Modify | ~13 |
| `src-tauri/src/ai/openai_compat.rs` | Modify | ~9 |
| `src-tauri/src/commands/metadata.rs` | Modify | ~3 |

---

### Task 1: Fix `test-integration.sh` — register orphaned stateful tests

**Files:**
- Modify: `scripts/test-integration.sh`

Two stateful test files exist but are never executed by the integration test script:
- `mssql_stateful_command_integration.rs` (768 lines, 18 tests)
- `starrocks_stateful_command_integration.rs` (594 lines, 13 tests)

- [ ] **Step 1: Add StarRocks stateful test registration**

In `scripts/test-integration.sh`, add to the `starrocks)` case (after line 122):

```bash
  run_integration_test "starrocks_stateful_command_integration"
```

- [ ] **Step 2: Add MSSQL stateful test registration**

In `scripts/test-integration.sh`, add to the `mssql)` case (after line 143):

```bash
  run_integration_test "mssql_stateful_command_integration"
```

- [ ] **Step 3: Add both to the `all)` case**

In the `all)` case, add after the existing `starrocks_command_integration` line:

```bash
  run_integration_test "starrocks_stateful_command_integration"
```

And after the existing `mssql_command_integration` line:

```bash
  run_integration_test "mssql_stateful_command_integration"
```

- [ ] **Step 4: Verify script syntax**

Run: `bash -n scripts/test-integration.sh`
Expected: No output (syntax OK)

- [ ] **Step 5: Commit**

```bash
git add scripts/test-integration.sh
git commit -m "test: register orphaned mssql and starrocks stateful integration tests"
```

---

### Task 2: Cassandra driver unit tests — `normalize_cassandra_error`

**Files:**
- Modify: `src-tauri/src/db/drivers/cassandra.rs`

The function is at line 26. It classifies error strings by pattern matching into 6 categories.

- [ ] **Step 1: Add test module for `normalize_cassandra_error`**

At the end of `cassandra.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_error_authentication() {
        let result = normalize_cassandra_error("Authentication failed for user");
        assert!(result.starts_with("[CASSANDRA_ERROR] Authentication failed"));
    }

    #[test]
    fn normalize_error_credentials() {
        let result = normalize_cassandra_error("invalid credentials provided");
        assert!(result.starts_with("[CASSANDRA_ERROR] Authentication failed"));
    }

    #[test]
    fn normalize_error_connection_refused() {
        let result = normalize_cassandra_error("Connection refused by remote host");
        assert!(result.starts_with("[CASSANDRA_ERROR] Connection refused"));
    }

    #[test]
    fn normalize_error_timeout() {
        let result = normalize_cassandra_error("request timed out after 30s");
        assert!(result.starts_with("[CASSANDRA_ERROR] Connection timed out"));
    }

    #[test]
    fn normalize_error_dns() {
        let result = normalize_cassandra_error("cannot resolve hostname");
        assert!(result.starts_with("[CASSANDRA_ERROR] DNS resolution failed"));
    }

    #[test]
    fn normalize_error_tls() {
        let result = normalize_cassandra_error("certificate verify failed");
        assert!(result.starts_with("[CASSANDRA_ERROR] TLS/SSL error"));
    }

    #[test]
    fn normalize_error_unknown() {
        let result = normalize_cassandra_error("something weird happened");
        assert_eq!(result, "[CASSANDRA_ERROR] something weird happened");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib db::drivers::cassandra::tests 2>&1`
Expected: All 7 tests PASS

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/db/drivers/cassandra.rs
git commit -m "test: add normalize_cassandra_error unit tests"
```

---

### Task 3: Cassandra driver unit tests — `bytes_to_signed_bigint_string` + `unsigned_bytes_to_decimal`

**Files:**
- Modify: `src-tauri/src/db/drivers/cassandra.rs`

These functions convert bytes to decimal strings. `bytes_to_signed_bigint_string` handles two's complement negative numbers.

- [ ] **Step 1: Add tests for `bytes_to_signed_bigint_string`**

Append to the existing `tests` module in `cassandra.rs`:

```rust
    #[test]
    fn signed_bigint_empty() {
        assert_eq!(bytes_to_signed_bigint_string(&[]), "0");
    }

    #[test]
    fn signed_bigint_positive() {
        assert_eq!(bytes_to_signed_bigint_string(&[0x01]), "1");
    }

    #[test]
    fn signed_bigint_negative_one() {
        // 0xFF = two's complement -1
        assert_eq!(bytes_to_signed_bigint_string(&[0xFF]), "-1");
    }

    #[test]
    fn signed_bigint_negative_128() {
        // 0x80 = two's complement -128
        assert_eq!(bytes_to_signed_bigint_string(&[0x80]), "-128");
    }
```

- [ ] **Step 2: Add tests for `unsigned_bytes_to_decimal`**

```rust
    #[test]
    fn unsigned_decimal_empty() {
        assert_eq!(unsigned_bytes_to_decimal(&[]), "0");
    }

    #[test]
    fn unsigned_decimal_all_zeros() {
        assert_eq!(unsigned_bytes_to_decimal(&[0, 0]), "0");
    }

    #[test]
    fn unsigned_decimal_single_byte() {
        assert_eq!(unsigned_bytes_to_decimal(&[255]), "255");
    }

    #[test]
    fn unsigned_decimal_multi_byte() {
        // 0x01 0x00 = 256
        assert_eq!(unsigned_bytes_to_decimal(&[0x01, 0x00]), "256");
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib db::drivers::cassandra::tests 2>&1`
Expected: All 15 tests PASS (7 from Task 2 + 8 new)

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/drivers/cassandra.rs
git commit -m "test: add bytes_to_signed_bigint_string and unsigned_bytes_to_decimal tests"
```

---

### Task 4: Cassandra driver unit tests — `column_type_to_string`

**Files:**
- Modify: `src-tauri/src/db/drivers/cassandra.rs`

This function maps `ColumnType` enum variants to string representations. It handles Native, Collection (List/Set/Map with frozen), Tuple, Vector, and UDT.

- [ ] **Step 1: Add tests for `column_type_to_string`**

Append to the existing `tests` module in `cassandra.rs`:

```rust
    use scylla::frame::response::result::{CollectionType, NativeType};

    #[test]
    fn column_type_native_int() {
        assert_eq!(column_type_to_string(&ColumnType::Native(NativeType::Int)), "int");
    }

    #[test]
    fn column_type_native_text() {
        assert_eq!(column_type_to_string(&ColumnType::Native(NativeType::Text)), "text");
    }

    #[test]
    fn column_type_list() {
        let ct = ColumnType::Collection {
            typ: CollectionType::List(Box::new(ColumnType::Native(NativeType::Int))),
            frozen: false,
        };
        assert_eq!(column_type_to_string(&ct), "list<int>");
    }

    #[test]
    fn column_type_frozen_map() {
        let ct = ColumnType::Collection {
            typ: CollectionType::Map(
                Box::new(ColumnType::Native(NativeType::Text)),
                Box::new(ColumnType::Native(NativeType::Int)),
            ),
            frozen: true,
        };
        assert_eq!(column_type_to_string(&ct), "frozen<map<text, int>>");
    }

    #[test]
    fn column_type_tuple() {
        let ct = ColumnType::Tuple(vec![
            ColumnType::Native(NativeType::Int),
            ColumnType::Native(NativeType::Text),
        ]);
        assert_eq!(column_type_to_string(&ct), "tuple<int, text>");
    }
```

- [ ] **Step 2: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib db::drivers::cassandra::tests 2>&1`
Expected: All 20 tests PASS (15 from previous tasks + 5 new)

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/db/drivers/cassandra.rs
git commit -m "test: add column_type_to_string unit tests"
```

---

### Task 5: MCP types unit tests — constructors + serde round-trip

**Files:**
- Modify: `src-tauri/src/mcp/types.rs`

Test `JsonRpcResponse`, `ToolResult` constructors and serde serialization for all types.

- [ ] **Step 1: Add test module to types.rs**

At the end of `types.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // --- Constructor tests ---

    #[test]
    fn jsonrpc_response_success() {
        let resp = JsonRpcResponse::success(Some(serde_json::json!(1)), serde_json::json!("ok"));
        assert_eq!(resp.jsonrpc, "2.0");
        assert_eq!(resp.id, Some(serde_json::json!(1)));
        assert_eq!(resp.result, Some(serde_json::json!("ok")));
        assert!(resp.error.is_none());
    }

    #[test]
    fn jsonrpc_response_error() {
        let resp = JsonRpcResponse::error(Some(serde_json::json!(1)), -32601, "not found".to_string());
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.result.is_none());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32601);
        assert_eq!(err.message, "not found");
        assert!(err.data.is_none());
    }

    #[test]
    fn tool_result_text() {
        let tr = ToolResult::text("hello".to_string());
        assert_eq!(tr.content.len(), 1);
        assert_eq!(tr.content[0].content_type, "text");
        assert_eq!(tr.content[0].text, "hello");
        assert!(tr.is_error.is_none());
    }

    #[test]
    fn tool_result_error() {
        let tr = ToolResult::error("oops".to_string());
        assert_eq!(tr.content.len(), 1);
        assert_eq!(tr.content[0].text, "oops");
        assert_eq!(tr.is_error, Some(true));
    }

    // --- Serde round-trip tests ---

    #[test]
    fn serde_jsonrpc_request_full() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "initialize".to_string(),
            params: Some(serde_json::json!({})),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"initialize\""));
        let decoded: JsonRpcRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.method, "initialize");
    }

    #[test]
    fn serde_jsonrpc_request_minimal() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "ping".to_string(),
            params: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("\"id\""));
        assert!(!json.contains("\"params\""));
    }

    #[test]
    fn serde_tool_definition_rename() {
        let td = ToolDefinition {
            name: "test".to_string(),
            description: "desc".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        };
        let json = serde_json::to_string(&td).unwrap();
        assert!(json.contains("\"inputSchema\""));
        assert!(!json.contains("\"input_schema\""));
    }

    #[test]
    fn serde_resource_definition_rename() {
        let rd = ResourceDefinition {
            uri: "file:///test".to_string(),
            name: "test".to_string(),
            description: "desc".to_string(),
            mime_type: "text/plain".to_string(),
        };
        let json = serde_json::to_string(&rd).unwrap();
        assert!(json.contains("\"mimeType\""));
    }

    #[test]
    fn serde_prompt_definition_with_arguments() {
        let pd = PromptDefinition {
            name: "test".to_string(),
            description: "desc".to_string(),
            arguments: Some(vec![PromptArgument {
                name: "arg1".to_string(),
                description: "an arg".to_string(),
                required: true,
            }]),
        };
        let json = serde_json::to_string(&pd).unwrap();
        assert!(json.contains("\"arguments\""));
    }

    #[test]
    fn serde_prompt_definition_without_arguments() {
        let pd = PromptDefinition {
            name: "test".to_string(),
            description: "desc".to_string(),
            arguments: None,
        };
        let json = serde_json::to_string(&pd).unwrap();
        assert!(!json.contains("\"arguments\""));
    }

    #[test]
    fn serde_text_content_rename() {
        let tc = TextContent {
            content_type: "text".to_string(),
            text: "hello".to_string(),
        };
        let json = serde_json::to_string(&tc).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(!json.contains("\"content_type\""));
    }

    #[test]
    fn serde_tool_result_is_error_rename() {
        let tr = ToolResult::error("fail".to_string());
        let json = serde_json::to_string(&tr).unwrap();
        assert!(json.contains("\"isError\":true"));
        assert!(!json.contains("\"is_error\""));
    }

    #[test]
    fn serde_tool_result_no_error_field_when_none() {
        let tr = ToolResult::text("ok".to_string());
        let json = serde_json::to_string(&tr).unwrap();
        assert!(!json.contains("isError"));
    }

    #[test]
    fn serde_roundtrip_tool_result() {
        let tr = ToolResult::text("hello world".to_string());
        let json = serde_json::to_string(&tr).unwrap();
        let decoded: ToolResult = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.content[0].text, "hello world");
    }

    // --- Error code constants ---

    #[test]
    fn error_codes_match_jsonrpc_spec() {
        assert_eq!(PARSE_ERROR, -32700);
        assert_eq!(INVALID_REQUEST, -32600);
        assert_eq!(METHOD_NOT_FOUND, -32601);
        assert_eq!(INVALID_PARAMS, -32602);
        assert_eq!(INTERNAL_ERROR, -32603);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib mcp::types::tests 2>&1`
Expected: All 17 tests PASS

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/mcp/types.rs
git commit -m "test: add MCP types unit tests (constructors, serde, error codes)"
```

---

### Task 6: MCP tools — deduplicate `default_schema_for_driver` + add tests

**Files:**
- Modify: `src-tauri/src/mcp/tools/mod.rs`
- Modify: `src-tauri/src/mcp/tools/connection.rs`
- Modify: `src-tauri/src/mcp/tools/schema.rs`

The function `default_schema_for_driver` is duplicated identically in `connection.rs` (line 5) and `schema.rs` (line 5). Move it to `mod.rs` and update both files.

- [ ] **Step 1: Add shared function + tests to `mod.rs`**

In `src-tauri/src/mcp/tools/mod.rs`, add the shared function before `get_tool_definitions`:

```rust
pub fn default_schema_for_driver(driver: &str) -> String {
    match driver.to_ascii_lowercase().as_str() {
        "postgres" | "cockroach" => "public".to_string(),
        "mysql" | "mariadb" | "tidb" | "starrocks" | "doris" => "main".to_string(),
        "sqlite" | "duckdb" => "main".to_string(),
        "clickhouse" => "default".to_string(),
        "mssql" => "dbo".to_string(),
        _ => "public".to_string(),
    }
}
```

At the end of `mod.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_postgres() {
        assert_eq!(default_schema_for_driver("postgres"), "public");
    }

    #[test]
    fn schema_cockroach() {
        assert_eq!(default_schema_for_driver("cockroach"), "public");
    }

    #[test]
    fn schema_mysql() {
        assert_eq!(default_schema_for_driver("mysql"), "main");
    }

    #[test]
    fn schema_sqlite() {
        assert_eq!(default_schema_for_driver("sqlite"), "main");
    }

    #[test]
    fn schema_clickhouse() {
        assert_eq!(default_schema_for_driver("clickhouse"), "default");
    }

    #[test]
    fn schema_mssql() {
        assert_eq!(default_schema_for_driver("mssql"), "dbo");
    }

    #[test]
    fn schema_unknown_defaults_to_public() {
        assert_eq!(default_schema_for_driver("some_new_db"), "public");
    }

    #[test]
    fn schema_case_insensitive() {
        assert_eq!(default_schema_for_driver("POSTGRES"), "public");
        assert_eq!(default_schema_for_driver("MySQL"), "main");
    }
}
```

- [ ] **Step 2: Update `connection.rs` to use shared function**

In `src-tauri/src/mcp/tools/connection.rs`, remove lines 5-14 (the local `default_schema_for_driver` function) and update line 16 to use the shared one:

```rust
async fn get_schema_for_connection(state: &AppState, connection_id: i64) -> Result<String, String> {
    let connections = crate::commands::connection::get_connections_direct(state).await?;
    let conn = connections
        .iter()
        .find(|c| c.id == connection_id)
        .ok_or_else(|| format!("Connection {} not found", connection_id))?;
    Ok(super::default_schema_for_driver(&conn.driver))
}
```

- [ ] **Step 3: Update `schema.rs` to use shared function**

In `src-tauri/src/mcp/tools/schema.rs`, remove lines 5-13 (the local `default_schema_for_driver` function). Find any call to `default_schema_for_driver` in the file and replace with `super::default_schema_for_driver`.

- [ ] **Step 4: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib mcp::tools::tests 2>&1`
Expected: All 8 tests PASS

- [ ] **Step 5: Verify no regressions**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib 2>&1`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/mcp/tools/mod.rs src-tauri/src/mcp/tools/connection.rs src-tauri/src/mcp/tools/schema.rs
git commit -m "refactor: deduplicate default_schema_for_driver into mcp/tools/mod.rs"
```

---

### Task 7: MCP tools `sql.rs` — `format_value` + `get_definitions` tests

**Files:**
- Modify: `src-tauri/src/mcp/tools/sql.rs`

- [ ] **Step 1: Add test module to sql.rs**

At the end of `sql.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_value_null() {
        assert_eq!(format_value(&Value::Null), "NULL");
    }

    #[test]
    fn format_value_string_short() {
        assert_eq!(format_value(&Value::String("hello".to_string())), "hello");
    }

    #[test]
    fn format_value_string_long_truncated() {
        let long = "a".repeat(101);
        let result = format_value(&Value::String(long));
        assert_eq!(result.len(), 100); // 97 chars + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn format_value_string_exactly_100_chars() {
        let s = "a".repeat(100);
        let result = format_value(&Value::String(s.clone()));
        assert_eq!(result, s); // no truncation
    }

    #[test]
    fn format_value_number() {
        assert_eq!(format_value(&serde_json::json!(42)), "42");
    }

    #[test]
    fn format_value_bool() {
        assert_eq!(format_value(&Value::Bool(true)), "true");
        assert_eq!(format_value(&Value::Bool(false)), "false");
    }

    #[test]
    fn format_value_array() {
        assert_eq!(format_value(&Value::Array(vec![])), "[array]");
    }

    #[test]
    fn format_value_object() {
        assert_eq!(format_value(&Value::Object(serde_json::Map::new())), "{object}");
    }

    #[test]
    fn get_definitions_returns_execute_query_tool() {
        let defs = get_definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "dbpaw_execute_query");
        let schema = &defs[0].input_schema;
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&Value::String("connection_id".to_string())));
        assert!(required.contains(&Value::String("sql".to_string())));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib mcp::tools::sql::tests 2>&1`
Expected: All 9 tests PASS

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/mcp/tools/sql.rs
git commit -m "test: add MCP sql tool unit tests (format_value, get_definitions)"
```

---

### Task 8: Redis command unit tests

**Files:**
- Modify: `src-tauri/src/commands/redis.rs`

Three pure functions: `cache_key` (line 34), `is_io_error` (line 43), `clamp_redis_command_logs_limit` (line 1420).

- [ ] **Step 1: Add test module to redis.rs**

At the end of `redis.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // --- cache_key ---

    #[test]
    fn cache_key_standalone_with_database() {
        assert_eq!(cache_key(1, Some("db0"), false), "1:db0");
    }

    #[test]
    fn cache_key_standalone_no_database() {
        assert_eq!(cache_key(1, None, false), "1:");
    }

    #[test]
    fn cache_key_cluster_with_database() {
        assert_eq!(cache_key(42, Some("db1"), true), "42:cluster");
    }

    #[test]
    fn cache_key_cluster_no_database() {
        assert_eq!(cache_key(42, None, true), "42:cluster");
    }

    #[test]
    fn cache_key_standalone_custom_db() {
        assert_eq!(cache_key(99, Some("mydb"), false), "99:mydb");
    }

    // --- is_io_error ---

    #[test]
    fn io_error_broken_pipe() {
        assert!(is_io_error("[REDIS_ERROR] broken pipe"));
    }

    #[test]
    fn io_error_connection_reset() {
        assert!(is_io_error("[REDIS_ERROR] connection reset by peer"));
    }

    #[test]
    fn io_error_connection_refused() {
        assert!(is_io_error("[REDIS_ERROR] connection refused"));
    }

    #[test]
    fn io_error_not_redis_error() {
        assert!(!is_io_error("some other error"));
    }

    #[test]
    fn io_error_redis_but_not_io() {
        assert!(!is_io_error("[REDIS_ERROR] ERR wrong number of arguments"));
    }

    // --- clamp_redis_command_logs_limit ---

    #[test]
    fn clamp_none_returns_default() {
        assert_eq!(clamp_redis_command_logs_limit(None), 100);
    }

    #[test]
    fn clamp_within_range() {
        assert_eq!(clamp_redis_command_logs_limit(Some(50)), 50);
    }

    #[test]
    fn clamp_below_minimum() {
        assert_eq!(clamp_redis_command_logs_limit(Some(0)), 1);
    }

    #[test]
    fn clamp_above_maximum() {
        assert_eq!(clamp_redis_command_logs_limit(Some(200)), 100);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib commands::redis::tests 2>&1`
Expected: All 14 tests PASS

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/redis.rs
git commit -m "test: add Redis command unit tests (cache_key, is_io_error, clamp)"
```

---

### Task 9: AI openai_compat — extract `normalize_role` + add tests

**Files:**
- Modify: `src-tauri/src/ai/openai_compat.rs`

The role normalization logic is duplicated at lines 98-102 and 207-210. Extract to a standalone function and test it.

- [ ] **Step 1: Add `normalize_role` function**

In `src-tauri/src/ai/openai_compat.rs`, add after the imports (before `impl AIProvider`):

```rust
fn normalize_role(role: &str) -> String {
    match role {
        "system" | "user" | "assistant" | "tool" => role.to_string(),
        "developer" => "system".to_string(),
        _ => "user".to_string(),
    }
}
```

- [ ] **Step 2: Replace inline closures in `chat_once`**

Replace the closure at lines 98-102 in `chat_once`:

```rust
// Before:
let role = match m.role.as_str() {
    "system" | "user" | "assistant" | "tool" => m.role,
    "developer" => "system".to_string(),
    _ => "user".to_string(),
};

// After:
let role = normalize_role(&m.role);
```

- [ ] **Step 3: Replace inline closures in `chat_stream`**

Replace the closure at lines 207-210 in `chat_stream`:

```rust
// Before:
let role = match m.role.as_str() {
    "system" | "user" | "assistant" | "tool" => m.role,
    "developer" => "system".to_string(),
    _ => "user".to_string(),
};

// After:
let role = normalize_role(&m.role);
```

- [ ] **Step 4: Add tests for `validate_config` and `normalize_role`**

At the end of `openai_compat.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_provider(base_url: &str, api_key: &str, model: &str) -> OpenAICompatProvider {
        OpenAICompatProvider {
            name: "test".to_string(),
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
            temperature: 0.7,
            max_tokens: 4096,
            extra_json: None,
        }
    }

    // --- validate_config ---

    #[test]
    fn validate_config_all_valid() {
        let p = make_provider("http://localhost", "key", "gpt-4");
        assert!(p.validate_config().is_ok());
    }

    #[test]
    fn validate_config_empty_base_url() {
        let p = make_provider("", "key", "gpt-4");
        let err = p.validate_config().unwrap_err();
        assert!(err.contains("baseUrl"));
    }

    #[test]
    fn validate_config_empty_api_key() {
        let p = make_provider("http://localhost", "", "gpt-4");
        let err = p.validate_config().unwrap_err();
        assert!(err.contains("apiKey"));
    }

    #[test]
    fn validate_config_empty_model() {
        let p = make_provider("http://localhost", "key", "");
        let err = p.validate_config().unwrap_err();
        assert!(err.contains("model"));
    }

    // --- normalize_role ---

    #[test]
    fn normalize_role_system() {
        assert_eq!(normalize_role("system"), "system");
    }

    #[test]
    fn normalize_role_user() {
        assert_eq!(normalize_role("user"), "user");
    }

    #[test]
    fn normalize_role_assistant() {
        assert_eq!(normalize_role("assistant"), "assistant");
    }

    #[test]
    fn normalize_role_developer_to_system() {
        assert_eq!(normalize_role("developer"), "system");
    }

    #[test]
    fn normalize_role_unknown_to_user() {
        assert_eq!(normalize_role("unknown_role"), "user");
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib ai::openai_compat::tests 2>&1`
Expected: All 9 tests PASS

- [ ] **Step 6: Verify no regressions**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib 2>&1`
Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/ai/openai_compat.rs
git commit -m "refactor: extract normalize_role, add openai_compat unit tests"
```

---

### Task 10: Commands metadata — `ensure_table_structure_found` test

**Files:**
- Modify: `src-tauri/src/commands/metadata.rs`

- [ ] **Step 1: Add test module to metadata.rs**

At the end of `metadata.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_structure(columns: Vec<crate::models::ColumnInfo>) -> TableStructure {
        TableStructure { columns }
    }

    #[test]
    fn ensure_table_structure_found_with_columns() {
        let structure = make_structure(vec![crate::models::ColumnInfo {
            name: "id".to_string(),
            r#type: "int".to_string(),
            nullable: false,
            default_value: None,
            primary_key: true,
            comment: None,
            default_constraint_name: None,
        }]);
        let result = ensure_table_structure_found(structure, "users");
        assert!(result.is_ok());
    }

    #[test]
    fn ensure_table_structure_found_empty_columns() {
        let structure = make_structure(vec![]);
        let result = ensure_table_structure_found(structure, "users");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("[NOT_FOUND]"));
        assert!(err.contains("users"));
    }

    #[test]
    fn ensure_table_structure_error_includes_table_name() {
        let structure = make_structure(vec![]);
        let err = ensure_table_structure_found(structure, "orders").unwrap_err();
        assert!(err.contains("orders"));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib commands::metadata::tests 2>&1`
Expected: All 3 tests PASS

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/metadata.rs
git commit -m "test: add ensure_table_structure_found unit tests"
```

---

## Verification

After all tasks complete, run the full test suite:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

Expected: All tests pass (386 existing + ~77 new = ~463 total).

Then run smoke test from the main worktree to verify no regressions:

```bash
bun run test:smoke
```
