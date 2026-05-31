pub mod analyze_table;

use super::types::*;

pub struct PromptRegistry;

impl PromptRegistry {
    pub fn get_prompt_definitions() -> Vec<PromptDefinition> {
        vec![analyze_table::get_definition()]
    }

    pub async fn get_prompt(
        state: &crate::state::AppState,
        name: &str,
        arguments: &serde_json::Value,
    ) -> Result<PromptResponse, String> {
        match name {
            "analyze_table" => analyze_table::execute(state, arguments).await,
            _ => Err(format!("Unknown prompt: {}", name)),
        }
    }
}
