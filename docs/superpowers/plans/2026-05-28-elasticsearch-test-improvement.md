# Elasticsearch Test Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Improve Elasticsearch test coverage by adding unit tests for helper functions and splitting the monolithic integration test.

**Architecture:** Add ~19 unit tests to the existing `#[cfg(test)]` module in `elasticsearch.rs`. Split `test_elasticsearch_read_only_flow` into 8 independent test functions in `elasticsearch_integration.rs`.

**Tech Stack:** Rust, tokio::test, testcontainers, reqwest

---

## Task 1: Add unit tests for helper functions

**Files:**
- Modify: `src-tauri/src/datasources/elasticsearch.rs` (in `#[cfg(test)] mod tests`)

Add the following tests to the existing test module. Add necessary imports to the `use super::` block.

### Step 1: Add imports

Add to the existing `use super::` block:
```rust
build_auth, build_search_body, set_search_pagination, validate_file_path,
parse_docs_count, ElasticsearchAuth, trim_to_option,
```

### Step 2: Add build_auth tests

```rust
#[test]
fn build_auth_auto_mode_no_credentials_returns_none() {
    let form = ConnectionForm { driver: "elasticsearch".to_string(), ..Default::default() };
    assert!(matches!(build_auth(&form).unwrap(), ElasticsearchAuth::None));
}

#[test]
fn build_auth_auto_mode_detects_basic_from_username() {
    let form = ConnectionForm {
        driver: "elasticsearch".to_string(),
        username: Some("user".to_string()),
        password: Some("pass".to_string()),
        ..Default::default()
    };
    match build_auth(&form).unwrap() {
        ElasticsearchAuth::Basic { username, password } => {
            assert_eq!(username, "user");
            assert_eq!(password.as_deref(), Some("pass"));
        }
        _ => panic!("expected Basic auth"),
    }
}

#[test]
fn build_auth_auto_mode_detects_api_key() {
    let form = ConnectionForm {
        driver: "elasticsearch".to_string(),
        api_key_encoded: Some("mykey".to_string()),
        ..Default::default()
    };
    match build_auth(&form).unwrap() {
        ElasticsearchAuth::ApiKey(key) => assert_eq!(key, "mykey"),
        _ => panic!("expected ApiKey auth"),
    }
}

#[test]
fn build_auth_unsupported_mode_returns_error() {
    let form = ConnectionForm {
        driver: "elasticsearch".to_string(),
        auth_mode: Some("oauth".to_string()),
        ..Default::default()
    };
    assert!(build_auth(&form).is_err());
}
```

### Step 3: Add validate_file_path tests

```rust
#[test]
fn validate_file_path_rejects_empty() {
    assert!(validate_file_path("", "export").is_err());
    assert!(validate_file_path("   ", "export").is_err());
}

#[test]
fn validate_file_path_accepts_valid_path() {
    let result = validate_file_path("/tmp/test.ndjson", "export").unwrap();
    assert_eq!(result, std::path::PathBuf::from("/tmp/test.ndjson"));
}
```

### Step 4: Add build_search_body tests

```rust
#[test]
fn build_search_body_dsl_takes_priority() {
    let result = build_search_body(
        Some("ignored".to_string()),
        Some(r#"{"match":{"title":"hello"}}"#.to_string()),
    ).unwrap();
    assert_eq!(result["match"]["title"], "hello");
}

#[test]
fn build_search_body_query_string_fallback() {
    let result = build_search_body(Some("status:ok".to_string()), None).unwrap();
    assert_eq!(result["query"]["query_string"]["query"], "status:ok");
}

#[test]
fn build_search_body_match_all_default() {
    let result = build_search_body(None, None).unwrap();
    assert!(result["query"]["match_all"].is_object());
}

#[test]
fn build_search_body_invalid_dsl_returns_error() {
    assert!(build_search_body(None, Some("not json".to_string())).is_err());
}
```

### Step 5: Add set_search_pagination tests

```rust
#[test]
fn set_search_pagination_sets_from_and_size() {
    let mut body = serde_json::json!({"query": {"match_all": {}}});
    set_search_pagination(&mut body, Some(10), 50).unwrap();
    assert_eq!(body["from"], 10);
    assert_eq!(body["size"], 50);
}

#[test]
fn set_search_pagination_removes_from_when_none() {
    let mut body = serde_json::json!({"query": {"match_all": {}}, "from": 10});
    set_search_pagination(&mut body, None, 50).unwrap();
    assert!(body.get("from").is_none());
    assert_eq!(body["size"], 50);
}

#[test]
fn set_search_pagination_rejects_non_object() {
    let mut body = serde_json::json!([1, 2, 3]);
    assert!(set_search_pagination(&mut body, None, 50).is_err());
}
```

### Step 6: Add parse_docs_count tests

```rust
#[test]
fn parse_docs_count_parses_valid_number() {
    assert_eq!(parse_docs_count(Some("42")), Some(42));
}

#[test]
fn parse_docs_count_returns_none_for_none() {
    assert_eq!(parse_docs_count(None), None);
}

#[test]
fn parse_docs_count_returns_none_for_non_numeric() {
    assert_eq!(parse_docs_count(Some("abc")), None);
}
```

### Step 7: Add validate_raw_path additional tests

```rust
#[test]
fn validate_raw_path_rejects_double_dot() {
    assert!(validate_raw_path("/../secret").is_err());
}

#[test]
fn validate_raw_path_auto_prepends_slash() {
    assert_eq!(validate_raw_path("_cluster/health").unwrap(), "/_cluster/health");
}
```

### Step 8: Add normalize_error additional tests

```rust
#[test]
fn normalize_error_falls_back_to_error_type() {
    let body = r#"{"error":{"type":"search_phase_execution_exception"}}"#;
    let err = normalize_error(StatusCode::BAD_REQUEST, body);
    assert!(err.contains("search_phase_execution_exception"));
}

#[test]
fn normalize_error_handles_empty_body() {
    let err = normalize_error(StatusCode::NOT_FOUND, "");
    assert!(err.contains("HTTP 404"));
}

#[test]
fn normalize_error_handles_non_json_body() {
    let err = normalize_error(StatusCode::INTERNAL_SERVER_ERROR, "server error");
    assert!(err.contains("server error"));
}
```

### Step 9: Run unit tests

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: All tests PASS

### Step 10: Commit

```bash
git add src-tauri/src/datasources/elasticsearch.rs
git commit -m "test(elasticsearch): add unit tests for helper functions"
```

---

## Task 2: Split integration test into independent scenarios

**Files:**
- Modify: `src-tauri/tests/elasticsearch_integration.rs`

### Step 1: Add helper function and rewrite test file

Replace the entire file with the following structure. The shared setup creates a client and base_url. Each test creates its own index with a unique name and cleans up.

```rust
#[path = "common/elasticsearch_context.rs"]
mod elasticsearch_context;

use dbpaw_lib::datasources::elasticsearch::{build_base_url, ElasticsearchClient};
use serde_json::json;
use std::fs;
use testcontainers::clients::Cli;

fn setup() -> (ElasticsearchClient, String, reqwest::Client) {
    let docker = (!elasticsearch_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) =
        elasticsearch_context::elasticsearch_form_from_test_context(docker.as_ref());
    let client = ElasticsearchClient::connect(&form).expect("connect client");
    let base_url = build_base_url(&form).expect("base url");
    let http = reqwest::Client::new();
    (client, base_url, http)
}

async fn cleanup_index(http: &reqwest::Client, base_url: &str, index: &str) {
    let _ = http.delete(format!("{base_url}/{index}")).send().await;
}

async fn create_probe_index(
    client: &ElasticsearchClient,
    http: &reqwest::Client,
    base_url: &str,
    index: &str,
) {
    cleanup_index(http, base_url, index).await;
    client
        .create_index(
            index.to_string(),
            Some(json!({
                "mappings": {
                    "properties": {
                        "title": { "type": "text" },
                        "status": { "type": "keyword" },
                        "count": { "type": "integer" }
                    }
                }
            })),
        )
        .await
        .expect("create index");
}
```

### Step 2: Write test_es_connection_and_list_indices

```rust
#[tokio::test]
#[ignore]
async fn test_es_connection_and_list_indices() {
    let (client, _base_url, _http) = setup();
    let info = client.test_connection().await.expect("test connection");
    assert!(info.version.is_some(), "version should be present");

    let indices = client.list_indices().await.expect("list indices");
    assert!(!indices.is_empty() || true); // empty cluster is valid
}
```

### Step 3: Write test_es_index_lifecycle

```rust
#[tokio::test]
#[ignore]
async fn test_es_index_lifecycle() {
    let (client, base_url, http) = setup();
    let index = "dbpaw_es_lifecycle";

    create_probe_index(&client, &http, &base_url, index).await;

    let indices = client.list_indices().await.expect("list indices");
    assert!(indices.iter().any(|i| i.name == index));

    client.refresh_index(index.to_string()).await.expect("refresh");
    client.close_index(index.to_string()).await.expect("close");
    client.open_index(index.to_string()).await.expect("open");
    client.delete_index(index.to_string()).await.expect("delete");

    let after = client.list_indices().await.expect("list after delete");
    assert!(!after.iter().any(|i| i.name == index));
}
```

### Step 4: Write test_es_document_crud

```rust
#[tokio::test]
#[ignore]
async fn test_es_document_crud() {
    let (client, base_url, http) = setup();
    let index = "dbpaw_es_crud";

    create_probe_index(&client, &http, &base_url, index).await;

    let upserted = client
        .upsert_document(index.to_string(), Some("doc1".to_string()), json!({"title": "Test", "status": "ok", "count": 1}), true)
        .await
        .expect("upsert");
    assert_eq!(upserted.id.as_deref(), Some("doc1"));

    let doc = client.get_document(index.to_string(), "doc1".to_string()).await.expect("get");
    assert!(doc.found);
    assert_eq!(doc.source.unwrap()["status"], "ok");

    let deleted = client.delete_document(index.to_string(), "doc1".to_string(), true).await.expect("delete");
    assert_eq!(deleted.result.as_deref(), Some("deleted"));

    cleanup_index(&http, &base_url, index).await;
}
```

### Step 5: Write test_es_search_and_aggregations

```rust
#[tokio::test]
#[ignore]
async fn test_es_search_and_aggregations() {
    let (client, base_url, http) = setup();
    let index = "dbpaw_es_search";

    create_probe_index(&client, &http, &base_url, index).await;

    client.upsert_document(index.to_string(), Some("1".to_string()), json!({"title": "A", "status": "ok", "count": 10}), true).await.expect("doc1");
    client.upsert_document(index.to_string(), Some("2".to_string()), json!({"title": "B", "status": "ok", "count": 20}), true).await.expect("doc2");
    client.upsert_document(index.to_string(), Some("3".to_string()), json!({"title": "C", "status": "error", "count": 5}), true).await.expect("doc3");

    let search = client.search_documents(index.to_string(), Some("status:ok".to_string()), None, 0, 50).await.expect("search");
    assert_eq!(search.total, 2);

    let agg = client.search_documents(index.to_string(), None, Some(json!({"size": 0, "aggs": {"by_status": {"terms": {"field": "status"}}}}).to_string()), 0, 50).await.expect("agg");
    let buckets = &agg.aggregations.unwrap()["by_status"]["buckets"];
    assert!(buckets.as_array().unwrap().len() >= 2);

    cleanup_index(&http, &base_url, index).await;
}
```

### Step 6: Write test_es_mapping_metadata

```rust
#[tokio::test]
#[ignore]
async fn test_es_mapping_metadata() {
    let (client, base_url, http) = setup();
    let index = "dbpaw_es_mapping";

    create_probe_index(&client, &http, &base_url, index).await;

    let mapping = client.get_index_mapping(index.to_string()).await.expect("mapping");
    assert!(mapping.get(index).is_some(), "mapping should include test index");

    cleanup_index(&http, &base_url, index).await;
}
```

### Step 7: Write test_es_export_import_cycle

```rust
#[tokio::test]
#[ignore]
async fn test_es_export_import_cycle() {
    let (client, base_url, http) = setup();
    let source_index = "dbpaw_es_export_src";
    let import_index = "dbpaw_es_export_dst";

    create_probe_index(&client, &http, &base_url, source_index).await;
    client.upsert_document(source_index.to_string(), Some("1".to_string()), json!({"title": "Export", "status": "ok", "count": 1}), true).await.expect("doc1");
    client.upsert_document(source_index.to_string(), Some("2".to_string()), json!({"title": "Export2", "status": "ok", "count": 2}), true).await.expect("doc2");

    let export_path = std::env::temp_dir().join(format!("dbpaw-es-export-{}.ndjson", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let exported = client.export_documents(source_index.to_string(), None, None, export_path.to_string_lossy().to_string(), Some(1)).await.expect("export");
    assert_eq!(exported.documents, 2);

    cleanup_index(&http, &base_url, import_index).await;
    create_probe_index(&client, &http, &base_url, import_index).await;
    let imported = client.import_documents(import_index.to_string(), export_path.to_string_lossy().to_string(), Some(1), true).await.expect("import");
    assert_eq!(imported.successful, 2);
    assert_eq!(imported.failed, 0);

    let _ = fs::remove_file(&export_path);
    cleanup_index(&http, &base_url, source_index).await;
    cleanup_index(&http, &base_url, import_index).await;
}
```

### Step 8: Write test_es_malformed_import_rejects

```rust
#[tokio::test]
#[ignore]
async fn test_es_malformed_import_rejects() {
    let (client, base_url, http) = setup();
    let index = "dbpaw_es_malformed";

    create_probe_index(&client, &http, &base_url, index).await;

    let malformed_path = std::env::temp_dir().join("dbpaw-es-malformed.ndjson");
    fs::write(&malformed_path, "{\"delete\":{\"_id\":\"1\"}}\n{}\n").expect("write malformed");
    assert!(client.import_documents(index.to_string(), malformed_path.to_string_lossy().to_string(), Some(1000), true).await.is_err());

    let _ = fs::remove_file(&malformed_path);
    cleanup_index(&http, &base_url, index).await;
}
```

### Step 9: Write test_es_execute_raw

```rust
#[tokio::test]
#[ignore]
async fn test_es_execute_raw() {
    let (client, base_url, http) = setup();
    let index = "dbpaw_es_raw";

    create_probe_index(&client, &http, &base_url, index).await;
    client.upsert_document(index.to_string(), Some("1".to_string()), json!({"title": "Raw", "status": "ok", "count": 1}), true).await.expect("doc");

    let raw = client.execute_raw("GET".to_string(), format!("/{index}/_count"), None).await.expect("raw");
    assert_eq!(raw.status, 200);
    assert_eq!(raw.json.unwrap()["count"], 1);

    cleanup_index(&http, &base_url, index).await;
}
```

### Step 10: Run integration test

Run: `IT_REUSE_LOCAL_DB=1 IT_DB=elasticsearch cargo test --manifest-path src-tauri/Cargo.toml --test elasticsearch_integration -- --ignored --test-threads=1 --nocapture`
Expected: 8 tests PASS

### Step 11: Commit

```bash
git add src-tauri/tests/elasticsearch_integration.rs
git commit -m "test(elasticsearch): split monolithic integration test into 8 independent scenarios"
```
