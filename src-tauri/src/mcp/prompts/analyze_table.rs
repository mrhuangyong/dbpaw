use super::super::types::*;

pub fn get_definition() -> PromptDefinition {
    PromptDefinition {
        name: "analyze_table".to_string(),
        description: "Analyze table structure and provide optimization suggestions".to_string(),
        arguments: Some(vec![
            PromptArgument {
                name: "connection_id".to_string(),
                description: "Connection ID".to_string(),
                required: true,
            },
            PromptArgument {
                name: "database".to_string(),
                description: "Database name".to_string(),
                required: true,
            },
            PromptArgument {
                name: "table".to_string(),
                description: "Table name".to_string(),
                required: true,
            },
        ]),
    }
}

pub async fn execute(
    state: &crate::state::AppState,
    arguments: &serde_json::Value,
) -> Result<PromptResponse, String> {
    let connection_id = arguments["connection_id"]
        .as_i64()
        .ok_or("Missing connection_id")?;
    let database = arguments["database"]
        .as_str()
        .ok_or("Missing database")?
        .to_string();
    let table = arguments["table"]
        .as_str()
        .ok_or("Missing table")?
        .to_string();

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

    let mut structure = format!("## {}.{}\n\n", database, table);
    structure.push_str("| Column | Type | Nullable | PK |\n");
    structure.push_str("|--------|------|----------|----|\n");
    for col in &metadata.columns {
        structure.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            col.name, col.r#type, col.nullable, col.primary_key
        ));
    }

    Ok(PromptResponse {
        description: format!("Analyze {}.{} table structure", database, table),
        messages: vec![PromptMessage {
            role: "user".to_string(),
            content: TextContent {
                content_type: "text".to_string(),
                text: format!(
                    "请分析以下表结构并给出优化建议（索引、数据类型、规范化等）：\n\n{}",
                    structure
                ),
            },
        }],
    })
}
