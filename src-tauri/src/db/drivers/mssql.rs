use super::DatabaseDriver;
use crate::models::{
    ColumnInfo, ColumnSchema, ConnectionForm, ForeignKeyInfo, IndexInfo, QueryColumn, QueryResult,
    RoutineInfo, SchemaOverview, TableDataResponse, TableInfo, TableMetadata, TableSchema,
    TableStructure,
};
use async_trait::async_trait;
use bb8::{Pool, RunError};
use futures_util::TryStreamExt;
use std::collections::{HashMap, HashSet};
use tiberius::{AuthMethod, Client, Config, EncryptionLevel, QueryItem, Row};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use crate::ssh::SshTunnel;

pub struct MssqlDriver {
    pub pool: Pool<MssqlConnectionManager>,
    pub ssh_tunnel: Option<SshTunnel>,
}

pub struct MssqlConnectionManager {
    config: MssqlConfig,
}

#[derive(Clone)]
struct MssqlConfig {
    host: String,
    port: u16,
    database: String,
    username: String,
    password: String,
    ssl: bool,
}

fn build_config(form: &ConnectionForm) -> Result<MssqlConfig, String> {
    let host = form
        .host
        .clone()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.trim().is_empty())
        .ok_or("[VALIDATION_ERROR] host cannot be empty")?;
    let port = form.port.unwrap_or(1433);
    if !(0..=65535).contains(&port) {
        return Err("[VALIDATION_ERROR] port out of range".to_string());
    }
    let database = form
        .database
        .clone()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "master".to_string());
    let username = form
        .username
        .clone()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.trim().is_empty())
        .ok_or("[VALIDATION_ERROR] username cannot be empty")?;
    let password = form.password.clone().unwrap_or_default();

    Ok(MssqlConfig {
        host,
        port: port as u16,
        database,
        username,
        password,
        ssl: form.ssl.unwrap_or(false),
    })
}

fn escape_literal(value: &str) -> String {
    value.replace('\'', "''")
}

fn routine_type_sql_filter(routine_type: &str) -> Result<&'static str, String> {
    if routine_type.eq_ignore_ascii_case("procedure") {
        Ok("('P')")
    } else if routine_type.eq_ignore_ascii_case("function") {
        Ok("('FN','IF','TF','FS','FT')")
    } else {
        Err(format!(
            "[VALIDATION_ERROR] Unsupported routine type '{}'",
            routine_type
        ))
    }
}

fn map_pool_error(err: RunError<String>) -> String {
    match err {
        RunError::User(inner) => inner,
        RunError::TimedOut => "[CONN_FAILED] Timed out acquiring MSSQL connection".to_string(),
    }
}

fn quote_ident(ident: &str) -> Result<String, String> {
    let trimmed = ident.trim();
    if trimmed.is_empty() {
        return Err("[VALIDATION_ERROR] identifier cannot be empty".to_string());
    }
    if trimmed.chars().any(|c| c == '\0') {
        return Err("[VALIDATION_ERROR] identifier contains null byte".to_string());
    }
    Ok(format!("[{}]", trimmed.replace(']', "]]")))
}

fn table_ref(schema: &str, table: &str) -> Result<String, String> {
    Ok(format!("{}.{}", quote_ident(schema)?, quote_ident(table)?))
}

fn first_sql_keyword(sql: &str) -> Option<String> {
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    loop {
        while i < len && (bytes[i].is_ascii_whitespace() || bytes[i] == b';') {
            i += 1;
        }

        if i + 1 < len && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            i += 2;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 >= len {
                return None;
            }
            i += 2;
            continue;
        }

        break;
    }

    if i >= len {
        return None;
    }

    let start = i;
    while i < len && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    if start == i {
        return None;
    }

    Some(sql[start..i].to_ascii_lowercase())
}

impl MssqlConnectionManager {
    fn new(config: MssqlConfig) -> Self {
        Self { config }
    }

    fn build_tiberius_config(&self, encryption: EncryptionLevel, trust_cert: bool) -> Config {
        let mut config = Config::new();
        config.host(&self.config.host);
        config.port(self.config.port);
        config.database(&self.config.database);
        config.authentication(AuthMethod::sql_server(
            self.config.username.clone(),
            self.config.password.clone(),
        ));
        config.encryption(encryption);
        if trust_cert
            && !matches!(
                encryption,
                EncryptionLevel::Off | EncryptionLevel::NotSupported
            )
        {
            config.trust_cert();
        }
        config
    }

    async fn connect_single(&self) -> Result<Client<Compat<TcpStream>>, String> {
        let attempts = if self.config.ssl {
            vec![
                (
                    EncryptionLevel::Required,
                    false,
                    "encrypt=required,trust_cert=false",
                ),
                (EncryptionLevel::On, false, "encrypt=on,trust_cert=false"),
            ]
        } else {
            vec![
                (EncryptionLevel::Off, false, "encrypt=off"),
                (
                    EncryptionLevel::NotSupported,
                    false,
                    "encrypt=not_supported",
                ),
                (EncryptionLevel::On, true, "encrypt=on,trust_cert=true"),
                (
                    EncryptionLevel::Required,
                    true,
                    "encrypt=required,trust_cert=true",
                ),
            ]
        };

        let mut errors = Vec::new();
        for (encryption, trust_cert, label) in attempts {
            let config = self.build_tiberius_config(encryption, trust_cert);
            match Self::connect_with_config(config).await {
                Ok(client) => return Ok(client),
                Err(err) => errors.push(format!("{label}: {err}")),
            }
        }

        Err(format!(
            "[CONN_FAILED] SQL Server handshake failed after retries: {}",
            errors.join(" | ")
        ))
    }

    async fn connect_with_config(config: Config) -> Result<Client<Compat<TcpStream>>, String> {
        let connect_future = async {
            let tcp = TcpStream::connect(config.get_addr())
                .await
                .map_err(|e| format!("{}", e))?;
            tcp.set_nodelay(true).map_err(|e| format!("{}", e))?;
            Ok::<TcpStream, String>(tcp)
        };

        let tcp = tokio::time::timeout(std::time::Duration::from_secs(10), connect_future)
            .await
            .map_err(|_| "Connection timed out".to_string())?
            .map_err(|e| format!("{}", e))?;

        Client::connect(config, tcp.compat_write())
            .await
            .map_err(|e| format!("{}", e))
    }
}

#[async_trait]
impl bb8::ManageConnection for MssqlConnectionManager {
    type Connection = Client<Compat<TcpStream>>;
    type Error = String;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        self.connect_single().await
    }

    async fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        conn.simple_query("SELECT 1")
            .await
            .map_err(|e| format!("{}", e))?;
        Ok(())
    }

    fn has_broken(&self, _conn: &mut Self::Connection) -> bool {
        false
    }
}

#[allow(dead_code)]
struct MssqlColumnInfo {
    name: String,
    data_type: String,
    full_type: String,
    is_nullable: bool,
    is_identity: bool,
    is_computed: bool,
    computed_definition: Option<String>,
    default_definition: Option<String>,
    default_constraint_name: Option<String>,
}

struct MssqlKeyConstraint {
    name: String,
    constraint_type: String,
    columns: Vec<String>,
}

fn mssql_full_type_string(data_type: &str, max_length: i64, precision: i64, scale: i64) -> String {
    let dt = data_type.to_ascii_lowercase();
    match dt.as_str() {
        "varchar" | "char" | "varbinary" | "binary" => {
            let len = if max_length == -1 {
                "MAX".to_string()
            } else {
                max_length.to_string()
            };
            format!("{}({})", data_type, len)
        }
        "nvarchar" | "nchar" => {
            let len = if max_length == -1 {
                "MAX".to_string()
            } else {
                // nvarchar stores 2 bytes per char
                (max_length / 2).to_string()
            };
            format!("{}({})", data_type, len)
        }
        "decimal" | "numeric" => format!("{}({},{})", data_type, precision, scale),
        "datetime2" | "datetimeoffset" | "time" => {
            if scale > 0 {
                format!("{}({})", data_type, scale)
            } else {
                data_type.to_string()
            }
        }
        _ => data_type.to_string(),
    }
}

impl MssqlDriver {
    pub async fn connect(form: &ConnectionForm) -> Result<Self, String> {
        let mut cfg_form = form.clone();
        let mut ssh_tunnel = None;

        if let Some(true) = form.ssh_enabled {
            let tunnel = crate::ssh::start_ssh_tunnel(form)?;
            cfg_form.host = Some("127.0.0.1".to_string());
            cfg_form.port = Some(tunnel.local_port as i64);
            ssh_tunnel = Some(tunnel);
        }

        let config = build_config(&cfg_form)?;
        let manager = MssqlConnectionManager::new(config);
        let pool = Pool::builder()
            .max_size(10)
            .build(manager)
            .await
            .map_err(|e| format!("[CONN_FAILED] Failed to create connection pool: {}", e))?;

        let driver = Self { pool, ssh_tunnel };
        driver.test_connection().await?;
        Ok(driver)
    }

    async fn fetch_rows(&self, sql: &str) -> Result<Vec<Row>, String> {
        Ok(self.fetch_rows_with_columns(sql).await?.0)
    }

    async fn fetch_rows_with_columns(
        &self,
        sql: &str,
    ) -> Result<(Vec<Row>, Vec<QueryColumn>), String> {
        let mut client = self.pool.get().await.map_err(map_pool_error)?;
        let mut stream = client
            .simple_query(sql)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {}", e))?;
        let mut rows = Vec::new();
        let mut columns = Vec::new();

        while let Some(item) = stream
            .try_next()
            .await
            .map_err(|e| format!("[QUERY_ERROR] {}", e))?
        {
            match item {
                QueryItem::Metadata(meta) if columns.is_empty() => {
                    columns = meta
                        .columns()
                        .iter()
                        .map(|col| QueryColumn {
                            name: col.name().to_string(),
                            r#type: format!("{:?}", col.column_type()),
                        })
                        .collect();
                }
                QueryItem::Row(row) => rows.push(row),
                _ => {}
            }
        }

        Ok((rows, columns))
    }

    /// Execute a single query wrapped with FOR JSON, collecting both column
    /// metadata and JSON row data from the same stream to avoid dual execution.
    async fn fetch_query_result_json(
        &self,
        sql: &str,
    ) -> Result<(Vec<serde_json::Value>, Vec<QueryColumn>), String> {
        let mut client = self.pool.get().await.map_err(map_pool_error)?;
        let json_sql = Self::build_for_json_query(sql);
        let mut stream = client
            .simple_query(&json_sql)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {}", e))?;

        let mut columns: Vec<QueryColumn> = Vec::new();
        let mut high_precision_cols = HashSet::new();
        let mut json_text = String::new();

        while let Some(item) = stream
            .try_next()
            .await
            .map_err(|e| format!("[QUERY_ERROR] {}", e))?
        {
            match item {
                QueryItem::Metadata(meta) if columns.is_empty() => {
                    for col in meta.columns() {
                        let type_str = format!("{:?}", col.column_type());
                        if is_high_precision_mssql_query_type(&type_str) {
                            high_precision_cols.insert(col.name().to_string());
                        }
                        columns.push(QueryColumn {
                            name: col.name().to_string(),
                            r#type: type_str,
                        });
                    }
                }
                QueryItem::Row(row) => {
                    json_text.push_str(&Self::parse_string(&row, 0));
                }
                _ => {}
            }
        }

        if json_text.trim().is_empty() {
            return Ok((Vec::new(), columns));
        }

        let parsed: serde_json::Value = serde_json::from_str(&json_text)
            .map_err(|e| format!("[QUERY_ERROR] Failed to parse MSSQL JSON result: {e}"))?;
        let mut data = match parsed {
            serde_json::Value::Array(arr) => arr,
            serde_json::Value::Object(obj) => vec![serde_json::Value::Object(obj)],
            _ => {
                return Err("[QUERY_ERROR] MSSQL FOR JSON result is not array/object".to_string());
            }
        };
        for row in &mut data {
            normalize_mssql_row_json(row, &high_precision_cols)?;
        }

        Ok((data, columns))
    }

    fn build_for_json_query(sql: &str) -> String {
        let trimmed = sql.trim_end().trim_end_matches(';').trim_end();
        // Avoid double-wrapping when the query already ends with FOR JSON / FOR XML
        let upper = trimmed.to_uppercase();
        if upper.ends_with("FOR JSON PATH")
            || upper.ends_with("FOR JSON AUTO")
            || upper.ends_with("FOR JSON EXPLICIT")
            || upper.ends_with("FOR JSON PATH, WITHOUT_ARRAY_WRAPPER")
        {
            return trimmed.to_string();
        }
        format!("{trimmed} FOR JSON PATH, INCLUDE_NULL_VALUES")
    }

    fn parse_i64(row: &Row, idx: usize) -> i64 {
        if let Ok(Some(v)) = row.try_get::<i64, _>(idx) {
            return v;
        }
        if let Ok(Some(v)) = row.try_get::<i32, _>(idx) {
            return v as i64;
        }
        if let Ok(Some(v)) = row.try_get::<&str, _>(idx) {
            return v.parse::<i64>().unwrap_or(0);
        }
        0
    }

    fn parse_string(row: &Row, idx: usize) -> String {
        if let Ok(Some(v)) = row.try_get::<&str, _>(idx) {
            return v.to_string();
        }
        if let Ok(Some(v)) = row.try_get::<&[u8], _>(idx) {
            return String::from_utf8_lossy(v).to_string();
        }
        String::new()
    }

    /// Load columns with identity, computed, and precision/scale/length info.
    async fn load_mssql_columns(
        &self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<MssqlColumnInfo>, String> {
        let sql = format!(
            "SELECT c.name, t.name AS data_type, c.is_nullable, \
                    c.max_length, c.precision, c.scale, \
                    c.is_identity, c.is_computed, \
                    cc.definition AS computed_definition, \
                    dc.definition AS default_definition, \
                    dc.name AS default_constraint_name \
             FROM sys.columns c \
             JOIN sys.types t ON c.user_type_id = t.user_type_id \
             JOIN sys.tables tbl ON tbl.object_id = c.object_id \
             JOIN sys.schemas s ON s.schema_id = tbl.schema_id \
             LEFT JOIN sys.computed_columns cc \
               ON cc.object_id = c.object_id AND cc.column_id = c.column_id \
             LEFT JOIN sys.default_constraints dc \
               ON dc.parent_object_id = c.object_id AND dc.parent_column_id = c.column_id \
             WHERE s.name = '{}' AND tbl.name = '{}' \
             ORDER BY c.column_id",
            escape_literal(schema),
            escape_literal(table)
        );
        let rows = self.fetch_rows(&sql).await?;
        let mut cols = Vec::new();
        for row in rows {
            let data_type = Self::parse_string(&row, 1);
            let max_length = Self::parse_i64(&row, 3);
            let precision = Self::parse_i64(&row, 4);
            let scale = Self::parse_i64(&row, 5);
            let is_identity = Self::parse_i64(&row, 6) == 1;
            let is_computed = Self::parse_i64(&row, 7) == 1;
            let computed_def = Self::parse_string(&row, 8);
            let default_def = Self::parse_string(&row, 9);
            let default_cn = Self::parse_string(&row, 10);

            let full_type = mssql_full_type_string(&data_type, max_length, precision, scale);

            cols.push(MssqlColumnInfo {
                name: Self::parse_string(&row, 0),
                data_type,
                full_type,
                is_nullable: Self::parse_string(&row, 2).eq_ignore_ascii_case("1"),
                is_identity,
                is_computed,
                computed_definition: if computed_def.is_empty() {
                    None
                } else {
                    Some(computed_def)
                },
                default_definition: if default_def.is_empty() {
                    None
                } else {
                    Some(default_def)
                },
                default_constraint_name: if default_cn.is_empty() {
                    None
                } else {
                    Some(default_cn)
                },
            });
        }
        Ok(cols)
    }

    /// Load key constraints (PRIMARY KEY and UNIQUE).
    async fn load_mssql_key_constraints(
        &self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<MssqlKeyConstraint>, String> {
        let sql = format!(
            "SELECT kc.name, kc.type_desc, c.name AS col_name, ic.key_ordinal \
             FROM sys.key_constraints kc \
             JOIN sys.index_columns ic \
               ON ic.object_id = kc.parent_object_id AND ic.index_id = kc.unique_index_id \
             JOIN sys.columns c \
               ON c.object_id = ic.object_id AND c.column_id = ic.column_id \
             JOIN sys.tables t ON t.object_id = kc.parent_object_id \
             JOIN sys.schemas s ON s.schema_id = t.schema_id \
             WHERE s.name = '{}' AND t.name = '{}' \
             ORDER BY kc.name, ic.key_ordinal",
            escape_literal(schema),
            escape_literal(table)
        );
        let rows = self.fetch_rows(&sql).await?;
        let mut map: HashMap<String, (String, Vec<(i64, String)>)> = HashMap::new();
        for row in rows {
            let name = Self::parse_string(&row, 0);
            let type_desc = Self::parse_string(&row, 1);
            let col = Self::parse_string(&row, 2);
            let ord = Self::parse_i64(&row, 3);
            map.entry(name)
                .or_insert((type_desc, Vec::new()))
                .1
                .push((ord, col));
        }
        Ok(map
            .into_iter()
            .map(|(name, (type_desc, mut cols))| {
                cols.sort_by_key(|(ord, _)| *ord);
                MssqlKeyConstraint {
                    name,
                    constraint_type: if type_desc.contains("PRIMARY") {
                        "PRIMARY KEY".to_string()
                    } else {
                        "UNIQUE".to_string()
                    },
                    columns: cols.into_iter().map(|(_, c)| c).collect(),
                }
            })
            .collect())
    }

    /// Load check constraints.
    async fn load_mssql_check_constraints(
        &self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<(String, String)>, String> {
        let sql = format!(
            "SELECT cc.name, cc.definition \
             FROM sys.check_constraints cc \
             JOIN sys.tables t ON t.object_id = cc.parent_object_id \
             JOIN sys.schemas s ON s.schema_id = t.schema_id \
             WHERE s.name = '{}' AND t.name = '{}' AND cc.is_ms_shipped = 0 \
             ORDER BY cc.name",
            escape_literal(schema),
            escape_literal(table)
        );
        let rows = self.fetch_rows(&sql).await?;
        Ok(rows
            .into_iter()
            .map(|row| (Self::parse_string(&row, 0), Self::parse_string(&row, 1)))
            .collect())
    }

    /// Load foreign key constraints.
    async fn load_mssql_foreign_keys(
        &self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ForeignKeyInfo>, String> {
        let sql = format!(
            "SELECT fk.name, pc.name, rs.name, rt.name, rc.name, \
                    fk.update_referential_action_desc, fk.delete_referential_action_desc \
             FROM sys.foreign_keys fk \
             JOIN sys.foreign_key_columns fkc ON fk.object_id = fkc.constraint_object_id \
             JOIN sys.tables pt ON pt.object_id = fk.parent_object_id \
             JOIN sys.schemas ps ON ps.schema_id = pt.schema_id \
             JOIN sys.columns pc ON pc.object_id = pt.object_id AND pc.column_id = fkc.parent_column_id \
             JOIN sys.tables rt ON rt.object_id = fk.referenced_object_id \
             JOIN sys.schemas rs ON rs.schema_id = rt.schema_id \
             JOIN sys.columns rc ON rc.object_id = rt.object_id AND rc.column_id = fkc.referenced_column_id \
             WHERE ps.name = '{}' AND pt.name = '{}' \
             ORDER BY fk.name, fkc.constraint_column_id",
            escape_literal(schema),
            escape_literal(table)
        );
        let rows = self.fetch_rows(&sql).await?;
        let mut fks: Vec<ForeignKeyInfo> = Vec::new();
        for row in rows {
            fks.push(ForeignKeyInfo {
                name: Self::parse_string(&row, 0),
                column: Self::parse_string(&row, 1),
                referenced_schema: Some(Self::parse_string(&row, 2)),
                referenced_table: Self::parse_string(&row, 3),
                referenced_column: Self::parse_string(&row, 4),
                on_update: Some(Self::parse_string(&row, 5)),
                on_delete: Some(Self::parse_string(&row, 6)),
            });
        }
        Ok(fks)
    }

    /// Load indexes for a table. When `include_constraints` is false, PK and
    /// unique-constraint indexes are excluded (used for DDL generation).
    async fn load_mssql_indexes(
        &self,
        schema: &str,
        table: &str,
        include_constraints: bool,
    ) -> Result<Vec<IndexInfo>, String> {
        let constraint_filter = if include_constraints {
            ""
        } else {
            " AND i.is_primary_key = 0 AND i.is_unique_constraint = 0"
        };
        let sql = format!(
            "SELECT i.name, i.is_unique, i.type_desc, c.name, ic.key_ordinal \
             FROM sys.indexes i \
             JOIN sys.index_columns ic ON i.object_id = ic.object_id AND i.index_id = ic.index_id \
             JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id \
             JOIN sys.tables t ON t.object_id = i.object_id \
             JOIN sys.schemas s ON s.schema_id = t.schema_id \
             WHERE s.name = '{}' AND t.name = '{}' \
               AND i.name IS NOT NULL{} \
             ORDER BY i.name, ic.key_ordinal",
            escape_literal(schema),
            escape_literal(table),
            constraint_filter
        );
        let rows = self.fetch_rows(&sql).await?;
        let mut map: HashMap<String, (bool, Option<String>, Vec<(i64, String)>)> = HashMap::new();
        for row in rows {
            let name = Self::parse_string(&row, 0);
            let unique = Self::parse_i64(&row, 1) == 1;
            let idx_type = Self::parse_string(&row, 2);
            let col = Self::parse_string(&row, 3);
            let ord = Self::parse_i64(&row, 4);
            let entry = map
                .entry(name)
                .or_insert((unique, Some(idx_type.clone()), Vec::new()));
            entry.0 = unique;
            if entry.1.is_none() && !idx_type.is_empty() {
                entry.1 = Some(idx_type);
            }
            entry.2.push((ord, col));
        }
        let mut indexes: Vec<IndexInfo> = map
            .into_iter()
            .map(|(name, (unique, index_type, mut cols))| {
                cols.sort_by_key(|(ord, _)| *ord);
                IndexInfo {
                    name,
                    unique,
                    index_type,
                    columns: cols.into_iter().map(|(_, c)| c).collect(),
                }
            })
            .collect();
        indexes.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(indexes)
    }
}

#[cfg(test)]
fn is_high_precision_mssql_data_type(data_type: &str) -> bool {
    matches!(
        data_type.trim().to_ascii_lowercase().as_str(),
        "bigint" | "decimal" | "numeric" | "money" | "smallmoney"
    )
}

fn is_high_precision_mssql_query_type(type_name: &str) -> bool {
    let t = type_name.trim().to_ascii_lowercase();
    t.contains("int8")
        || t.contains("bigint")
        || t.contains("numeric")
        || t.contains("decimal")
        || t.contains("money")
}

fn normalize_mssql_row_json(
    row_json: &mut serde_json::Value,
    high_precision_cols: &HashSet<String>,
) -> Result<(), String> {
    let obj = row_json
        .as_object_mut()
        .ok_or("[QUERY_ERROR] Expected JSON object row from MSSQL FOR JSON".to_string())?;

    let mut lookup: HashMap<String, String> = HashMap::new();
    for key in obj.keys() {
        lookup.insert(key.to_ascii_lowercase(), key.clone());
    }

    for col in high_precision_cols {
        let Some(actual_key) = lookup.get(&col.to_ascii_lowercase()) else {
            continue;
        };
        let Some(v) = obj.get_mut(actual_key) else {
            continue;
        };
        if v.is_number() {
            *v = serde_json::Value::String(v.to_string());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        is_high_precision_mssql_data_type, is_high_precision_mssql_query_type, quote_ident,
        routine_type_sql_filter, MssqlDriver,
    };
    use std::collections::HashSet;

    #[test]
    fn quote_ident_allows_common_mssql_names() {
        assert_eq!(
            quote_ident("order-detail 2026").unwrap(),
            "[order-detail 2026]"
        );
        assert_eq!(quote_ident("用户表").unwrap(), "[用户表]");
    }

    #[test]
    fn quote_ident_escapes_bracket_and_trims() {
        assert_eq!(quote_ident("  a]b ").unwrap(), "[a]]b]");
    }

    #[test]
    fn quote_ident_rejects_empty_and_null_byte() {
        assert!(quote_ident("   ").is_err());
        assert!(quote_ident("abc\0def").is_err());
    }

    #[test]
    fn test_is_high_precision_mssql_data_type() {
        assert!(is_high_precision_mssql_data_type("bigint"));
        assert!(is_high_precision_mssql_data_type("DECIMAL"));
        assert!(is_high_precision_mssql_data_type("money"));
        assert!(!is_high_precision_mssql_data_type("int"));
    }

    #[test]
    fn test_is_high_precision_mssql_query_type() {
        assert!(is_high_precision_mssql_query_type("Int8"));
        assert!(is_high_precision_mssql_query_type("Numericn"));
        assert!(is_high_precision_mssql_query_type("Money"));
        assert!(!is_high_precision_mssql_query_type("Int4"));
    }

    #[test]
    fn test_routine_type_sql_filter_maps_supported_types() {
        assert_eq!(routine_type_sql_filter("procedure").unwrap(), "('P')");
        assert_eq!(routine_type_sql_filter("PROCEDURE").unwrap(), "('P')");
        assert_eq!(
            routine_type_sql_filter("function").unwrap(),
            "('FN','IF','TF','FS','FT')"
        );
        assert!(routine_type_sql_filter("trigger").is_err());
    }

    #[test]
    fn test_normalize_mssql_row_json_stringify_high_precision() {
        let mut row = serde_json::json!({
            "id": 9223372036854775807_i64,
            "amount": 1234.56,
            "name": "x"
        });
        let hp = HashSet::from(["ID".to_string(), "amount".to_string()]);
        super::normalize_mssql_row_json(&mut row, &hp).unwrap();
        assert_eq!(
            row.get("id").and_then(|v| v.as_str()),
            Some("9223372036854775807")
        );
        assert_eq!(row.get("amount").and_then(|v| v.as_str()), Some("1234.56"));
        assert_eq!(row.get("name").and_then(|v| v.as_str()), Some("x"));
    }

    #[test]
    fn test_build_for_json_query_trims_trailing_semicolon() {
        let sql = "SELECT id, name FROM dbo.users;";
        assert_eq!(
            MssqlDriver::build_for_json_query(sql),
            "SELECT id, name FROM dbo.users FOR JSON PATH, INCLUDE_NULL_VALUES"
        );
    }
}

fn render_mssql_create_table_ddl(
    schema: &str,
    table: &str,
    columns: &[MssqlColumnInfo],
    key_constraints: &[MssqlKeyConstraint],
    check_constraints: &[(String, String)],
    foreign_keys: &[ForeignKeyInfo],
    indexes: &[IndexInfo],
) -> String {
    let mut lines: Vec<String> = Vec::new();

    // Column definitions
    for col in columns {
        if col.is_computed {
            let def = col.computed_definition.as_deref().unwrap_or("(NULL)");
            lines.push(format!("    {} AS {}", quote_ident_or(&col.name), def));
            continue;
        }

        let mut line = format!("    {} {}", quote_ident_or(&col.name), col.full_type);
        if col.is_identity {
            line.push_str(" IDENTITY(1,1)");
        }
        if !col.is_nullable {
            line.push_str(" NOT NULL");
        }
        // Inline default from column info (loaded via sys.default_constraints join)
        if let (Some(def), Some(cn)) = (&col.default_definition, &col.default_constraint_name) {
            line.push_str(&format!(
                " CONSTRAINT {} DEFAULT {}",
                quote_ident_or(cn),
                def
            ));
        }
        lines.push(line);
    }

    // Primary key constraints (inline)
    for kc in key_constraints {
        if kc.constraint_type == "PRIMARY KEY" {
            let cols: Vec<String> = kc.columns.iter().map(|c| quote_ident_or(c)).collect();
            lines.push(format!(
                "    CONSTRAINT {} PRIMARY KEY ({})",
                quote_ident_or(&kc.name),
                cols.join(", ")
            ));
        }
    }

    // Unique constraints (inline)
    for kc in key_constraints {
        if kc.constraint_type == "UNIQUE" {
            let cols: Vec<String> = kc.columns.iter().map(|c| quote_ident_or(c)).collect();
            lines.push(format!(
                "    CONSTRAINT {} UNIQUE ({})",
                quote_ident_or(&kc.name),
                cols.join(", ")
            ));
        }
    }

    // Check constraints (inline)
    for (name, definition) in check_constraints {
        lines.push(format!(
            "    CONSTRAINT {} CHECK {}",
            quote_ident_or(name),
            definition
        ));
    }

    // Foreign keys (inline)
    for fk in foreign_keys {
        let ref_schema = fk.referenced_schema.as_deref().unwrap_or("dbo");
        let mut fk_line = format!(
            "    CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}.{} ({})",
            quote_ident_or(&fk.name),
            quote_ident_or(&fk.column),
            quote_ident_or(ref_schema),
            quote_ident_or(&fk.referenced_table),
            quote_ident_or(&fk.referenced_column),
        );
        if let Some(ref action) = fk.on_update {
            if action != "NO_ACTION" {
                fk_line.push_str(&format!(" ON UPDATE {}", action));
            }
        }
        if let Some(ref action) = fk.on_delete {
            if action != "NO_ACTION" {
                fk_line.push_str(&format!(" ON DELETE {}", action));
            }
        }
        lines.push(fk_line);
    }

    let body = lines.join(",\n");
    let mut ddl = format!(
        "-- Note: This DDL is reconstructed from table metadata.\n\
         CREATE TABLE {}.{} (\n{}\n);",
        quote_ident_or(schema),
        quote_ident_or(table),
        body
    );

    // Non-constraint indexes as separate CREATE INDEX statements
    for idx in indexes {
        let unique_keyword = if idx.unique { "UNIQUE " } else { "" };
        let idx_type = idx.index_type.as_deref().unwrap_or("");
        let idx_hint = if idx_type.to_ascii_lowercase().contains("clustered") {
            "CLUSTERED "
        } else if idx_type.to_ascii_lowercase().contains("nonclustered") {
            "NONCLUSTERED "
        } else {
            ""
        };
        let cols: Vec<String> = idx.columns.iter().map(|c| quote_ident_or(c)).collect();
        ddl.push_str(&format!(
            "\nCREATE {unique_keyword}INDEX {idx_hint}{} ON {}.{} ({});",
            quote_ident_or(&idx.name),
            quote_ident_or(schema),
            quote_ident_or(table),
            cols.join(", ")
        ));
    }

    ddl
}

fn quote_ident_or(name: &str) -> String {
    quote_ident(name).unwrap_or_else(|_| format!("[{}]", name))
}

#[async_trait]
impl DatabaseDriver for MssqlDriver {
    async fn close(&self) {}

    async fn test_connection(&self) -> Result<(), String> {
        let rows = self.fetch_rows("SELECT 1").await?;
        if rows.is_empty() {
            return Err("[CONN_FAILED] Empty response".to_string());
        }
        Ok(())
    }

    async fn list_databases(&self) -> Result<Vec<String>, String> {
        let rows = self
            .fetch_rows(
                "SELECT name FROM sys.databases WHERE state = 0 AND name NOT IN ('tempdb') ORDER BY name",
            )
            .await?;

        Ok(rows
            .iter()
            .map(|row| Self::parse_string(row, 0))
            .filter(|s| !s.is_empty())
            .collect())
    }

    async fn list_tables(&self, schema: Option<String>) -> Result<Vec<TableInfo>, String> {
        let schema_filter = schema
            .filter(|s| !s.trim().is_empty())
            .map(|s| format!("AND s.name = '{}'", escape_literal(s.trim())));

        let sql = format!(
            "SELECT s.name AS schema_name, o.name AS table_name, CASE WHEN o.type = 'V' THEN 'view' ELSE 'table' END AS table_type \
             FROM sys.objects o \
             JOIN sys.schemas s ON s.schema_id = o.schema_id \
             WHERE o.type IN ('U','V') {} \
             ORDER BY s.name, o.name",
            schema_filter.unwrap_or_default(),
        );
        let rows = self.fetch_rows(&sql).await?;

        Ok(rows
            .into_iter()
            .map(|row| TableInfo {
                schema: Self::parse_string(&row, 0),
                name: Self::parse_string(&row, 1),
                r#type: Self::parse_string(&row, 2),
            })
            .collect())
    }

    async fn list_routines(&self, schema: Option<String>) -> Result<Vec<RoutineInfo>, String> {
        let schema_filter = schema
            .filter(|s| !s.trim().is_empty())
            .map(|s| format!("AND s.name = '{}'", escape_literal(s.trim())));

        let sql = format!(
            "SELECT s.name AS schema_name, o.name AS routine_name, \
                    CASE WHEN o.type = 'P' THEN 'procedure' ELSE 'function' END AS routine_type \
             FROM sys.objects o \
             JOIN sys.schemas s ON s.schema_id = o.schema_id \
             WHERE o.type IN ('P','FN','IF','TF','FS','FT') \
               AND o.is_ms_shipped = 0 {} \
             ORDER BY s.name, routine_type, o.name",
            schema_filter.unwrap_or_default(),
        );
        let rows = self.fetch_rows(&sql).await?;

        Ok(rows
            .into_iter()
            .map(|row| RoutineInfo {
                schema: Self::parse_string(&row, 0),
                name: Self::parse_string(&row, 1),
                r#type: Self::parse_string(&row, 2),
            })
            .collect())
    }

    async fn get_routine_ddl(
        &self,
        schema: String,
        name: String,
        routine_type: String,
    ) -> Result<String, String> {
        let type_filter = routine_type_sql_filter(&routine_type)?;

        let sql = format!(
            "SELECT m.definition \
             FROM sys.objects o \
             JOIN sys.schemas s ON s.schema_id = o.schema_id \
             JOIN sys.sql_modules m ON m.object_id = o.object_id \
             WHERE s.name = '{}' \
               AND o.name = '{}' \
               AND o.type IN {} \
               AND o.is_ms_shipped = 0",
            escape_literal(&schema),
            escape_literal(&name),
            type_filter
        );
        let rows = self.fetch_rows(&sql).await?;
        let ddl = rows
            .first()
            .map(|row| Self::parse_string(row, 0))
            .unwrap_or_default();

        if ddl.trim().is_empty() {
            return Err(format!(
                "[NOT_FOUND] Routine '{}.{}' does not exist or its definition is not visible",
                schema, name
            ));
        }

        Ok(ddl)
    }

    async fn get_table_structure(
        &self,
        schema: String,
        table: String,
    ) -> Result<TableStructure, String> {
        let pk_sql = format!(
            "SELECT kcu.COLUMN_NAME \
             FROM INFORMATION_SCHEMA.TABLE_CONSTRAINTS tc \
             JOIN INFORMATION_SCHEMA.KEY_COLUMN_USAGE kcu \
               ON tc.CONSTRAINT_NAME = kcu.CONSTRAINT_NAME \
              AND tc.TABLE_SCHEMA = kcu.TABLE_SCHEMA \
              AND tc.TABLE_NAME = kcu.TABLE_NAME \
             WHERE tc.CONSTRAINT_TYPE = 'PRIMARY KEY' \
               AND tc.TABLE_SCHEMA = '{}' \
               AND tc.TABLE_NAME = '{}'",
            escape_literal(&schema),
            escape_literal(&table)
        );
        let pk_rows = self.fetch_rows(&pk_sql).await?;
        let pk_set: HashSet<String> = pk_rows
            .iter()
            .map(|row| Self::parse_string(row, 0))
            .collect();

        let dc_sql = format!(
            "SELECT c.name AS column_name, dc.name AS constraint_name \
             FROM sys.default_constraints dc \
             JOIN sys.columns c \
               ON dc.parent_object_id = c.object_id AND dc.parent_column_id = c.column_id \
             JOIN sys.tables tbl ON tbl.object_id = c.object_id \
             JOIN sys.schemas s ON s.schema_id = tbl.schema_id \
             WHERE s.name = '{}' AND tbl.name = '{}'",
            escape_literal(&schema),
            escape_literal(&table)
        );
        let dc_rows = self.fetch_rows(&dc_sql).await?;
        let dc_map: HashMap<String, String> = dc_rows
            .iter()
            .map(|row| (Self::parse_string(row, 0), Self::parse_string(row, 1)))
            .collect();

        let sql = format!(
            "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE, COLUMN_DEFAULT \
             FROM INFORMATION_SCHEMA.COLUMNS \
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' \
             ORDER BY ORDINAL_POSITION",
            escape_literal(&schema),
            escape_literal(&table)
        );

        let rows = self.fetch_rows(&sql).await?;
        let mut columns = Vec::new();
        for row in rows {
            let name = Self::parse_string(&row, 0);
            let default_raw = Self::parse_string(&row, 3);
            columns.push(ColumnInfo {
                name: name.clone(),
                r#type: Self::parse_string(&row, 1),
                nullable: Self::parse_string(&row, 2).eq_ignore_ascii_case("YES"),
                default_value: if default_raw.is_empty() {
                    None
                } else {
                    Some(default_raw)
                },
                primary_key: pk_set.contains(&name),
                comment: None,
                default_constraint_name: dc_map.get(&name).cloned(),
            });
        }

        Ok(TableStructure { columns })
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

        let indexes = self.load_mssql_indexes(&schema, &table, true).await?;
        let foreign_keys = self.load_mssql_foreign_keys(&schema, &table).await?;

        Ok(TableMetadata {
            columns,
            indexes,
            foreign_keys,
            clickhouse_extra: None,
            special_type_summaries: vec![],
        })
    }

    async fn get_table_ddl(&self, schema: String, table: String) -> Result<String, String> {
        let columns = self.load_mssql_columns(&schema, &table).await?;
        let key_constraints = self.load_mssql_key_constraints(&schema, &table).await?;
        let check_constraints = self.load_mssql_check_constraints(&schema, &table).await?;
        let foreign_keys = self.load_mssql_foreign_keys(&schema, &table).await?;
        let indexes = self.load_mssql_indexes(&schema, &table, false).await?;

        Ok(render_mssql_create_table_ddl(
            &schema,
            &table,
            &columns,
            &key_constraints,
            &check_constraints,
            &foreign_keys,
            &indexes,
        ))
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
        let qualified = table_ref(&schema, &table)?;

        let filter = filter.map(|f| super::normalize_quotes(&f));
        let order_by = order_by.map(|f| super::normalize_quotes(&f));

        let where_clause = match &filter {
            Some(f) if !f.trim().is_empty() => format!(" WHERE {}", f.trim()),
            _ => String::new(),
        };

        let count_sql = format!(
            "SELECT COUNT_BIG(1) AS total FROM {}{}",
            qualified, where_clause
        );
        let count_rows = self.fetch_rows(&count_sql).await?;
        let total = count_rows
            .first()
            .map(|row| Self::parse_i64(row, 0))
            .unwrap_or(0);

        let order_clause = if let Some(ref raw) = order_by {
            if raw.trim().is_empty() {
                " ORDER BY (SELECT NULL)".to_string()
            } else {
                format!(" ORDER BY {}", raw.trim())
            }
        } else if let Some(ref col) = sort_column {
            let dir = if matches!(sort_direction.as_deref(), Some("desc")) {
                "DESC"
            } else {
                "ASC"
            };
            format!(" ORDER BY {} {}", quote_ident(col)?, dir)
        } else {
            " ORDER BY (SELECT NULL)".to_string()
        };

        let sql = if offset == 0 {
            // Simple TOP query for first page (compatible with all SQL Server versions)
            format!(
                "SELECT TOP ({}) * FROM {}{}{}",
                safe_limit, qualified, where_clause, order_clause
            )
        } else {
            // ROW_NUMBER() based pagination for subsequent pages (compatible with SQL Server 2005+)
            // Extract the ORDER BY columns for ROW_NUMBER() OVER clause
            let row_num_order = if order_clause.trim().is_empty() || order_clause.contains("SELECT NULL") {
                "(SELECT NULL)".to_string()
            } else {
                // Remove "ORDER BY" prefix to get just the columns
                order_clause.strip_prefix(" ORDER BY").unwrap_or(&order_clause).trim().to_string()
            };

            format!(
                "SELECT * FROM ( \
                    SELECT TOP ({}) *, ROW_NUMBER() OVER (ORDER BY {}) AS __row_num \
                    FROM {}{} \
                ) AS __paged \
                WHERE __row_num > {} \
                ORDER BY __row_num",
                offset + safe_limit, row_num_order, qualified, where_clause, offset
            )
        };
        let (data, _columns) = self.fetch_query_result_json(&sql).await?;

        Ok(TableDataResponse {
            data,
            total,
            page: safe_page,
            limit: safe_limit,
            execution_time_ms: start.elapsed().as_millis() as i64,
        })
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
        let first_keyword = first_sql_keyword(&sql);
        let is_read_query = matches!(
            first_keyword.as_deref(),
            Some("select") | Some("with") | Some("show") | Some("exec") | Some("execute")
        );

        if is_read_query {
            let (data, columns) = self.fetch_query_result_json(&sql).await?;

            return Ok(QueryResult {
                row_count: data.len() as i64,
                data,
                columns,
                time_taken_ms: start.elapsed().as_millis() as i64,
                success: true,
                error: None,
            });
        }

        let mut client = self.pool.get().await.map_err(map_pool_error)?;
        let result = client
            .execute(&sql, &[])
            .await
            .map_err(|e| format!("[QUERY_ERROR] {}", e))?;
        let row_count = result.rows_affected().iter().sum::<u64>() as i64;

        Ok(QueryResult {
            data: vec![],
            row_count,
            columns: vec![],
            time_taken_ms: start.elapsed().as_millis() as i64,
            success: true,
            error: None,
        })
    }

    async fn get_schema_overview(&self, schema: Option<String>) -> Result<SchemaOverview, String> {
        let sql = if let Some(schema_name) = schema.filter(|s| !s.trim().is_empty()) {
            format!(
                "SELECT TABLE_SCHEMA, TABLE_NAME, COLUMN_NAME, DATA_TYPE \
                 FROM INFORMATION_SCHEMA.COLUMNS \
                 WHERE TABLE_SCHEMA = '{}' \
                 ORDER BY TABLE_SCHEMA, TABLE_NAME, ORDINAL_POSITION",
                escape_literal(schema_name.trim())
            )
        } else {
            "SELECT TABLE_SCHEMA, TABLE_NAME, COLUMN_NAME, DATA_TYPE \
             FROM INFORMATION_SCHEMA.COLUMNS \
             WHERE TABLE_SCHEMA NOT IN ('INFORMATION_SCHEMA', 'sys') \
             ORDER BY TABLE_SCHEMA, TABLE_NAME, ORDINAL_POSITION"
                .to_string()
        };

        let rows = self.fetch_rows(&sql).await?;
        let mut table_map: HashMap<(String, String), Vec<ColumnSchema>> = HashMap::new();

        for row in rows {
            let schema_name = Self::parse_string(&row, 0);
            let table_name = Self::parse_string(&row, 1);
            let col_name = Self::parse_string(&row, 2);
            let col_type = Self::parse_string(&row, 3);

            table_map
                .entry((schema_name, table_name))
                .or_default()
                .push(ColumnSchema {
                    name: col_name,
                    r#type: col_type,
                });
        }

        let mut tables = table_map
            .into_iter()
            .map(|((schema, name), columns)| TableSchema {
                schema,
                name,
                columns,
            })
            .collect::<Vec<_>>();

        tables.sort_by(|a, b| a.schema.cmp(&b.schema).then(a.name.cmp(&b.name)));
        Ok(SchemaOverview { tables })
    }
}
