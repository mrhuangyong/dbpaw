use super::{conn_failed_error, DatabaseDriver};
use crate::models::{
    ColumnInfo, ColumnSchema, ConnectionForm, ForeignKeyInfo, IndexInfo, QueryColumn, QueryResult,
    RoutineInfo, SchemaForeignKey, SchemaOverview, SequenceInfo, SingleResultSet,
    TableDataResponse, TableInfo, TableMetadata, TableSchema, TableStructure,
};
use async_trait::async_trait;
use odbc_api::{ConnectionOptions, Cursor, Environment, ResultSetMetadata};
use std::collections::HashMap;

pub struct Db2Driver {
    config: Db2Config,
    _ssh_tunnel: Option<crate::ssh::SshTunnel>,
}

#[derive(Clone)]
struct Db2Config {
    host: String,
    port: u16,
    database: String,
    username: String,
    password: String,
}

fn odbc_escape_value(v: &str) -> String {
    if v.contains(';') || v.contains('{') || v.contains('}') || v.contains('[') {
        format!("{{{}}}", v.replace('}', "}}"))
    } else {
        v.to_string()
    }
}

fn build_connection_string(cfg: &Db2Config) -> String {
    format!(
        "DRIVER={{IBM DB2 ODBC DRIVER}};DATABASE={};HOSTNAME={};PORT={};PROTOCOL=TCPIP;UID={};PWD={};",
        cfg.database, cfg.host, cfg.port, odbc_escape_value(&cfg.username), odbc_escape_value(&cfg.password)
    )
}

fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

fn escape_literal(value: &str) -> String {
    value.replace('\'', "''")
}

fn odbc_value_to_json(row: &mut odbc_api::CursorRow<'_>, col_idx: u16) -> serde_json::Value {
    let mut buf = Vec::new();
    match row.get_text(col_idx, &mut buf) {
        Ok(true) => {
            let s = String::from_utf8_lossy(&buf).to_string();
            if s.is_empty() {
                return serde_json::Value::String(s);
            }
            if let Ok(v) = s.parse::<i64>() {
                return serde_json::Value::Number(v.into());
            }
            if let Ok(v) = s.parse::<f64>() {
                if let Some(n) = serde_json::Number::from_f64(v) {
                    return serde_json::Value::Number(n);
                }
                return serde_json::Value::String(s);
            }
            serde_json::Value::String(s)
        }
        Ok(false) => serde_json::Value::Null,
        Err(_) => serde_json::Value::Null,
    }
}

fn collect_cursor_data(
    mut cursor: odbc_api::CursorImpl<odbc_api::handles::StatementImpl<'_>>,
) -> Result<(Vec<String>, Vec<serde_json::Value>), String> {
    let num_cols = cursor
        .num_result_cols()
        .map_err(|e| format!("[QUERY_ERROR] {e}"))? as u16;
    let mut col_names = Vec::with_capacity(num_cols as usize);
    for i in 1..=num_cols {
        let name = cursor
            .col_name(i)
            .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
        col_names.push(name);
    }

    let mut rows = Vec::new();
    while let Some(mut row) = cursor.next_row().map_err(|e| format!("[QUERY_ERROR] {e}"))? {
        let mut map = serde_json::Map::new();
        for (i, name) in col_names.iter().enumerate() {
            map.insert(name.clone(), odbc_value_to_json(&mut row, (i + 1) as u16));
        }
        rows.push(serde_json::Value::Object(map));
    }
    Ok((col_names, rows))
}

impl Db2Driver {
    pub async fn connect(form: &ConnectionForm) -> Result<Self, String> {
        let mut effective_form = form.clone();
        let mut ssh_tunnel = None;

        if let Some(true) = form.ssh_enabled {
            let tunnel = crate::ssh::start_ssh_tunnel(form)?;
            effective_form.host = Some("127.0.0.1".to_string());
            effective_form.port = Some(tunnel.local_port as i64);
            ssh_tunnel = Some(tunnel);
        }

        let host = effective_form
            .host
            .clone()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .ok_or("[VALIDATION_ERROR] host cannot be empty")?;
        let port = effective_form.port.unwrap_or(50000);
        if !(1..=65535).contains(&port) {
            return Err("[VALIDATION_ERROR] port out of range".to_string());
        }
        let database = effective_form
            .database
            .clone()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .ok_or("[VALIDATION_ERROR] database cannot be empty")?;
        let username = effective_form
            .username
            .clone()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .ok_or("[VALIDATION_ERROR] username cannot be empty")?;
        let password = effective_form.password.clone().unwrap_or_default();

        let config = Db2Config {
            host,
            port: port as u16,
            database,
            username,
            password,
        };
        let driver = Self {
            config,
            _ssh_tunnel: ssh_tunnel,
        };
        driver.test_connection().await?;
        Ok(driver)
    }

    async fn run_blocking<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(odbc_api::Connection<'_>) -> Result<T, String> + Send + 'static,
        T: Send + 'static,
    {
        let cfg = self.config.clone();
        tokio::task::spawn_blocking(move || {
            let conn_string = build_connection_string(&cfg);
            let env = Environment::new().map_err(|e| conn_failed_error(&e))?;
            let conn = env
                .connect_with_connection_string(&conn_string, ConnectionOptions::default())
                .map_err(|e| conn_failed_error(&e))?;
            f(conn)
        })
        .await
        .map_err(|e| format!("[DB2_ERROR] {e}"))?
    }
}

#[async_trait]
impl DatabaseDriver for Db2Driver {
    async fn test_connection(&self) -> Result<(), String> {
        self.run_blocking(|conn| {
            let cursor = conn
                .execute("SELECT 1 FROM SYSIBM.SYSDUMMY1", ())
                .map_err(|e| conn_failed_error(&e))?;
            if cursor.is_none() {
                return Err(conn_failed_error(&"Empty response from SYSIBM.SYSDUMMY1".to_string()));
            }
            Ok(())
        })
        .await
    }

    async fn list_databases(&self) -> Result<Vec<String>, String> {
        self.run_blocking(|conn| {
            let cursor = conn
                .execute("SELECT CURRENT_SERVER FROM SYSIBM.SYSDUMMY1", ())
                .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            match cursor {
                Some(c) => {
                    let (_, rows) = collect_cursor_data(c)?;
                    let mut result = Vec::new();
                    for row in &rows {
                        if let Some(val) = row.as_str() {
                            result.push(val.to_string());
                        }
                    }
                    Ok(result)
                }
                None => Ok(vec![]),
            }
        })
        .await
    }

    async fn list_tables(&self, schema: Option<String>) -> Result<Vec<TableInfo>, String> {
        let schema_upper = schema
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty());
        self.run_blocking(move |conn| {
            let sql = if let Some(ref s) = schema_upper {
                format!(
                    "SELECT TABSCHEMA, TABNAME, \
                     CASE WHEN TYPE = 'V' THEN 'view' ELSE 'table' END \
                     FROM SYSCAT.TABLES \
                     WHERE TABSCHEMA = '{}' AND TYPE IN ('T', 'V') \
                     ORDER BY TABSCHEMA, TABNAME",
                    escape_literal(s)
                )
            } else {
                "SELECT TABSCHEMA, TABNAME, \
                 CASE WHEN TYPE = 'V' THEN 'view' ELSE 'table' END \
                 FROM SYSCAT.TABLES \
                 WHERE TYPE IN ('T', 'V') \
                 ORDER BY TABSCHEMA, TABNAME"
                    .to_string()
            };
            let cursor = conn
                .execute(&sql, ())
                .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            let mut result = Vec::new();
            if let Some(c) = cursor {
                let (_, rows) = collect_cursor_data(c)?;
                for row in &rows {
                    if let Some(arr) = row.as_array() {
                        let schema_name = arr.first().and_then(|v| v.as_str()).unwrap_or("");
                        let table_name = arr.get(1).and_then(|v| v.as_str()).unwrap_or("");
                        let table_type = arr.get(2).and_then(|v| v.as_str()).unwrap_or("table");
                        if !schema_name.is_empty() && !table_name.is_empty() {
                            result.push(TableInfo {
                                schema: schema_name.to_string(),
                                name: table_name.to_string(),
                                r#type: table_type.to_string(),
                            });
                        }
                    }
                }
            }
            Ok(result)
        })
        .await
    }

    async fn list_routines(&self, schema: Option<String>) -> Result<Vec<RoutineInfo>, String> {
        let schema_upper = schema
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty());
        self.run_blocking(move |conn| {
            let sql = if let Some(ref s) = schema_upper {
                format!(
                    "SELECT ROUTINESCHEMA, ROUTINENAME, ROUTINETYPE \
                     FROM SYSCAT.ROUTINES \
                     WHERE ROUTINESCHEMA = '{}' \
                     ORDER BY ROUTINESCHEMA, ROUTINENAME",
                    escape_literal(s)
                )
            } else {
                "SELECT ROUTINESCHEMA, ROUTINENAME, ROUTINETYPE \
                 FROM SYSCAT.ROUTINES \
                 ORDER BY ROUTINESCHEMA, ROUTINENAME"
                    .to_string()
            };
            let cursor = conn
                .execute(&sql, ())
                .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            let mut result = Vec::new();
            if let Some(c) = cursor {
                let (_, rows) = collect_cursor_data(c)?;
                for row in &rows {
                    if let Some(arr) = row.as_array() {
                        let schema_name = arr.first().and_then(|v| v.as_str()).unwrap_or("");
                        let routine_name = arr.get(1).and_then(|v| v.as_str()).unwrap_or("");
                        let routine_type = arr.get(2).and_then(|v| v.as_str()).unwrap_or("");
                        if !schema_name.is_empty() && !routine_name.is_empty() {
                            result.push(RoutineInfo {
                                schema: schema_name.to_string(),
                                name: routine_name.to_string(),
                                r#type: routine_type.to_string(),
                            });
                        }
                    }
                }
            }
            Ok(result)
        })
        .await
    }

    async fn list_sequences(&self, schema: Option<String>) -> Result<Vec<SequenceInfo>, String> {
        let schema_upper = schema
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty());
        self.run_blocking(move |conn| {
            let sql = if let Some(ref s) = schema_upper {
                format!(
                    "SELECT SEQSCHEMA, SEQNAME, DATA_TYPE, CAST(START AS VARCHAR(64)), CAST(INCREMENT AS VARCHAR(64)) \
                     FROM SYSCAT.SEQUENCES \
                     WHERE SEQSCHEMA = '{}' \
                     ORDER BY SEQSCHEMA, SEQNAME",
                    escape_literal(s)
                )
            } else {
                "SELECT SEQSCHEMA, SEQNAME, DATA_TYPE, CAST(START AS VARCHAR(64)), CAST(INCREMENT AS VARCHAR(64)) \
                 FROM SYSCAT.SEQUENCES \
                 ORDER BY SEQSCHEMA, SEQNAME"
                    .to_string()
            };
            let cursor = conn
                .execute(&sql, ())
                .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            let mut result = Vec::new();
            if let Some(c) = cursor {
                let (_, rows) = collect_cursor_data(c)?;
                for row in &rows {
                    if let Some(arr) = row.as_array() {
                        let schema_name = arr.first().and_then(|v| v.as_str()).unwrap_or("");
                        let seq_name = arr.get(1).and_then(|v| v.as_str()).unwrap_or("");
                        let data_type = arr.get(2).and_then(|v| v.as_str()).unwrap_or("");
                        let start_value = arr.get(3).and_then(|v| v.as_str()).unwrap_or("");
                        let increment = arr.get(4).and_then(|v| v.as_str()).unwrap_or("");
                        if !schema_name.is_empty() && !seq_name.is_empty() {
                            result.push(SequenceInfo {
                                schema: schema_name.to_string(),
                                name: seq_name.to_string(),
                                data_type: data_type.to_string(),
                                start_value: Some(start_value.to_string()),
                                increment: Some(increment.to_string()),
                            });
                        }
                    }
                }
            }
            Ok(result)
        })
        .await
    }

    async fn get_routine_ddl(
        &self,
        schema: String,
        name: String,
        _routine_type: String,
    ) -> Result<String, String> {
        self.run_blocking(move |conn| {
            let sql = format!(
                "SELECT TEXT FROM SYSCAT.ROUTINES \
                 WHERE ROUTINESCHEMA = '{}' AND ROUTINENAME = '{}'",
                escape_literal(&schema),
                escape_literal(&name)
            );
            let cursor = conn
                .execute(&sql, ())
                .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            if let Some(c) = cursor {
                let (_, rows) = collect_cursor_data(c)?;
                if let Some(row) = rows.first() {
                    if let Some(text) = row.as_str() {
                        return Ok(text.trim().to_string());
                    }
                }
            }
            Err("[QUERY_ERROR] Routine not found".to_string())
        })
        .await
    }

    async fn get_table_structure(
        &self,
        schema: String,
        table: String,
    ) -> Result<TableStructure, String> {
        self.run_blocking(move |conn| {
            // Primary keys
            let pk_sql = format!(
                "SELECT COLNAME \
                 FROM SYSCAT.KEYCOLUSE \
                 WHERE TABSCHEMA = '{}' AND TABNAME = '{}' \
                 ORDER BY COLSEQ",
                escape_literal(&schema),
                escape_literal(&table)
            );
            let pk_cursor = conn
                .execute(&pk_sql, ())
                .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            let mut pk_set = std::collections::HashSet::<String>::new();
            if let Some(c) = pk_cursor {
                let (_, pk_rows) = collect_cursor_data(c)?;
                for row in &pk_rows {
                    if let Some(col) = row.as_str() {
                        pk_set.insert(col.to_string());
                    }
                }
            }

            // Columns
            let col_sql = format!(
                "SELECT COLNAME, TYPENAME, LENGTH, SCALE, NULLS, DEFAULT, IDENTITY, REMARKS \
                 FROM SYSCAT.COLUMNS \
                 WHERE TABSCHEMA = '{}' AND TABNAME = '{}' \
                 ORDER BY COLNO",
                escape_literal(&schema),
                escape_literal(&table)
            );
            let col_cursor = conn
                .execute(&col_sql, ())
                .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            let mut columns = Vec::new();
            if let Some(c) = col_cursor {
                let (_, col_rows) = collect_cursor_data(c)?;
                for row in &col_rows {
                    if let Some(arr) = row.as_array() {
                        let col_name = arr.first().and_then(|v| v.as_str()).unwrap_or("");
                        let type_name = arr.get(1).and_then(|v| v.as_str()).unwrap_or("");
                        let length: i64 = arr
                            .get(2)
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        let scale: i64 = arr
                            .get(3)
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        let nulls = arr.get(4).and_then(|v| v.as_str()).unwrap_or("Y");
                        let default_val = arr.get(5).and_then(|v| v.as_str());
                        let identity = arr.get(6).and_then(|v| v.as_str()).unwrap_or("");
                        let comment = arr.get(7).and_then(|v| v.as_str());

                        if col_name.is_empty() {
                            continue;
                        }

                        let col_type = format_db2_type(type_name, length, scale);
                        let is_nullable = nulls != "N";
                        let is_identity = !identity.is_empty() && identity != " ";
                        let default_value = default_val
                            .map(|d| d.trim().to_string())
                            .filter(|d| !d.is_empty());
                        let comment_val = comment
                            .map(|c| c.trim().to_string())
                            .filter(|c| !c.is_empty());

                        let mut extra = String::new();
                        if is_identity {
                            extra.push_str(" GENERATED ALWAYS AS IDENTITY");
                        }

                        columns.push(ColumnInfo {
                            name: col_name.to_string(),
                            r#type: if extra.is_empty() {
                                col_type
                            } else {
                                format!("{}{}", col_type, extra)
                            },
                            nullable: is_nullable,
                            default_value,
                            primary_key: pk_set.contains(col_name),
                            comment: comment_val,
                            default_constraint_name: None,
                        });
                    }
                }
            }
            Ok(TableStructure { columns })
        })
        .await
    }

    async fn get_table_metadata(
        &self,
        schema: String,
        table: String,
    ) -> Result<TableMetadata, String> {
        let columns = self
            .get_table_structure(schema.clone(), table.clone())
            .await?
            .columns;

        let (indexes, foreign_keys) = self
            .run_blocking(move |conn| {
                // Indexes
                let idx_sql = format!(
                    "SELECT i.INDNAME, \
                     CASE WHEN i.UNIQUERULE = 'U' OR i.UNIQUERULE = 'P' THEN 1 ELSE 0 END, \
                     i.INDEXTYPE, ic.COLNAME, ic.COLSEQ \
                     FROM SYSCAT.INDEXES i \
                     JOIN SYSCAT.INDEXCOLUSE ic \
                       ON ic.INDNAME = i.INDNAME AND ic.INDSCHEMA = i.INDSCHEMA \
                     WHERE i.TABSCHEMA = '{}' AND i.TABNAME = '{}' \
                     ORDER BY i.INDNAME, ic.COLSEQ",
                    escape_literal(&schema),
                    escape_literal(&table)
                );
                let idx_cursor = conn
                    .execute(&idx_sql, ())
                    .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
                let mut idx_map: HashMap<String, (bool, Option<String>, Vec<(i64, String)>)> =
                    HashMap::new();
                if let Some(c) = idx_cursor {
                    let (_, idx_rows) = collect_cursor_data(c)?;
                    for row in &idx_rows {
                        if let Some(arr) = row.as_array() {
                            let idx_name = arr.first().and_then(|v| v.as_str()).unwrap_or("");
                            let is_unique: i64 = arr
                                .get(1)
                                .and_then(|v| v.as_str())
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(0);
                            let idx_type = arr.get(2).and_then(|v| v.as_str());
                            let col_name = arr.get(3).and_then(|v| v.as_str()).unwrap_or("");
                            let position: i64 = arr
                                .get(4)
                                .and_then(|v| v.as_str())
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(0);

                            if !idx_name.is_empty() && !col_name.is_empty() {
                                let entry = idx_map
                                    .entry(idx_name.to_string())
                                    .or_insert((false, idx_type.map(|s| s.to_string()), Vec::new()));
                                entry.0 = is_unique == 1;
                                if entry.1.is_none() {
                                    entry.1 = idx_type.map(|s| s.to_string());
                                }
                                entry.2.push((position, col_name.to_string()));
                            }
                        }
                    }
                }
                let mut indexes: Vec<IndexInfo> = idx_map
                    .into_iter()
                    .map(|(name, (unique, index_type, mut cols))| {
                        cols.sort_by_key(|(pos, _)| *pos);
                        IndexInfo {
                            name,
                            unique,
                            index_type,
                            columns: cols.into_iter().map(|(_, c)| c).collect(),
                        }
                    })
                    .collect();
                indexes.sort_by(|a, b| a.name.cmp(&b.name));

                // Foreign keys
                let fk_sql = format!(
                    "SELECT fk.CONSTNAME, fk.COLNAME, \
                     reftab.TABSCHEMA, reftab.TABNAME, refcol.COLNAME, fk.DELETERULE \
                     FROM SYSCAT.REFERENCES fk \
                     JOIN SYSCAT.TABLES reftab \
                       ON reftab.TABNAME = fk.REFTABNAME AND reftab.TABSCHEMA = fk.REFTABSCHEMA \
                     JOIN SYSCAT.KEYCOLUSE refcol \
                       ON refcol.CONSTNAME = fk.REFKEYNAME AND refcol.TABSCHEMA = fk.REFTABSCHEMA \
                     WHERE fk.TABSCHEMA = '{}' AND fk.TABNAME = '{}' \
                     ORDER BY fk.CONSTNAME, refcol.COLSEQ",
                    escape_literal(&schema),
                    escape_literal(&table)
                );
                let fk_cursor = conn
                    .execute(&fk_sql, ())
                    .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
                let mut foreign_keys = Vec::new();
                if let Some(c) = fk_cursor {
                    let (_, fk_rows) = collect_cursor_data(c)?;
                    for row in &fk_rows {
                        if let Some(arr) = row.as_array() {
                            let fk_name = arr.first().and_then(|v| v.as_str()).unwrap_or("");
                            let col_name = arr.get(1).and_then(|v| v.as_str()).unwrap_or("");
                            let ref_schema = arr.get(2).and_then(|v| v.as_str());
                            let ref_table = arr.get(3).and_then(|v| v.as_str()).unwrap_or("");
                            let ref_col = arr.get(4).and_then(|v| v.as_str()).unwrap_or("");
                            let delete_rule = arr.get(5).and_then(|v| v.as_str());

                            if !fk_name.is_empty()
                                && !col_name.is_empty()
                                && !ref_table.is_empty()
                                && !ref_col.is_empty()
                            {
                                foreign_keys.push(ForeignKeyInfo {
                                    name: fk_name.to_string(),
                                    column: col_name.to_string(),
                                    referenced_schema: ref_schema.map(|s| s.to_string()),
                                    referenced_table: ref_table.to_string(),
                                    referenced_column: ref_col.to_string(),
                                    on_update: None,
                                    on_delete: delete_rule.map(|s| s.to_string()),
                                });
                            }
                        }
                    }
                }
                Ok((indexes, foreign_keys))
            })
            .await?;

        Ok(TableMetadata {
            columns,
            indexes,
            foreign_keys,
            clickhouse_extra: None,
            cassandra_extra: None,
            special_type_summaries: vec![],
        })
    }

    // Db2 has no native GET_DDL; this generates minimal DDL (no indexes,
    // foreign keys, comments, or tablespaces).
    async fn get_table_ddl(&self, schema: String, table: String) -> Result<String, String> {
        let structure = self
            .get_table_structure(schema.clone(), table.clone())
            .await?;
        let mut ddl = format!("CREATE TABLE {} (\n", quote_ident(&table));
        for (i, col) in structure.columns.iter().enumerate() {
            if i > 0 {
                ddl.push_str(",\n");
            }
            ddl.push_str(&format!("  {} {}", quote_ident(&col.name), col.r#type));
            if !col.nullable {
                ddl.push_str(" NOT NULL");
            }
            if let Some(ref default) = col.default_value {
                ddl.push_str(&format!(" DEFAULT {}", default));
            }
        }
        let pk_cols: Vec<&str> = structure
            .columns
            .iter()
            .filter(|c| c.primary_key)
            .map(|c| c.name.as_str())
            .collect();
        if !pk_cols.is_empty() {
            ddl.push_str(",\n  PRIMARY KEY (");
            ddl.push_str(
                &pk_cols
                    .iter()
                    .map(|c| quote_ident(c))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            ddl.push(')');
        }
        ddl.push_str("\n);");
        Ok(ddl)
    }

    async fn get_table_data(
        &self,
        schema: String,
        table: String,
        page: i64,
        limit: i64,
        sort_column: Option<String>,
        sort_direction: Option<String>,
        filter: Option<String>,
        order_by: Option<String>,
    ) -> Result<TableDataResponse, String> {
        let start = std::time::Instant::now();
        let safe_page = if page < 1 { 1 } else { page };
        let safe_limit = if limit < 1 { 100 } else { limit };
        let offset = (safe_page - 1) * safe_limit;

        let filter = filter.map(|f| super::normalize_quotes(&f));
        let order_by = order_by.map(|f| super::normalize_quotes(&f));

        let table_ref = format!("{}.{}", quote_ident(&schema), quote_ident(&table));

        let where_clause = match &filter {
            Some(f) if !f.trim().is_empty() => format!(" WHERE {}", f.trim()),
            _ => String::new(),
        };

        let order_clause = if let Some(ref raw) = order_by {
            if raw.trim().is_empty() {
                String::new()
            } else {
                format!(" ORDER BY {}", raw.trim())
            }
        } else if let Some(ref col) = sort_column {
            let dir = if matches!(sort_direction.as_deref(), Some("desc")) {
                "DESC"
            } else {
                "ASC"
            };
            format!(" ORDER BY {} {}", quote_ident(col), dir)
        } else {
            String::new()
        };

        self.run_blocking(move |conn| {
            // Total count
            let count_sql = format!("SELECT COUNT(*) FROM {}{}", table_ref, where_clause);
            let count_cursor = conn
                .execute(&count_sql, ())
                .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            let mut total: i64 = 0;
            if let Some(c) = count_cursor {
                let (_, count_rows) = collect_cursor_data(c)?;
                if let Some(row) = count_rows.first() {
                    total = row
                        .as_str()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                }
            }

            // Paginated data
            let data_sql = format!(
                "SELECT * FROM {}{}{} OFFSET {} ROWS FETCH NEXT {} ROWS ONLY",
                table_ref, where_clause, order_clause, offset, safe_limit
            );
            let data_cursor = conn
                .execute(&data_sql, ())
                .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            let mut data = Vec::new();
            if let Some(c) = data_cursor {
                let (_, rows) = collect_cursor_data(c)?;
                data = rows;
            }

            Ok(TableDataResponse {
                data,
                total,
                page: safe_page,
                limit: safe_limit,
                execution_time_ms: start.elapsed().as_millis() as i64,
            })
        })
        .await
    }

    async fn get_table_data_chunk(
        &self,
        schema: String,
        table: String,
        page: i64,
        limit: i64,
        sort_column: Option<String>,
        sort_direction: Option<String>,
        filter: Option<String>,
        order_by: Option<String>,
    ) -> Result<TableDataResponse, String> {
        self.get_table_data(
            schema,
            table,
            page,
            limit,
            sort_column,
            sort_direction,
            filter,
            order_by,
        )
        .await
    }

    async fn execute_query(&self, sql: String) -> Result<QueryResult, String> {
        let start = std::time::Instant::now();
        let statements = super::split_sql_statements(&sql);
        if statements.is_empty() {
            return Err("[QUERY_ERROR] Empty SQL statement".to_string());
        }

        if statements.len() == 1 {
            let last_sql = statements.last().unwrap().clone();
            return self
                .run_blocking(move |conn| {
                    let sql_clean = super::strip_trailing_statement_terminator(&last_sql);
                    let first_kw = super::first_sql_keyword(sql_clean);
                    let is_read = matches!(
                        first_kw.as_deref(),
                        Some("SELECT") | Some("WITH") | Some("SHOW")
                    );

                    if is_read {
                        let cursor = conn
                            .execute(sql_clean, ())
                            .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
                        match cursor {
                            Some(mut c) => {
                                let num_cols = c
                                    .num_result_cols()
                                    .map_err(|e| format!("[QUERY_ERROR] {e}"))?
                                    as u16;
                                let mut col_names = Vec::with_capacity(num_cols as usize);
                                let mut col_types = Vec::with_capacity(num_cols as usize);
                                for i in 1..=num_cols {
                                    let name = c
                                        .col_name(i)
                                        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
                                    let data_type = c
                                        .col_data_type(i)
                                        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
                                    col_names.push(name.clone());
                                    col_types.push(format!("{:?}", data_type));
                                }
                                let columns: Vec<QueryColumn> = col_names
                                    .iter()
                                    .zip(col_types.iter())
                                    .map(|(name, ty)| QueryColumn {
                                        name: name.clone(),
                                        r#type: ty.clone(),
                                    })
                                    .collect();

                                let mut data = Vec::new();
                                while let Some(mut row) = c
                                    .next_row()
                                    .map_err(|e| format!("[QUERY_ERROR] {e}"))?
                                {
                                    let mut map = serde_json::Map::new();
                                    for (i, name) in col_names.iter().enumerate() {
                                        map.insert(
                                            name.clone(),
                                            odbc_value_to_json(&mut row, (i + 1) as u16),
                                        );
                                    }
                                    data.push(serde_json::Value::Object(map));
                                }

                                Ok(QueryResult {
                                    row_count: data.len() as i64,
                                    data,
                                    columns,
                                    time_taken_ms: start.elapsed().as_millis() as i64,
                                    success: true,
                                    error: None,
                                    result_sets: None,
                                })
                            }
                            None => Ok(QueryResult {
                                row_count: 0,
                                data: vec![],
                                columns: vec![],
                                time_taken_ms: start.elapsed().as_millis() as i64,
                                success: true,
                                error: None,
                                result_sets: None,
                            }),
                        }
                    } else {
                        let mut prepared = conn
                            .prepare(sql_clean)
                            .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
                        prepared
                            .execute(())
                            .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
                        conn.commit().map_err(|e| format!("[QUERY_ERROR] commit failed: {e}"))?;
                        let row_count = prepared.row_count().map_err(|e| format!("[QUERY_ERROR] {e}")).ok().flatten().unwrap_or(0) as i64;
                        Ok(QueryResult {
                            row_count,
                            data: vec![],
                            columns: vec![],
                            time_taken_ms: start.elapsed().as_millis() as i64,
                            success: true,
                            error: None,
                            result_sets: None,
                        })
                    }
                })
                .await;
        }

        // Multiple statements
        self.run_blocking(move |conn| {
            let mut result_sets = Vec::new();
            let mut last_error: Option<String> = None;

            for (idx, statement) in statements.iter().enumerate() {
                let sql_clean = super::strip_trailing_statement_terminator(statement);
                let first_kw = super::first_sql_keyword(sql_clean);
                let is_read = matches!(
                    first_kw.as_deref(),
                    Some("SELECT") | Some("WITH") | Some("SHOW")
                );

                let result = if is_read {
                    let cursor = conn
                        .execute(sql_clean, ())
                        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
                    match cursor {
                        Some(mut c) => {
                            let num_cols = c
                                .num_result_cols()
                                .map_err(|e| format!("[QUERY_ERROR] {e}"))?
                                as u16;
                            let mut col_names = Vec::with_capacity(num_cols as usize);
                            let mut col_types = Vec::with_capacity(num_cols as usize);
                            for i in 1..=num_cols {
                                let name = c
                                    .col_name(i)
                                    .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
                                let data_type = c
                                    .col_data_type(i)
                                    .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
                                col_names.push(name.clone());
                                col_types.push(format!("{:?}", data_type));
                            }
                            let columns: Vec<QueryColumn> = col_names
                                .iter()
                                .zip(col_types.iter())
                                .map(|(name, ty)| QueryColumn {
                                    name: name.clone(),
                                    r#type: ty.clone(),
                                })
                                .collect();

                            let mut data = Vec::new();
                            while let Some(mut row) = c
                                .next_row()
                                .map_err(|e| format!("[QUERY_ERROR] {e}"))?
                            {
                                let mut map = serde_json::Map::new();
                                for (i, name) in col_names.iter().enumerate() {
                                    map.insert(
                                        name.clone(),
                                        odbc_value_to_json(&mut row, (i + 1) as u16),
                                    );
                                }
                                data.push(serde_json::Value::Object(map));
                            }
                            let row_count = data.len() as i64;
                            Ok((columns, data, row_count))
                        }
                        None => Ok((Vec::new(), Vec::new(), 0i64)),
                    }
                } else {
                    let mut prepared = conn
                        .prepare(sql_clean)
                        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
                    prepared
                        .execute(())
                        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
                    conn.commit().map_err(|e| format!("[QUERY_ERROR] commit failed: {e}"))?;
                    let row_count = prepared.row_count().map_err(|e| format!("[QUERY_ERROR] {e}")).ok().flatten().unwrap_or(0) as i64;
                    Ok((Vec::new(), Vec::new(), row_count))
                };

                match result {
                    Ok((columns, data, row_count)) => {
                        result_sets.push(SingleResultSet {
                            data,
                            row_count,
                            columns,
                            index: idx as u32,
                            statement: statement.clone(),
                        });
                    }
                    Err(e) => {
                        last_error = Some(e);
                        break;
                    }
                }
            }

            if let Some(err) = last_error {
                return Ok(QueryResult {
                    data: vec![],
                    row_count: 0,
                    columns: vec![],
                    time_taken_ms: start.elapsed().as_millis() as i64,
                    success: false,
                    error: Some(err),
                    result_sets: Some(result_sets),
                });
            }

            Ok(QueryResult {
                data: vec![],
                row_count: 0,
                columns: vec![],
                time_taken_ms: start.elapsed().as_millis() as i64,
                success: true,
                error: None,
                result_sets: Some(result_sets),
            })
        })
        .await
    }

    async fn get_schema_overview(&self, schema: Option<String>) -> Result<SchemaOverview, String> {
        let schema_upper = schema
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty());
        self.run_blocking(move |conn| {
            let sql = if let Some(ref s) = schema_upper {
                format!(
                    "SELECT TABSCHEMA, TABNAME, COLNAME, TYPENAME \
                     FROM SYSCAT.COLUMNS \
                     WHERE TABSCHEMA = '{}' \
                     ORDER BY TABSCHEMA, TABNAME, COLNO",
                    escape_literal(s)
                )
            } else {
                "SELECT TABSCHEMA, TABNAME, COLNAME, TYPENAME \
                 FROM SYSCAT.COLUMNS \
                 WHERE TABSCHEMA NOT LIKE 'SYS%' \
                 ORDER BY TABSCHEMA, TABNAME, COLNO"
                    .to_string()
            };
            let cursor = conn
                .execute(&sql, ())
                .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            let mut table_map: HashMap<(String, String), Vec<ColumnSchema>> = HashMap::new();
            if let Some(c) = cursor {
                let (_, rows) = collect_cursor_data(c)?;
                for row in &rows {
                    if let Some(arr) = row.as_array() {
                        let schema_name = arr.first().and_then(|v| v.as_str()).unwrap_or("");
                        let table_name = arr.get(1).and_then(|v| v.as_str()).unwrap_or("");
                        let col_name = arr.get(2).and_then(|v| v.as_str()).unwrap_or("");
                        let col_type = arr.get(3).and_then(|v| v.as_str()).unwrap_or("");
                        if !schema_name.is_empty()
                            && !table_name.is_empty()
                            && !col_name.is_empty()
                        {
                            table_map
                                .entry((schema_name.to_string(), table_name.to_string()))
                                .or_default()
                                .push(ColumnSchema {
                                    name: col_name.to_string(),
                                    r#type: col_type.to_string(),
                                });
                        }
                    }
                }
            }
            let mut tables: Vec<TableSchema> = table_map
                .into_iter()
                .map(|((s, n), cols)| TableSchema {
                    schema: s,
                    name: n,
                    columns: cols,
                })
                .collect();
            tables.sort_by(|a, b| a.schema.cmp(&b.schema).then(a.name.cmp(&b.name)));
            Ok(SchemaOverview { tables })
        })
        .await
    }

    async fn get_schema_foreign_keys(
        &self,
        _database: Option<&str>,
    ) -> Result<Vec<SchemaForeignKey>, String> {
        self.run_blocking(move |conn| {
            let sql = "SELECT fk.CONSTNAME, \
                     fk.TABSCHEMA, fk.TABNAME, fk.COLNAME, \
                     fk.REFTABSCHEMA, fk.REFTABNAME, refcol.COLNAME, fk.DELETERULE \
                     FROM SYSCAT.REFERENCES fk \
                     JOIN SYSCAT.KEYCOLUSE refcol \
                       ON refcol.CONSTNAME = fk.REFKEYNAME \
                       AND refcol.TABSCHEMA = fk.REFTABSCHEMA \
                     ORDER BY fk.CONSTNAME, refcol.COLSEQ";
            let cursor = conn
                .execute(sql, ())
                .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            let mut foreign_keys = Vec::new();
            if let Some(c) = cursor {
                let (_, rows) = collect_cursor_data(c)?;
                for row in &rows {
                    if let Some(arr) = row.as_array() {
                        let fk_name = arr.first().and_then(|v| v.as_str()).unwrap_or("");
                        let src_schema = arr.get(1).and_then(|v| v.as_str());
                        let src_table = arr.get(2).and_then(|v| v.as_str()).unwrap_or("");
                        let src_col = arr.get(3).and_then(|v| v.as_str()).unwrap_or("");
                        let tgt_schema = arr.get(4).and_then(|v| v.as_str());
                        let tgt_table = arr.get(5).and_then(|v| v.as_str()).unwrap_or("");
                        let tgt_col = arr.get(6).and_then(|v| v.as_str()).unwrap_or("");
                        let delete_rule = arr.get(7).and_then(|v| v.as_str());

                        if !fk_name.is_empty()
                            && !src_table.is_empty()
                            && !src_col.is_empty()
                            && !tgt_table.is_empty()
                            && !tgt_col.is_empty()
                        {
                            foreign_keys.push(SchemaForeignKey {
                                name: fk_name.to_string(),
                                source_schema: src_schema.map(|s| s.to_string()),
                                source_table: src_table.to_string(),
                                source_column: src_col.to_string(),
                                target_schema: tgt_schema.map(|s| s.to_string()),
                                target_table: tgt_table.to_string(),
                                target_column: tgt_col.to_string(),
                                on_update: None,
                                on_delete: delete_rule.map(|s| s.to_string()),
                            });
                        }
                    }
                }
            }
            Ok(foreign_keys)
        })
        .await
    }

    async fn close(&self) {}
}

fn format_db2_type(type_name: &str, length: i64, scale: i64) -> String {
    match type_name {
        "VARCHAR" | "NVARCHAR" | "CHAR" | "NCHAR" | "VARGRAPHIC" | "DBCLOB" => {
            if length > 0 {
                format!("{}({})", type_name, length)
            } else {
                type_name.to_string()
            }
        }
        "DECIMAL" | "NUMERIC" => {
            if length > 0 && scale > 0 {
                format!("{}({},{})", type_name, length, scale)
            } else if length > 0 {
                format!("{}({})", type_name, length)
            } else {
                type_name.to_string()
            }
        }
        _ => type_name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{escape_literal, format_db2_type, odbc_escape_value, quote_ident};

    #[test]
    fn quote_ident_wraps_in_double_quotes() {
        assert_eq!(quote_ident("MY_TABLE"), "\"MY_TABLE\"");
    }

    #[test]
    fn quote_ident_escapes_embedded_double_quote() {
        assert_eq!(quote_ident("a\"b"), "\"a\"\"b\"");
    }

    #[test]
    fn escape_literal_escapes_single_quote() {
        assert_eq!(escape_literal("O'Brien"), "O''Brien");
    }

    #[test]
    fn format_db2_type_varchar_with_length() {
        assert_eq!(format_db2_type("VARCHAR", 255, 0), "VARCHAR(255)");
    }

    #[test]
    fn format_db2_type_decimal_with_scale() {
        assert_eq!(format_db2_type("DECIMAL", 10, 2), "DECIMAL(10,2)");
    }

    #[test]
    fn format_db2_type_integer() {
        assert_eq!(format_db2_type("INTEGER", 0, 0), "INTEGER");
    }

    #[test]
    fn format_db2_type_timestamp() {
        assert_eq!(format_db2_type("TIMESTAMP", 0, 0), "TIMESTAMP");
    }

    #[test]
    fn odbc_escape_value_plain() {
        assert_eq!(odbc_escape_value("myuser"), "myuser");
    }

    #[test]
    fn odbc_escape_value_with_semicolon() {
        assert_eq!(odbc_escape_value("p@ss;word"), "{p@ss;word}");
    }

    #[test]
    fn odbc_escape_value_with_braces() {
        assert_eq!(odbc_escape_value("a{b}c"), "{a{b}}c}");
    }

    #[test]
    fn first_sql_keyword_extracts_select() {
        assert_eq!(
            crate::db::drivers::first_sql_keyword("  SELECT id FROM t"),
            Some("SELECT".to_string())
        );
    }

    #[test]
    fn first_sql_keyword_skips_comments() {
        assert_eq!(
            crate::db::drivers::first_sql_keyword("-- comment\nINSERT INTO t VALUES(1)"),
            Some("INSERT".to_string())
        );
    }

    #[test]
    fn first_sql_keyword_identifies_with() {
        assert_eq!(
            crate::db::drivers::first_sql_keyword("WITH cte AS (SELECT 1) SELECT * FROM cte"),
            Some("WITH".to_string())
        );
    }
}
