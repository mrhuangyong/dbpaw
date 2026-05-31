pub mod connections;
pub mod tables;

use super::types::*;

pub struct ResourceRegistry;

impl ResourceRegistry {
    pub fn get_resource_definitions() -> Vec<ResourceDefinition> {
        let mut resources = Vec::new();
        resources.extend(connections::get_definitions());
        resources.extend(tables::get_definitions());
        resources
    }

    pub fn get_resource_templates() -> Vec<ResourceTemplate> {
        let mut templates = Vec::new();
        templates.extend(connections::get_templates());
        templates.extend(tables::get_templates());
        templates
    }

    pub async fn read_resource(
        state: &crate::state::AppState,
        uri: &str,
    ) -> Result<ResourceContent, String> {
        if uri.starts_with("dbpaw://connections") {
            if uri.contains("/tables/") && !uri.ends_with("/tables") {
                tables::read_resource(state, uri).await
            } else if uri.ends_with("/tables") || uri.contains("/tables") && !uri.contains("/tables/") {
                tables::read_table_list(state, uri).await
            } else if uri.ends_with("/databases") {
                connections::read_databases(state, uri).await
            } else if uri == "dbpaw://connections" {
                connections::read_all(state, uri).await
            } else {
                connections::read_one(state, uri).await
            }
        } else {
            Err(format!("Unknown resource URI: {}", uri))
        }
    }
}
