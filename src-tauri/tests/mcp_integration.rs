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
    assert_eq!(v["result"]["protocolVersion"], "2025-03-26");
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

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"ping","params":{}}"#);
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
    stdin.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"initialized\"}\n").unwrap();
    stdin.flush().unwrap();

    // Read notification response (server responds even to notifications)
    let stdout = proc.stdout.as_mut().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut notification_response = String::new();
    reader.read_line(&mut notification_response).unwrap();

    // Verify server is still alive by sending a ping
    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"ping","params":{}}"#);
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

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"unknown_method","params":{}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(v["jsonrpc"], "2.0");
    assert_eq!(v["id"], 2);
    assert!(v.get("error").is_some(), "should return an error for unknown method");
    assert_eq!(v["error"]["code"], -32601, "error code should be -32601 (Method not found)");

    proc.kill().unwrap();
}

#[test]
fn test_mcp_resources_list() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"resources/list","params":{}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    let resources = v["result"]["resources"].as_array().unwrap();
    assert!(resources.len() >= 1, "Expected at least 1 resource");

    let names: Vec<&str> = resources.iter().map(|r| r["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"connections"), "Should contain 'connections' resource");

    proc.kill().unwrap();
}

#[test]
fn test_mcp_resources_templates_list() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"resources/templates/list","params":{}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    let templates = v["result"]["resourceTemplates"].as_array().unwrap();
    assert!(templates.len() >= 2, "Expected at least 2 resource templates");

    let names: Vec<&str> = templates.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"table_list"), "Should contain 'table_list' template");
    assert!(names.contains(&"table_detail"), "Should contain 'table_detail' template");

    proc.kill().unwrap();
}

#[test]
fn test_mcp_resources_read_connections() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"resources/read","params":{"uri":"dbpaw://connections"}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    // The response should have either a valid contents array or an error
    // (error occurs when the binary has no configured database)
    assert!(
        v["result"]["contents"].is_array() || v.get("error").is_some(),
        "Should have contents array or error response"
    );

    proc.kill().unwrap();
}

#[test]
fn test_mcp_resources_read_invalid_uri() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"resources/read","params":{"uri":"invalid://unknown"}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert!(v.get("error").is_some() || v["result"]["isError"] == true, "Should return error for invalid URI");

    proc.kill().unwrap();
}

#[test]
fn test_mcp_prompts_list() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"prompts/list","params":{}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    let prompts = v["result"]["prompts"].as_array().unwrap();
    let prompt_names: Vec<&str> = prompts.iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert!(prompt_names.contains(&"analyze_table"), "Should contain 'analyze_table' prompt");

    proc.kill().unwrap();
}

#[test]
fn test_mcp_prompts_get_unknown() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"prompts/get","params":{"name":"nonexistent_prompt","arguments":{}}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert!(v.get("error").is_some() || v["result"]["isError"] == true, "Should return error for unknown prompt");

    proc.kill().unwrap();
}

#[test]
fn test_mcp_prompts_get_missing_params() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"prompts/get"}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert!(v.get("error").is_some() || v["result"]["isError"] == true, "Should return error when params missing");

    proc.kill().unwrap();
}

#[test]
fn test_mcp_completion_complete() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"completion/complete","params":{"ref":{"type":"ref/prompt","name":"analyze_table"},"argument":{"name":"connection_id","value":""}}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(v["jsonrpc"], "2.0");
    assert_eq!(v["id"], 2);
    assert!(
        v["result"]["completion"]["values"].is_array() || v.get("error").is_some() || v["result"]["isError"] == true,
        "Should have completion.values array or error response"
    );

    proc.kill().unwrap();
}

#[test]
fn test_mcp_completion_unknown_arg() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"completion/complete","params":{"ref":{"type":"ref/prompt","name":"analyze_table"},"argument":{"name":"nonexistent_arg","value":"foo"}}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(v["jsonrpc"], "2.0");
    assert_eq!(v["id"], 2);
    let values = v["result"]["completion"]["values"].as_array().unwrap();
    assert!(values.is_empty(), "Values should be empty for unknown argument");

    proc.kill().unwrap();
}

#[test]
fn test_mcp_sampling_create_message() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"sampling/createMessage","params":{"messages":[{"role":"user","content":{"type":"text","text":"hello"}}]}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert!(v.get("error").is_some() || v["result"]["isError"] == true, "Should return error for unsupported sampling");

    proc.kill().unwrap();
}

#[test]
fn test_mcp_sql_safety_truncate_blocked() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"dbpaw_execute_query","arguments":{"connection_id":1,"sql":"TRUNCATE TABLE users"}}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(v["result"]["isError"], true);
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Dangerous keyword"));

    proc.kill().unwrap();
}

#[test]
fn test_mcp_sql_safety_alter_blocked() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"dbpaw_execute_query","arguments":{"connection_id":1,"sql":"ALTER TABLE users ADD COLUMN age INT"}}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(v["result"]["isError"], true);
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Dangerous keyword"));

    proc.kill().unwrap();
}

#[test]
fn test_mcp_sql_safety_create_blocked() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"dbpaw_execute_query","arguments":{"connection_id":1,"sql":"CREATE TABLE test (id INT)"}}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(v["result"]["isError"], true);
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Dangerous keyword"));

    proc.kill().unwrap();
}

#[test]
fn test_mcp_sql_safety_update_no_where_blocked() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"dbpaw_execute_query","arguments":{"connection_id":1,"sql":"UPDATE users SET name = 'test'"}}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(v["result"]["isError"], true);
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Write operation"), "UPDATE without WHERE should be blocked as write operation");

    proc.kill().unwrap();
}

#[test]
fn test_mcp_sql_safety_delete_no_where_blocked() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"dbpaw_execute_query","arguments":{"connection_id":1,"sql":"DELETE FROM users"}}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(v["result"]["isError"], true);
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Write operation"), "DELETE without WHERE should be blocked as write operation");

    proc.kill().unwrap();
}

#[test]
fn test_mcp_sql_safety_select_where_allowed() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"dbpaw_execute_query","arguments":{"connection_id":1,"sql":"SELECT * FROM users WHERE id = 1"}}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(!text.contains("Dangerous keyword"), "SELECT with WHERE should not be blocked by safety checks");
    assert!(!text.contains("Write operation"), "SELECT with WHERE should not be blocked by safety checks");
    assert!(!text.contains("Multiple statements"), "SELECT with WHERE should not be blocked by safety checks");

    proc.kill().unwrap();
}

#[test]
fn test_mcp_tools_call_missing_params() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"tools/call"}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert!(v.get("error").is_some() || v["result"]["isError"] == true, "Should return error when params missing");

    proc.kill().unwrap();
}

#[test]
fn test_mcp_tools_call_missing_name() {
    let mut proc = Command::new(get_mcp_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    send_request(&mut proc, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    let response = send_request(&mut proc, r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"arguments":{}}}"#);
    let v: Value = serde_json::from_str(&response).unwrap();

    assert!(v.get("error").is_some() || v["result"]["isError"] == true, "Should return error when name missing");

    proc.kill().unwrap();
}
