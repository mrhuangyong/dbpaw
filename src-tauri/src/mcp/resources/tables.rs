use super::super::types::*;

pub fn get_definitions() -> Vec<ResourceDefinition> {
    vec![]
}

pub fn get_templates() -> Vec<ResourceTemplate> {
    vec![
        ResourceTemplate {
            uri_template: "dbpaw://connections/{connection_id}/{database}/tables".to_string(),
            name: "table_list".to_string(),
            description: "Table list for a database".to_string(),
            mime_type: "application/json".to_string(),
        },
        ResourceTemplate {
            uri_template: "dbpaw://connections/{connection_id}/{database}/tables/{table}"
                .to_string(),
            name: "table_detail".to_string(),
            description: "Table structure and sample data".to_string(),
            mime_type: "text/markdown".to_string(),
        },
    ]
}

pub async fn read_table_list(
    state: &crate::state::AppState,
    uri: &str,
) -> Result<ResourceContent, String> {
    let path = uri.trim_start_matches("dbpaw://connections/");
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 3 {
        return Err("Invalid URI format".to_string());
    }
    let connection_id: i64 = parts[0].parse().map_err(|_| "Invalid connection_id")?;
    let database = parts[1].to_string();

    let tables = crate::commands::execute_with_retry_from_app_state(
        state,
        connection_id,
        Some(database),
        |driver| async move { driver.list_tables(None).await },
    )
    .await?;

    let json = serde_json::to_string_pretty(&tables).unwrap_or_default();
    Ok(ResourceContent {
        contents: vec![ResourceContentItem {
            uri: uri.to_string(),
            mime_type: Some("application/json".to_string()),
            text: Some(json),
        }],
    })
}

pub async fn read_resource(
    state: &crate::state::AppState,
    uri: &str,
) -> Result<ResourceContent, String> {
    let path = uri.trim_start_matches("dbpaw://connections/");
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 4 {
        return Err("Invalid URI format".to_string());
    }
    let connection_id: i64 = parts[0].parse().map_err(|_| "Invalid connection_id")?;
    let database = parts[1].to_string();
    let table = parts[3].to_string();

    let connections = crate::commands::connection::get_connections_direct(state).await?;
    let conn = connections
        .iter()
        .find(|c| c.id == connection_id)
        .ok_or(format!("Connection {} not found", connection_id))?;
    let schema = super::super::tools::default_schema_for_driver(&conn.db_type);

    let metadata = crate::commands::metadata::get_table_metadata_direct(
        state,
        connection_id,
        Some(database.clone()),
        schema,
        table.clone(),
    )
    .await?;

    let mut md = format!("## {}\n\n", table);
    md.push_str("### Columns\n");
    md.push_str("| Name | Type | Nullable | Default | Primary Key | Comment |\n");
    md.push_str("|------|------|----------|---------|-------------|--------|\n");
    for col in &metadata.columns {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            col.name,
            col.r#type,
            col.nullable,
            col.default_value.as_deref().unwrap_or("-"),
            col.primary_key,
            col.comment.as_deref().unwrap_or("")
        ));
    }

    if !metadata.indexes.is_empty() {
        md.push_str("\n### Indexes\n");
        md.push_str("| Name | Unique | Type | Columns |\n");
        md.push_str("|------|--------|------|--------|\n");
        for idx in &metadata.indexes {
            md.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                idx.name,
                idx.unique,
                idx.index_type.as_deref().unwrap_or("-"),
                idx.columns.join(", ")
            ));
        }
    }

    if !metadata.foreign_keys.is_empty() {
        md.push_str("\n### Foreign Keys\n");
        md.push_str("| Name | Column | Referenced Table | Referenced Column |\n");
        md.push_str("|------|--------|-----------------|------------------|\n");
        for fk in &metadata.foreign_keys {
            md.push_str(&format!(
                "| {} | {} | {}.{} | {} |\n",
                fk.name,
                fk.column,
                fk.referenced_schema.as_deref().unwrap_or(""),
                fk.referenced_table,
                fk.referenced_column
            ));
        }
    }

    Ok(ResourceContent {
        contents: vec![ResourceContentItem {
            uri: uri.to_string(),
            mime_type: Some("text/markdown".to_string()),
            text: Some(md),
        }],
    })
}
