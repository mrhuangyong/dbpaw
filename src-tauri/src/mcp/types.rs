use serde::{Deserialize, Serialize};

// JSON-RPC 2.0 基础类型

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

// MCP 协议特定类型

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDefinition {
    pub uri: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptDefinition {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<PromptArgument>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    pub name: String,
    pub description: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Vec<TextContent>,
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

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

// 错误码常量
pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

impl JsonRpcResponse {
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<serde_json::Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }
}

impl ToolResult {
    pub fn text(text: String) -> Self {
        Self {
            content: vec![TextContent {
                content_type: "text".to_string(),
                text,
            }],
            is_error: None,
        }
    }

    pub fn error(text: String) -> Self {
        Self {
            content: vec![TextContent {
                content_type: "text".to_string(),
                text,
            }],
            is_error: Some(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn error_codes_match_jsonrpc_spec() {
        assert_eq!(PARSE_ERROR, -32700);
        assert_eq!(INVALID_REQUEST, -32600);
        assert_eq!(METHOD_NOT_FOUND, -32601);
        assert_eq!(INVALID_PARAMS, -32602);
        assert_eq!(INTERNAL_ERROR, -32603);
    }
}
