use super::prompts::PromptRegistry;
use super::resources::ResourceRegistry;
use super::tools;
use super::types::*;
use crate::state::AppState;
use std::sync::Arc;

pub struct RequestHandler {
    state: Arc<AppState>,
}

impl RequestHandler {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    pub async fn handle(&self, request: JsonRpcRequest) -> Option<JsonRpcResponse> {
        let id = request.id.clone();

        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize().await,
            "initialized" => Ok(serde_json::json!({})),
            "ping" => Ok(serde_json::json!({})),

            "tools/list" => self.handle_tools_list().await,
            "tools/call" => self.handle_tools_call(request.params).await,

            "resources/list" => self.handle_resources_list().await,
            "resources/read" => self.handle_resources_read(request.params).await,
            "resources/templates/list" => self.handle_resources_templates_list().await,
            "resources/subscribe" => self.handle_resources_subscribe(request.params).await,
            "resources/unsubscribe" => self.handle_resources_unsubscribe(request.params).await,

            "prompts/list" => self.handle_prompts_list().await,
            "prompts/get" => self.handle_prompts_get(request.params).await,

            "sampling/createMessage" => {
                self.handle_sampling_create_message(request.params).await
            }

            "completion/complete" => self.handle_completion_complete(request.params).await,

            _ => {
                return Some(JsonRpcResponse::error(
                    id,
                    METHOD_NOT_FOUND,
                    format!("Method not found: {}", request.method),
                ));
            }
        };

        match result {
            Ok(value) => Some(JsonRpcResponse::success(id, value)),
            Err(e) => Some(JsonRpcResponse::error(id, INTERNAL_ERROR, e)),
        }
    }

    async fn handle_initialize(&self) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({
            "protocolVersion": "2025-03-26",
            "capabilities": {
                "tools": { "listChanged": true },
                "resources": { "subscribe": true, "listChanged": true },
                "prompts": { "listChanged": true },
                "sampling": {},
                "logging": {}
            },
            "serverInfo": {
                "name": "dbpaw",
                "version": "0.5.0"
            }
        }))
    }

    // Tools

    async fn handle_tools_list(&self) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({
            "tools": tools::get_tool_definitions()
        }))
    }

    async fn handle_tools_call(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let params = params.ok_or("Missing params")?;
        let name = params["name"]
            .as_str()
            .ok_or("Missing tool name")?
            .to_string();
        let arguments = params["arguments"].clone();

        let result = tools::execute_tool(&self.state, &name, arguments).await;

        match result {
            Ok(tool_result) => Ok(serde_json::to_value(tool_result).unwrap()),
            Err(e) => Ok(serde_json::to_value(ToolResult::error(e)).unwrap()),
        }
    }

    // Resources

    async fn handle_resources_list(&self) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({
            "resources": ResourceRegistry::get_resource_definitions()
        }))
    }

    async fn handle_resources_read(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let params = params.ok_or("Missing params")?;
        let uri = params["uri"].as_str().ok_or("Missing uri")?;
        let content = ResourceRegistry::read_resource(&self.state, uri).await?;
        Ok(serde_json::to_value(content).unwrap())
    }

    async fn handle_resources_templates_list(&self) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({
            "resourceTemplates": ResourceRegistry::get_resource_templates()
        }))
    }

    async fn handle_resources_subscribe(
        &self,
        _params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({}))
    }

    async fn handle_resources_unsubscribe(
        &self,
        _params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({}))
    }

    // Prompts

    async fn handle_prompts_list(&self) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({
            "prompts": PromptRegistry::get_prompt_definitions()
        }))
    }

    async fn handle_prompts_get(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let params = params.ok_or("Missing params")?;
        let name = params["name"].as_str().ok_or("Missing prompt name")?;
        let arguments = params["arguments"].clone();
        let response = PromptRegistry::get_prompt(&self.state, name, &arguments).await?;
        Ok(serde_json::to_value(response).unwrap())
    }

    // Sampling

    async fn handle_sampling_create_message(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let params = params.ok_or("Missing params")?;
        super::sampling::SamplingHandler::create_message(&params).await
    }

    // Completion

    async fn handle_completion_complete(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let params = params.ok_or("Missing params")?;
        let argument_name = params["argument"]["name"]
            .as_str()
            .ok_or("Missing argument name")?;
        let argument_value = params["argument"]["value"].as_str().unwrap_or("");

        let values: Vec<String> = match argument_name {
            "connection_id" => {
                let connections =
                    crate::commands::connection::get_connections_direct(&self.state).await?;
                connections
                    .iter()
                    .filter(|c| c.id.to_string().starts_with(argument_value))
                    .map(|c| c.id.to_string())
                    .collect()
            }
            _ => vec![],
        };

        Ok(serde_json::json!({
            "completion": {
                "values": values,
                "hasMore": false
            }
        }))
    }
}
