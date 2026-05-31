use super::super::types::*;

pub fn get_definitions() -> Vec<ResourceDefinition> {
    vec![
        ResourceDefinition {
            uri: "dbpaw://connections".to_string(),
            name: "connections".to_string(),
            description: "List all saved database connections".to_string(),
            mime_type: "application/json".to_string(),
        },
    ]
}

pub fn get_templates() -> Vec<ResourceTemplate> {
    vec![
        ResourceTemplate {
            uri_template: "dbpaw://connections/{connection_id}".to_string(),
            name: "connection_detail".to_string(),
            description: "Single connection details".to_string(),
            mime_type: "application/json".to_string(),
        },
        ResourceTemplate {
            uri_template: "dbpaw://connections/{connection_id}/databases".to_string(),
            name: "databases".to_string(),
            description: "Database list for a connection".to_string(),
            mime_type: "application/json".to_string(),
        },
    ]
}

pub async fn read_all(
    state: &crate::state::AppState,
    uri: &str,
) -> Result<ResourceContent, String> {
    let connections = crate::commands::connection::get_connections_direct(state).await?;
    let json = serde_json::to_string_pretty(&connections).unwrap_or_default();
    Ok(ResourceContent {
        contents: vec![ResourceContentItem {
            uri: uri.to_string(),
            mime_type: Some("application/json".to_string()),
            text: Some(json),
        }],
    })
}

pub async fn read_one(
    state: &crate::state::AppState,
    uri: &str,
) -> Result<ResourceContent, String> {
    let id_str = uri.trim_start_matches("dbpaw://connections/");
    let connection_id: i64 = id_str.parse().map_err(|_| "Invalid connection_id")?;

    let connections = crate::commands::connection::get_connections_direct(state).await?;
    let conn = connections
        .iter()
        .find(|c| c.id == connection_id)
        .ok_or(format!("Connection {} not found", connection_id))?;

    let json = serde_json::to_string_pretty(conn).unwrap_or_default();
    Ok(ResourceContent {
        contents: vec![ResourceContentItem {
            uri: uri.to_string(),
            mime_type: Some("application/json".to_string()),
            text: Some(json),
        }],
    })
}

pub async fn read_databases(
    state: &crate::state::AppState,
    uri: &str,
) -> Result<ResourceContent, String> {
    let path = uri.trim_start_matches("dbpaw://connections/");
    let id_str = path.trim_end_matches("/databases");
    let connection_id: i64 = id_str.parse().map_err(|_| "Invalid connection_id")?;

    let databases =
        crate::commands::connection::list_databases_by_id_direct(state, connection_id).await?;
    let json = serde_json::to_string_pretty(&databases).unwrap_or_default();
    Ok(ResourceContent {
        contents: vec![ResourceContentItem {
            uri: uri.to_string(),
            mime_type: Some("application/json".to_string()),
            text: Some(json),
        }],
    })
}
