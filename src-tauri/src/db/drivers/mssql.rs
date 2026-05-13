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
use tiberius::{AuthMethod, Client, ColumnData, Config, EncryptionLevel, QueryItem, Row};
use tiberius::SqlBrowser;
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use crate::ssh::SshTunnel;

pub struct MssqlDriver {
    pub pool: Pool<MssqlConnectionManager>,
    pub ssh_tunnel: Option<SshTunnel>,
    /// Whether the server supports FOR JSON (SQL Server 2016+, major version >= 13).
    /// When false, query results are read directly from tiberius Row instead.
    pub supports_for_json: bool,
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
    auth_mode: Option<String>,
    instance_name: Option<String>,
}

fn build_config(form: &ConnectionForm) -> Result<MssqlConfig, String> {
    let raw_host = form
        .host
        .clone()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.trim().is_empty())
        .ok_or("[VALIDATION_ERROR] host cannot be empty")?;

    // Parse SQL Server named instance format: HOST\INSTANCE_NAME
    let (host, instance_name) = if let Some((h, inst)) = raw_host.rsplit_once('\\') {
        let h = h.trim().to_string();
        let inst = inst.trim().to_string();
        if h.is_empty() || inst.is_empty() {
            return Err("[VALIDATION_ERROR] invalid host\\instance format".to_string());
        }
        (h, Some(inst))
    } else {
        (raw_host, None)
    };

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
    let auth_mode = form.auth_mode.clone().map(|v| v.trim().to_string());

    // Platform compatibility check for Windows-specific auth modes.
    if let Some(ref mode) = auth_mode {
        if mode.eq_ignore_ascii_case("windows") && !cfg!(target_os = "windows") {
            return Err(
                "[VALIDATION_ERROR] Windows authentication is only available on Windows. \
                 Please use SQL Server authentication or Integrated authentication instead."
                    .to_string(),
            );
        }
    }

    let username = form
        .username
        .clone()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_default();
    let password = form.password.clone().unwrap_or_default();

    Ok(MssqlConfig {
        host,
        port: port as u16,
        database,
        username,
        password,
        ssl: form.ssl.unwrap_or(false),
        auth_mode,
        instance_name,
    })
}

async fn detect_for_json_support(client: &mut Client<Compat<TcpStream>>) -> bool {
    let Ok(mut stream) = client
        .simple_query("SELECT CAST(SERVERPROPERTY('ProductMajorVersion') AS VARCHAR(10))")
        .await
    else {
        return false;
    };
    while let Ok(Some(item)) = stream.try_next().await {
        if let QueryItem::Row(row) = item {
            if let Ok(Some(v)) = row.try_get::<&str, _>(0) {
                if let Ok(major) = v.trim().parse::<u32>() {
                    return major >= 13;
                }
            }
        }
    }
    // Could not determine version (e.g. SQL Server 2008/R2 where
    // ProductMajorVersion is not available). Assume FOR JSON is NOT
    // supported — the safest default.
    false
}

fn column_data_to_json(col: &ColumnData<'static>) -> serde_json::Value {
    match col {
        ColumnData::U8(None)
        | ColumnData::I16(None)
        | ColumnData::I32(None)
        | ColumnData::I64(None)
        | ColumnData::F32(None)
        | ColumnData::F64(None)
        | ColumnData::Bit(None)
        | ColumnData::String(None)
        | ColumnData::Guid(None)
        | ColumnData::Binary(None)
        | ColumnData::Numeric(None)
        | ColumnData::Xml(None)
        | ColumnData::DateTime(None)
        | ColumnData::SmallDateTime(None)
        | ColumnData::Time(None)
        | ColumnData::Date(None)
        | ColumnData::DateTime2(None)
        | ColumnData::DateTimeOffset(None) => serde_json::Value::Null,
        ColumnData::U8(Some(v)) => serde_json::json!(v),
        ColumnData::I16(Some(v)) => serde_json::json!(v),
        ColumnData::I32(Some(v)) => serde_json::json!(v),
        ColumnData::I64(Some(v)) => serde_json::json!(v),
        ColumnData::F32(Some(v)) => serde_json::json!(v),
        ColumnData::F64(Some(v)) => serde_json::json!(v),
        ColumnData::Bit(Some(v)) => serde_json::json!(v),
        ColumnData::String(Some(v)) => serde_json::json!(*v),
        ColumnData::Guid(Some(v)) => serde_json::json!(v.to_string()),
        ColumnData::Binary(Some(v)) => {
            serde_json::json!(v.iter().map(|b| format!("{:02x}", b)).collect::<String>())
        }
        ColumnData::Numeric(Some(v)) => serde_json::json!(v.to_string()),
        ColumnData::Xml(Some(v)) => serde_json::json!(v.to_string()),
        ColumnData::DateTime(Some(v)) => {
            let base = chrono::NaiveDate::from_ymd_opt(1900, 1, 1).unwrap();
            let date = base + chrono::Duration::days(v.days() as i64);
            let ns = (v.seconds_fragments() as i64) * (1e9 as i64) / 300;
            let time = chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()
                + chrono::Duration::nanoseconds(ns);
            serde_json::json!(chrono::NaiveDateTime::new(date, time).format("%Y-%m-%dT%H:%M:%S%.3f").to_string())
        }
        ColumnData::SmallDateTime(Some(v)) => {
            let base = chrono::NaiveDate::from_ymd_opt(1900, 1, 1).unwrap();
            let date = base + chrono::Duration::days(v.days() as i64);
            let time = chrono::NaiveTime::from_num_seconds_from_midnight_opt(v.seconds_fragments() as u32, 0).unwrap_or_default();
            serde_json::json!(chrono::NaiveDateTime::new(date, time).format("%Y-%m-%dT%H:%M:%S").to_string())
        }
        ColumnData::Time(Some(v)) => {
            let ns = v.increments() as i64 * 10i64.pow(9 - v.scale() as u32);
            let time = chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()
                + chrono::Duration::nanoseconds(ns);
            serde_json::json!(time.format("%H:%M:%S%.f").to_string())
        }
        ColumnData::Date(Some(v)) => {
            let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).unwrap();
            let date = base + chrono::Duration::days(v.days() as i64);
            serde_json::json!(date.format("%Y-%m-%d").to_string())
        }
        ColumnData::DateTime2(Some(v)) => {
            let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).unwrap();
            let date = base + chrono::Duration::days(v.date().days() as i64);
            let t = v.time();
            let ns = t.increments() as i64 * 10i64.pow(9 - t.scale() as u32);
            let time = chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()
                + chrono::Duration::nanoseconds(ns);
            serde_json::json!(chrono::NaiveDateTime::new(date, time).format("%Y-%m-%dT%H:%M:%S%.f").to_string())
        }
        ColumnData::DateTimeOffset(Some(v)) => {
            let base = chrono::NaiveDate::from_ymd_opt(1, 1, 1).unwrap();
            let dt2 = v.datetime2();
            let date = base + chrono::Duration::days(dt2.date().days() as i64);
            let t = dt2.time();
            let ns = t.increments() as i64 * 10i64.pow(9 - t.scale() as u32);
            let time = chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()
                + chrono::Duration::nanoseconds(ns);
            let offset_mins = v.offset() as i32;
            let sign = if offset_mins < 0 { "-" } else { "+" };
            let abs = offset_mins.abs();
            serde_json::json!(format!(
                "{}{}{:02}:{:02}",
                chrono::NaiveDateTime::new(date, time).format("%Y-%m-%dT%H:%M:%S%.f"),
                sign,
                abs / 60,
                abs % 60
            ))
        }
        #[allow(unreachable_patterns)]
        _ => {
            // Defensive fallback for any future ColumnData variants or unsupported
            // types that tiberius might expose (e.g. SQL_VARIANT, GEOMETRY, etc.).
            serde_json::Value::Null
        }
    }
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



/// Check whether the SQL already contains a `FOR JSON` clause.
/// Scans the statement while skipping string literals and comments.
fn already_has_for_json(sql: &str) -> bool {
    let upper = sql.to_uppercase();
    let bytes = upper.as_bytes();
    let mut i = 0;

    while i + 8 <= bytes.len() {
        match bytes[i] {
            b'\'' => {
                // Skip over a single-quoted string literal.
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\'' {
                        i += 1;
                        // SQL Server escapes quotes by doubling them
                        if i < bytes.len() && bytes[i] == b'\'' {
                            continue;
                        }
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            b'-' if i + 1 < bytes.len() && bytes[i + 1] == b'-' => {
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                if i + 1 < bytes.len() {
                    i += 2;
                }
                continue;
            }
            _ => {
                if bytes[i..].starts_with(b"FOR JSON") {
                    let before_ok = i == 0 || {
                        let ch = bytes[i - 1];
                        !ch.is_ascii_alphabetic() && ch != b'_'
                    };
                    let after = i + 8;
                    let after_ok = after >= bytes.len() || {
                        let ch = bytes[after];
                        !ch.is_ascii_alphabetic() && ch != b'_'
                    };
                    if before_ok && after_ok {
                        return true;
                    }
                }
            }
        }
        i += 1;
    }
    false
}

/// Scan the SQL statement for a keyword that appears at "top level"
/// (depth == 0, outside of parentheses and comments).
fn has_top_level_keyword(sql: &str, keyword: &str) -> bool {
    let upper = sql.to_uppercase();
    let kw_bytes = keyword.to_uppercase().as_bytes().to_vec();
    let kw_len = kw_bytes.len();
    let bytes = upper.as_bytes();
    let mut depth: i32 = 0;
    let mut i = 0;

    while i + kw_len <= bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b'\'' => {
                // Skip over a single-quoted string literal.
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\'' {
                        i += 1;
                        // SQL Server escapes quotes by doubling them
                        if i < bytes.len() && bytes[i] == b'\'' {
                            continue;
                        }
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            b'-' if i + 1 < bytes.len() && bytes[i + 1] == b'-' => {
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                if i + 1 < bytes.len() {
                    i += 2;
                }
                continue;
            }
            _ if depth == 0 => {
                if bytes[i..].starts_with(&kw_bytes) {
                    let after = i + kw_len;
                    let after_ok = after >= bytes.len() || {
                        let ch = bytes[after];
                        !ch.is_ascii_alphabetic() && ch != b'_'
                    };
                    if after_ok {
                        return true;
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    false
}

fn has_top_level_union(sql: &str) -> bool {
    has_top_level_keyword(sql, "UNION")
}

/// Determine whether a query is safe to rewrite with FOR JSON PATH.
/// Returns false for CTEs, UNION, EXEC, and other constructs where
/// appending FOR JSON would produce invalid T-SQL.
fn is_for_json_safe(sql: &str) -> bool {
    let first = super::first_sql_keyword(sql);
    if !matches!(first.as_deref(), Some("SELECT")) {
        return false;
    }
    // UNION at the top level requires wrapping, which we don't do.
    if has_top_level_union(sql) {
        return false;
    }
    true
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
        if let Some(ref instance) = self.config.instance_name {
            config.instance_name(instance);
        }
        let auth = self.build_auth_method();
        config.authentication(auth);
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

    fn build_auth_method(&self) -> AuthMethod {
        let mode = self.config.auth_mode.as_deref().unwrap_or("sql_server");
        match mode {
            "integrated" => AuthMethod::Integrated,
            "windows" => {
                #[cfg(target_os = "windows")]
                {
                    AuthMethod::windows(
                        self.config.username.clone(),
                        self.config.password.clone(),
                    )
                }
                #[cfg(not(target_os = "windows"))]
                {
                    // Fallback that will fail at connection time with a clear message.
                    // We return sql_server here so compilation succeeds on all platforms;
                    // the actual check is done in connect_single.
                    AuthMethod::sql_server(
                        self.config.username.clone(),
                        self.config.password.clone(),
                    )
                }
            }
            "aad_token" => AuthMethod::aad_token(self.config.password.clone()),
            _ => AuthMethod::sql_server(
                self.config.username.clone(),
                self.config.password.clone(),
            ),
        }
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
            // Use SqlBrowser to resolve named instance port when instance_name is set.
            // If no instance_name is configured, connect_named falls back to direct TCP.
            let tcp = TcpStream::connect_named(&config)
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

/// Build a SELECT column list for SQL Server, automatically casting unsupported
/// types (sql_variant, geometry, geography, hierarchyid) to NVARCHAR(MAX)
/// so that tiberius can read them without panicking on `todo!()` for Udt/SSVariant.
fn build_mssql_select_list(columns: &[(String, String)]) -> Result<String, String> {
    let mut parts = Vec::new();
    for (name, data_type) in columns {
        let ident = quote_ident(name)?;
        let dt = data_type.to_ascii_lowercase();
        let expr = match dt.as_str() {
            "sql_variant" | "geometry" | "geography" | "hierarchyid" => {
                format!("CAST({} AS NVARCHAR(MAX)) AS {}", ident, ident)
            }
            _ => ident,
        };
        parts.push(expr);
    }
    Ok(parts.join(", "))
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

        let supports_for_json = {
            let mut client = pool.get().await.map_err(map_pool_error)?;
            detect_for_json_support(&mut client).await
        };
        let driver = Self {
            pool,
            ssh_tunnel,
            supports_for_json,
        };
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

    /// Execute a single query, collecting both column metadata and JSON row data.
    /// Uses FOR JSON on SQL Server 2016+ when the query is simple enough to be
    /// safely rewritten; falls back to direct Row conversion for older versions
    /// or complex queries (CTEs, UNION, EXEC, etc.).
    async fn fetch_query_result_json(
        &self,
        sql: &str,
    ) -> Result<(Vec<serde_json::Value>, Vec<QueryColumn>), String> {
        if self.supports_for_json && is_for_json_safe(sql) {
            self.fetch_query_result_for_json(sql).await
        } else {
            self.fetch_query_result_direct(sql).await
        }
    }

    /// Direct row-to-JSON conversion for SQL Server < 2016 (no FOR JSON support).
    async fn fetch_query_result_direct(
        &self,
        sql: &str,
    ) -> Result<(Vec<serde_json::Value>, Vec<QueryColumn>), String> {
        let mut client = self.pool.get().await.map_err(map_pool_error)?;
        let mut stream = client
            .simple_query(sql)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {}", e))?;

        let mut columns: Vec<QueryColumn> = Vec::new();
        let mut high_precision_cols = HashSet::new();
        let mut data: Vec<serde_json::Value> = Vec::new();

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
                    let cells: Vec<_> = row.cells().map(|(_, c)| c).collect();
                    let mut obj = serde_json::Map::new();
                    for (i, col) in columns.iter().enumerate() {
                        if let Some(cell) = cells.get(i) {
                            let mut val = column_data_to_json(cell);
                            if high_precision_cols.contains(&col.name) && val.is_number() {
                                val = serde_json::Value::String(val.to_string());
                            }
                            obj.insert(col.name.clone(), val);
                        }
                    }
                    data.push(serde_json::Value::Object(obj));
                }
                _ => {}
            }
        }

        Ok((data, columns))
    }

    /// FOR JSON path for SQL Server 2016+.
    async fn fetch_query_result_for_json(
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
        if already_has_for_json(trimmed) {
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
        if let Ok(Some(v)) = row.try_get::<bool, _>(idx) {
            return if v { 1 } else { 0 };
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
        already_has_for_json, has_top_level_union, is_for_json_safe,
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

    #[test]
    fn test_already_has_for_json_detects_variants() {
        assert!(already_has_for_json("SELECT 1 FOR JSON PATH"));
        assert!(already_has_for_json("SELECT 1 FOR JSON AUTO"));
        assert!(already_has_for_json("SELECT 1 FOR JSON EXPLICIT"));
        assert!(already_has_for_json("SELECT 1 FOR JSON PATH, WITHOUT_ARRAY_WRAPPER"));
        assert!(already_has_for_json("SELECT 1 FOR JSON PATH, ROOT('data')"));
        assert!(already_has_for_json("SELECT 1 FOR JSON PATH, INCLUDE_NULL_VALUES"));
        // Should not match partial words
        assert!(!already_has_for_json("SELECT * FROM performance_json_log"));
        assert!(!already_has_for_json("SELECT 'FOR JSON' AS label"));
    }

    #[test]
    fn test_has_top_level_union_detects_union() {
        assert!(has_top_level_union("SELECT 1 UNION SELECT 2"));
        assert!(has_top_level_union("SELECT 1 UNION ALL SELECT 2"));
        assert!(!has_top_level_union("SELECT * FROM (SELECT 1 UNION SELECT 2) AS t"));
        assert!(!has_top_level_union("SELECT 'union' AS word"));
    }

    #[test]
    fn test_is_for_json_safe() {
        // Safe: simple SELECT
        assert!(is_for_json_safe("SELECT * FROM users"));
        assert!(is_for_json_safe("  -- comment\nSELECT id FROM t"));

        // Unsafe: CTE
        assert!(!is_for_json_safe("WITH cte AS (SELECT 1) SELECT * FROM cte"));

        // Unsafe: UNION
        assert!(!is_for_json_safe("SELECT 1 UNION SELECT 2"));

        // Unsafe: EXEC
        assert!(!is_for_json_safe("EXEC dbo.MyProc"));
        assert!(!is_for_json_safe("EXECUTE dbo.MyProc"));

        // Unsafe: subquery as standalone statement (starts with paren)
        // Actually first_sql_keyword would return None here, so it's unsafe.
        // But more importantly, INSERT/UPDATE/DELETE are unsafe.
        assert!(!is_for_json_safe("INSERT INTO t VALUES (1)"));
        assert!(!is_for_json_safe("UPDATE t SET x = 1"));
        assert!(!is_for_json_safe("DELETE FROM t"));
    }

    #[test]
    fn test_build_for_json_query_preserves_existing_for_json() {
        let sql = "SELECT 1 FOR JSON PATH, ROOT('data')";
        assert_eq!(
            MssqlDriver::build_for_json_query(sql),
            "SELECT 1 FOR JSON PATH, ROOT('data')"
        );

        let sql2 = "SELECT 1 FOR JSON PATH, INCLUDE_NULL_VALUES";
        assert_eq!(
            MssqlDriver::build_for_json_query(sql2),
            "SELECT 1 FOR JSON PATH, INCLUDE_NULL_VALUES"
        );
    }

    #[test]
    fn test_build_config_parses_named_instance() {
        use crate::models::ConnectionForm;

        let form = ConnectionForm {
            driver: "mssql".to_string(),
            host: Some("192.168.1.10\\SQLEXPRESS".to_string()),
            port: Some(1433),
            username: Some("sa".to_string()),
            password: Some("pass".to_string()),
            ..Default::default()
        };
        let cfg = super::build_config(&form).unwrap();
        assert_eq!(cfg.host, "192.168.1.10");
        assert_eq!(cfg.instance_name, Some("SQLEXPRESS".to_string()));
        assert_eq!(cfg.port, 1433);
    }

    #[test]
    fn test_build_config_parses_plain_host() {
        use crate::models::ConnectionForm;

        let form = ConnectionForm {
            driver: "mssql".to_string(),
            host: Some("sql-server.example.com".to_string()),
            port: Some(1433),
            username: Some("sa".to_string()),
            password: Some("pass".to_string()),
            ..Default::default()
        };
        let cfg = super::build_config(&form).unwrap();
        assert_eq!(cfg.host, "sql-server.example.com");
        assert_eq!(cfg.instance_name, None);
    }

    #[test]
    fn test_build_config_rejects_invalid_instance_format() {
        use crate::models::ConnectionForm;

        let form = ConnectionForm {
            driver: "mssql".to_string(),
            host: Some("192.168.1.10\\".to_string()),
            port: Some(1433),
            username: Some("sa".to_string()),
            password: Some("pass".to_string()),
            ..Default::default()
        };
        assert!(super::build_config(&form).is_err());
    }

    #[test]
    fn test_mssql_full_type_string_varchar() {
        assert_eq!(super::mssql_full_type_string("varchar", 255, 0, 0), "varchar(255)");
        assert_eq!(super::mssql_full_type_string("varchar", -1, 0, 0), "varchar(MAX)");
        assert_eq!(super::mssql_full_type_string("char", 10, 0, 0), "char(10)");
        assert_eq!(super::mssql_full_type_string("varbinary", -1, 0, 0), "varbinary(MAX)");
        assert_eq!(super::mssql_full_type_string("binary", 16, 0, 0), "binary(16)");
    }

    #[test]
    fn test_mssql_full_type_string_nvarchar() {
        // nvarchar max_length is in bytes (2 bytes per char)
        assert_eq!(super::mssql_full_type_string("nvarchar", 100, 0, 0), "nvarchar(50)");
        assert_eq!(super::mssql_full_type_string("nvarchar", -1, 0, 0), "nvarchar(MAX)");
        assert_eq!(super::mssql_full_type_string("nchar", 20, 0, 0), "nchar(10)");
    }

    #[test]
    fn test_mssql_full_type_string_decimal() {
        assert_eq!(super::mssql_full_type_string("decimal", 0, 10, 2), "decimal(10,2)");
        assert_eq!(super::mssql_full_type_string("numeric", 0, 18, 0), "numeric(18,0)");
    }

    #[test]
    fn test_mssql_full_type_string_datetime_with_scale() {
        assert_eq!(super::mssql_full_type_string("datetime2", 0, 0, 7), "datetime2(7)");
        assert_eq!(super::mssql_full_type_string("datetime2", 0, 0, 0), "datetime2");
        assert_eq!(super::mssql_full_type_string("datetimeoffset", 0, 0, 3), "datetimeoffset(3)");
        assert_eq!(super::mssql_full_type_string("time", 0, 0, 4), "time(4)");
    }

    #[test]
    fn test_mssql_full_type_string_passthrough() {
        assert_eq!(super::mssql_full_type_string("int", 0, 0, 0), "int");
        assert_eq!(super::mssql_full_type_string("bigint", 0, 0, 0), "bigint");
        assert_eq!(super::mssql_full_type_string("bit", 0, 0, 0), "bit");
        assert_eq!(super::mssql_full_type_string("uniqueidentifier", 0, 0, 0), "uniqueidentifier");
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
            "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE, COLUMN_DEFAULT, \
                    CHARACTER_MAXIMUM_LENGTH, NUMERIC_PRECISION, NUMERIC_SCALE \
             FROM INFORMATION_SCHEMA.COLUMNS \
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' \
             ORDER BY ORDINAL_POSITION",
            escape_literal(&schema),
            escape_literal(&table)
        );

        let rows = self.fetch_rows(&sql).await?;

        let comment_sql = format!(
            "SELECT c.name, CAST(ep.value AS NVARCHAR(4000)) \
             FROM sys.extended_properties ep \
             JOIN sys.columns c \
               ON ep.major_id = c.object_id AND ep.minor_id = c.column_id \
             JOIN sys.tables t ON t.object_id = c.object_id \
             JOIN sys.schemas s ON s.schema_id = t.schema_id \
             WHERE ep.name = 'MS_Description' \
               AND s.name = '{}' AND t.name = '{}'",
            escape_literal(&schema),
            escape_literal(&table)
        );
        let comment_rows = self.fetch_rows(&comment_sql).await?;
        let comment_map: HashMap<String, String> = comment_rows
            .iter()
            .filter_map(|row| {
                let val = Self::parse_string(row, 1);
                if val.is_empty() {
                    None
                } else {
                    Some((Self::parse_string(row, 0), val))
                }
            })
            .collect();

        let mut columns = Vec::new();
        for row in rows {
            let name = Self::parse_string(&row, 0);
            let data_type = Self::parse_string(&row, 1);
            let max_length = Self::parse_i64(&row, 4);
            let precision = Self::parse_i64(&row, 5);
            let scale = Self::parse_i64(&row, 6);
            let full_type = mssql_full_type_string(&data_type, max_length, precision, scale);
            let default_raw = Self::parse_string(&row, 3);
            columns.push(ColumnInfo {
                name: name.clone(),
                r#type: full_type,
                nullable: Self::parse_string(&row, 2).eq_ignore_ascii_case("YES"),
                default_value: if default_raw.is_empty() {
                    None
                } else {
                    Some(default_raw)
                },
                primary_key: pk_set.contains(&name),
                comment: comment_map.get(&name).cloned(),
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

        // Build explicit column list, casting unsupported types (sql_variant,
        // geometry, geography, hierarchyid) to NVARCHAR(MAX) so tiberius can
        // read them without panicking on `todo!()` for Udt/SSVariant.
        let column_sql = format!(
            "SELECT c.name, t.name AS data_type \
             FROM sys.columns c \
             JOIN sys.types t ON c.user_type_id = t.user_type_id \
             JOIN sys.tables tbl ON tbl.object_id = c.object_id \
             JOIN sys.schemas s ON s.schema_id = tbl.schema_id \
             WHERE s.name = '{}' AND tbl.name = '{}' \
             ORDER BY c.column_id",
            escape_literal(&schema),
            escape_literal(&table)
        );
        let col_rows = self.fetch_rows(&column_sql).await?;
        let mut col_list = Vec::new();
        for row in &col_rows {
            let name = Self::parse_string(row, 0);
            let data_type = Self::parse_string(row, 1);
            col_list.push((name, data_type));
        }
        let select_list = build_mssql_select_list(&col_list)?;

        let sql = if offset == 0 {
            // Simple TOP query for first page (compatible with all SQL Server versions)
            format!(
                "SELECT TOP ({}) {} FROM {}{}{}",
                safe_limit, select_list, qualified, where_clause, order_clause
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
                    SELECT TOP ({}) {}, ROW_NUMBER() OVER (ORDER BY {}) AS __row_num \
                    FROM {}{} \
                ) AS __paged \
                WHERE __row_num > {} \
                ORDER BY __row_num",
                offset + safe_limit, select_list, row_num_order, qualified, where_clause, offset
            )
        };
        let (mut data, mut columns) = self.fetch_query_result_json(&sql).await?;

        // Filter out the internal __row_num column so it's not exposed to users
        if let Some(idx) = columns.iter().position(|c| c.name == "__row_num") {
            columns.remove(idx);
            for row in &mut data {
                if let serde_json::Value::Object(obj) = row {
                    obj.remove("__row_num");
                }
            }
        }

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
        let statements = super::split_sql_statements(&sql);
        if statements.is_empty() {
            return Err("[QUERY_ERROR] Empty SQL statement".to_string());
        }

        // Execute all statements except the last one
        if statements.len() > 1 {
            for statement in statements.iter().take(statements.len() - 1) {
                let mut client = self.pool.get().await.map_err(map_pool_error)?;
                client
                    .execute(statement, &[])
                    .await
                    .map_err(|e| format!("[QUERY_ERROR] {}", e))?;
            }
        }

        // Execute the last statement and return its result
        let last_sql = statements.last().unwrap();
        let first_keyword = super::first_sql_keyword(last_sql);
        let is_read_query = matches!(
            first_keyword.as_deref(),
            Some("SELECT") | Some("WITH") | Some("SHOW") | Some("EXEC") | Some("EXECUTE")
        );

        if is_read_query {
            let (data, columns) = self.fetch_query_result_json(last_sql).await?;

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
            .execute(last_sql, &[])
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
