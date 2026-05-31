use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn get_mcp_binary() -> String {
    // Use CARGO_MANIFEST_DIR to find the binary relative to src-tauri
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{}/target/debug/dbpaw-mcp", manifest_dir)
}

fn send_request(proc: &mut std::process::Child, request: &str) -> String {
    let stdin = proc.stdin.as_mut().unwrap();
    stdin.write_all(request.as_bytes()).unwrap();
    stdin.write_all(b"\n").unwrap();
    stdin.flush().unwrap();

    let stdout = proc.stdout.as_mut().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    line
}

#[test]
fn test_mcp_initialize() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(v["jsonrpc"], "2.0");
    assert_eq!(v["id"], 1);
    assert_eq!(v["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(v["result"]["serverInfo"]["name"], "dbpaw");

    proc.kill().unwrap();
}

#[test]
fn test_mcp_tools_list() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    // Initialize first
    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    // List tools
    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    let tools = v["result"]["tools"].as_array().unwrap();
    assert!(tools.len() >= 7, "Expected at least 7 tools");

    let tool_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(tool_names.contains(&"dbpaw_list_connections"));
    assert!(tool_names.contains(&"dbpaw_list_databases"));
    assert!(tool_names.contains(&"dbpaw_list_tables"));
    assert!(tool_names.contains(&"dbpaw_describe_table"));
    assert!(tool_names.contains(&"dbpaw_get_ddl"));
    assert!(tool_names.contains(&"dbpaw_get_schema_context"));
    assert!(tool_names.contains(&"dbpaw_execute_query"));

    proc.kill().unwrap();
}

#[test]
fn test_mcp_sql_safety_drop_blocked() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"dbpaw_execute_query","arguments":{"connection_id":1,"sql":"DROP TABLE users"}}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(v["result"]["isError"], true);
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Dangerous keyword"));

    proc.kill().unwrap();
}

#[test]
fn test_mcp_sql_safety_insert_blocked() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"dbpaw_execute_query","arguments":{"connection_id":1,"sql":"INSERT INTO users VALUES (1)"}}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(v["result"]["isError"], true);
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Write operation"));

    proc.kill().unwrap();
}

#[test]
fn test_mcp_sql_safety_multiple_statements_blocked() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"dbpaw_execute_query","arguments":{"connection_id":1,"sql":"SELECT 1; DROP TABLE users"}}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(v["result"]["isError"], true);
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Multiple statements"));

    proc.kill().unwrap();
}

#[test]
fn test_mcp_invalid_tool() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"nonexistent_tool","arguments":{}}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    // Should return error
    assert!(v.get("error").is_some() || v["result"]["isError"] == true);

    proc.kill().unwrap();
}

#[test]
fn test_mcp_ping() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(v["jsonrpc"], "2.0");
    assert_eq!(v["id"], 2);
    assert!(v.get("error").is_none(), "ping should not return an error");
    assert_eq!(v["result"], serde_json::json!({}));

    proc.kill().unwrap();
}

#[test]
fn test_mcp_initialized_notification() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    // Send initialized notification (no id field — it's a notification, not a request)
    let stdin = proc.stdin.as_mut().unwrap();
    stdin.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n").unwrap();
    stdin.flush().unwrap();

    // Read notification response (server responds even to notifications)
    let stdout = proc.stdout.as_mut().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut notification_response = String::new();
    reader.read_line(&mut notification_response).unwrap();

    // Verify server is still alive by sending a ping
    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(v["jsonrpc"], "2.0");
    assert_eq!(v["id"], 2);
    assert!(v.get("error").is_none(), "server should still be alive after notification");
    assert_eq!(v["result"], serde_json::json!({}));

    proc.kill().unwrap();
}

#[test]
fn test_mcp_method_not_found() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"unknown_method"}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(v["jsonrpc"], "2.0");
    assert_eq!(v["id"], 2);
    assert!(v.get("error").is_some(), "should return an error for unknown method");
    assert_eq!(v["error"]["code"], -32601, "error code should be -32601 (Method not found)");

    proc.kill().unwrap();
}
