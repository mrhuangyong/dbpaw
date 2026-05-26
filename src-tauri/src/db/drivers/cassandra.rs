use super::{conn_failed_error, DatabaseDriver};
use crate::models::{
    CassandraTableExtra, ColumnInfo, ColumnSchema, ConnectionForm, IndexInfo, QueryColumn,
    QueryResult, RoutineInfo, SchemaOverview, TableDataResponse, TableInfo,
    TableMetadata, TableSchema, TableStructure,
};
use async_trait::async_trait;
use chrono::{NaiveDate, NaiveTime, TimeZone, Utc};
use scylla::client::session::Session;
use scylla::client::session_builder::SessionBuilder;
use scylla::cluster::metadata::{CollectionType, ColumnType, NativeType};
use scylla::value::CqlValue;
use serde_json::Value;
use std::time::{Duration, Instant};

const DEFAULT_CASSANDRA_PORT: i64 = 9042;
const DEFAULT_CONNECT_TIMEOUT_MS: i64 = 5000;

pub struct CassandraDriver {
    session: Session,
    default_keyspace: String,
    #[allow(dead_code)]
    ssh_tunnel: Option<crate::ssh::SshTunnel>,
}

fn normalize_cassandra_error(e: impl std::fmt::Display) -> String {
    let msg = e.to_string();
    let lower = msg.to_ascii_lowercase();

    if lower.contains("authentication") || lower.contains("credentials") {
        format!("[CASSANDRA_ERROR] Authentication failed: {}", msg)
    } else if lower.contains("refused") || lower.contains("connect") {
        format!("[CASSANDRA_ERROR] Connection refused: {}", msg)
    } else if lower.contains("timeout") || lower.contains("timed out") {
        format!("[CASSANDRA_ERROR] Connection timed out: {}", msg)
    } else if lower.contains("resolve") || lower.contains("lookup") || lower.contains("dns") {
        format!("[CASSANDRA_ERROR] DNS resolution failed: {}", msg)
    } else if lower.contains("tls") || lower.contains("ssl") || lower.contains("certificate") {
        format!("[CASSANDRA_ERROR] TLS/SSL error: {}", msg)
    } else {
        format!("[CASSANDRA_ERROR] {}", msg)
    }
}

/// Convert big-endian two's complement bytes to a signed decimal string.
fn bytes_to_signed_bigint_string(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "0".to_string();
    }
    let negative = bytes[0] & 0x80 != 0;
    if negative {
        // Two's complement: invert all bytes and add 1
        let mut inverted: Vec<u8> = bytes.iter().map(|b| !b).collect();
        let mut carry = 1u8;
        for i in (0..inverted.len()).rev() {
            let (sum, overflow) = inverted[i].overflowing_add(carry);
            inverted[i] = sum;
            carry = overflow as u8;
        }
        let num = unsigned_bytes_to_decimal(&inverted);
        format!("-{}", num)
    } else {
        unsigned_bytes_to_decimal(bytes)
    }
}

fn unsigned_bytes_to_decimal(bytes: &[u8]) -> String {
    if bytes.is_empty() || bytes.iter().all(|&b| b == 0) {
        return "0".to_string();
    }
    // Simple base-256 to base-10 conversion
    let mut digits: Vec<u8> = bytes.to_vec();
    let mut result = String::new();
    while !digits.is_empty() && !digits.iter().all(|&b| b == 0) {
        let mut remainder = 0u16;
        for d in digits.iter_mut() {
            let val = (remainder << 8) | (*d as u16);
            *d = (val / 10) as u8;
            remainder = val % 10;
        }
        result.push((b'0' + remainder as u8) as char);
        // Remove leading zeros
        while digits.first() == Some(&0) {
            digits.remove(0);
        }
    }
    result.chars().rev().collect()
}

fn column_type_to_string(ct: &ColumnType) -> String {
    match ct {
        ColumnType::Native(native) => match native {
            NativeType::Ascii => "ascii",
            NativeType::Boolean => "boolean",
            NativeType::Blob => "blob",
            NativeType::Counter => "counter",
            NativeType::Date => "date",
            NativeType::Decimal => "decimal",
            NativeType::Double => "double",
            NativeType::Duration => "duration",
            NativeType::Float => "float",
            NativeType::Int => "int",
            NativeType::BigInt => "bigint",
            NativeType::SmallInt => "smallint",
            NativeType::TinyInt => "tinyint",
            NativeType::Text => "text",
            NativeType::Timestamp => "timestamp",
            NativeType::Uuid => "uuid",
            NativeType::Timeuuid => "timeuuid",
            NativeType::Varint => "varint",
            NativeType::Inet => "inet",
            NativeType::Time => "time",
            _ => "unknown",
        }
        .to_string(),
        ColumnType::Collection { typ, frozen } => {
            let inner = match typ {
                CollectionType::List(inner) => format!("list<{}>", column_type_to_string(inner)),
                CollectionType::Set(inner) => format!("set<{}>", column_type_to_string(inner)),
                CollectionType::Map(k, v) => {
                    format!(
                        "map<{}, {}>",
                        column_type_to_string(k),
                        column_type_to_string(v)
                    )
                }
                _ => "collection".to_string(),
            };
            if *frozen {
                format!("frozen<{}>", inner)
            } else {
                inner
            }
        }
        ColumnType::Tuple(types) => {
            let inner: Vec<String> = types.iter().map(column_type_to_string).collect();
            format!("tuple<{}>", inner.join(", "))
        }
        ColumnType::Vector { typ, dimensions } => {
            format!("vector<{}, {}>", column_type_to_string(typ), dimensions)
        }
        ColumnType::UserDefinedType { definition, .. } => {
            format!("{}.{}", definition.keyspace, definition.name)
        }
        _ => "unknown".to_string(),
    }
}

fn cql_value_to_json(val: Option<&CqlValue>) -> Value {
    match val {
        None => Value::Null,
        Some(cql) => match cql {
            CqlValue::Ascii(s) | CqlValue::Text(s) => Value::String(s.clone()),
            CqlValue::Boolean(b) => Value::Bool(*b),
            CqlValue::Int(n) => serde_json::json!(*n),
            CqlValue::BigInt(n) => serde_json::json!(*n),
            CqlValue::SmallInt(n) => serde_json::json!(*n),
            CqlValue::TinyInt(n) => serde_json::json!(*n),
            CqlValue::Float(f) => serde_json::json!(*f),
            CqlValue::Double(d) => serde_json::json!(*d),
            CqlValue::Uuid(u) => Value::String(u.to_string()),
            CqlValue::Timeuuid(u) => Value::String(format!("{}", u)),
            CqlValue::Timestamp(dt) => {
                let millis = dt.0;
                let secs = millis / 1000;
                let nanos = ((millis % 1000) * 1_000_000) as u32;
                let dt = Utc.timestamp_opt(secs, nanos).single();
                match dt {
                    Some(dt) => Value::String(dt.format("%Y-%m-%d %H:%M:%S%.3f+00:00").to_string()),
                    None => serde_json::json!(millis),
                }
            }
            CqlValue::Date(d) => {
                let days = d.0 as i64;
                let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
                let date = epoch + chrono::Duration::days(days);
                Value::String(date.format("%Y-%m-%d").to_string())
            }
            CqlValue::Time(t) => {
                let nanos = t.0;
                let total_secs = nanos / 1_000_000_000;
                let hours = total_secs / 3600;
                let minutes = (total_secs % 3600) / 60;
                let seconds = total_secs % 60;
                let subsec_nanos = (nanos % 1_000_000_000) as u32;
                let time = NaiveTime::from_hms_nano_opt(
                    hours as u32,
                    minutes as u32,
                    seconds as u32,
                    subsec_nanos,
                );
                match time {
                    Some(t) => Value::String(t.format("%H:%M:%S%.9f").to_string()),
                    None => serde_json::json!(nanos),
                }
            }
            CqlValue::Blob(b) => Value::String(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                b,
            )),
            CqlValue::Counter(c) => serde_json::json!(c.0),
            CqlValue::Inet(addr) => Value::String(addr.to_string()),
            CqlValue::List(items) | CqlValue::Set(items) => {
                Value::Array(items.iter().map(|v| cql_value_to_json(Some(v))).collect())
            }
            CqlValue::Map(pairs) => {
                let map: serde_json::Map<String, Value> = pairs
                    .iter()
                    .map(|(k, v)| {
                        let key = cql_value_to_json(Some(k));
                        let key_str = match key {
                            Value::String(s) => s,
                            other => other.to_string(),
                        };
                        (key_str, cql_value_to_json(Some(v)))
                    })
                    .collect();
                Value::Object(map)
            }
            CqlValue::Tuple(items) => {
                Value::Array(items.iter().map(|v| cql_value_to_json(v.as_ref())).collect())
            }
            CqlValue::UserDefinedType { fields, .. } => {
                let map: serde_json::Map<String, Value> = fields
                    .iter()
                    .map(|(k, v)| (k.clone(), cql_value_to_json(v.as_ref())))
                    .collect();
                Value::Object(map)
            }
            CqlValue::Decimal(d) => {
                let (bytes, scale) = d.as_signed_be_bytes_slice_and_exponent();
                let unscaled = bytes_to_signed_bigint_string(bytes);
                if scale == 0 {
                    Value::String(unscaled)
                } else {
                    // Insert decimal point
                    let negative = unscaled.starts_with('-');
                    let digits = if negative { &unscaled[1..] } else { &unscaled };
                    let scale = scale as usize;
                    if scale >= digits.len() {
                        let zeros = "0".repeat(scale - digits.len());
                        Value::String(format!("{}0.{}{}", if negative { "-" } else { "" }, zeros, digits))
                    } else {
                        let (int_part, frac_part) = digits.split_at(digits.len() - scale);
                        Value::String(format!("{}{}.{}", if negative { "-" } else { "" }, int_part, frac_part))
                    }
                }
            }
            CqlValue::Varint(v) => {
                Value::String(bytes_to_signed_bigint_string(v.as_signed_bytes_be_slice()))
            }
            CqlValue::Duration(d) => serde_json::json!({
                "months": d.months,
                "days": d.days,
                "nanoseconds": d.nanoseconds,
            }),
            CqlValue::Vector(items) => {
                Value::Array(items.iter().map(|v| cql_value_to_json(Some(v))).collect())
            }
            _ => Value::String(format!("{:?}", cql)),
        },
    }
}

impl CassandraDriver {
    pub async fn connect(form: &ConnectionForm) -> Result<Self, String> {
        let host = form
            .host
            .clone()
            .filter(|h| !h.trim().is_empty())
            .ok_or_else(|| "[VALIDATION_ERROR] host cannot be empty".to_string())?;
        let port = form.port.unwrap_or(DEFAULT_CASSANDRA_PORT);

        let mut effective_form = form.clone();
        let ssh_tunnel = if let Some(true) = form.ssh_enabled {
            let tunnel = crate::ssh::start_ssh_tunnel(form)?;
            effective_form.host = Some("127.0.0.1".to_string());
            effective_form.port = Some(tunnel.local_port as i64);
            Some(tunnel)
        } else {
            None
        };

        let effective_host = effective_form.host.as_deref().unwrap_or(&host);
        let effective_port = effective_form.port.unwrap_or(port);

        let mut builder =
            SessionBuilder::new().known_node(format!("{}:{}", effective_host, effective_port));

        if let Some(ref username) = effective_form.username {
            if !username.trim().is_empty() {
                let password = effective_form.password.as_deref().unwrap_or("");
                builder = builder.user(username, password);
            }
        }

        let timeout_ms = effective_form
            .connect_timeout_ms
            .filter(|&v| v > 0)
            .unwrap_or(DEFAULT_CONNECT_TIMEOUT_MS);
        builder = builder.connection_timeout(Duration::from_millis(timeout_ms as u64));

        let session = builder.build().await.map_err(|e| conn_failed_error(&e))?;

        let default_keyspace = effective_form
            .database
            .clone()
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_default();

        if !default_keyspace.is_empty() {
            session
                .query_unpaged(format!("USE {}", default_keyspace), &[])
                .await
                .map_err(|e| normalize_cassandra_error(e))?;
        }

        Ok(Self {
            session,
            default_keyspace,
            ssh_tunnel,
        })
    }
}

#[async_trait]
impl DatabaseDriver for CassandraDriver {
    async fn close(&self) {}

    async fn test_connection(&self) -> Result<(), String> {
        self.session
            .query_unpaged("SELECT now() FROM system.local", &[])
            .await
            .map_err(|e| conn_failed_error(&e))?;
        Ok(())
    }

    async fn list_databases(&self) -> Result<Vec<String>, String> {
        let result = self
            .session
            .query_unpaged("SELECT keyspace_name FROM system_schema.keyspaces", &[])
            .await
            .map_err(|e| normalize_cassandra_error(e))?;

        let rows_result = result
            .into_rows_result()
            .map_err(|e| normalize_cassandra_error(e))?;

        let mut keyspaces = Vec::new();
        let mut iter = rows_result
            .rows::<scylla::value::Row>()
            .map_err(|e| normalize_cassandra_error(e))?;

        while let Some(row) = iter.next() {
            let row = row.map_err(|e| normalize_cassandra_error(e))?;
            if let Some(CqlValue::Text(name)) = row.columns.first().and_then(|c| c.as_ref()) {
                keyspaces.push(name.clone());
            }
        }

        keyspaces.sort();
        Ok(keyspaces)
    }

    async fn list_tables(&self, schema: Option<String>) -> Result<Vec<TableInfo>, String> {
        let keyspace = schema
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| self.default_keyspace.clone());

        if keyspace.is_empty() {
            return Err("[VALIDATION_ERROR] keyspace is required".to_string());
        }

        let result = self
            .session
            .query_unpaged(
                "SELECT keyspace_name, table_name FROM system_schema.tables WHERE keyspace_name = ?",
                (&keyspace,),
            )
            .await
            .map_err(|e| normalize_cassandra_error(e))?;

        let rows_result = result
            .into_rows_result()
            .map_err(|e| normalize_cassandra_error(e))?;

        let mut tables = Vec::new();
        let mut iter = rows_result
            .rows::<scylla::value::Row>()
            .map_err(|e| normalize_cassandra_error(e))?;

        while let Some(row) = iter.next() {
            let row = row.map_err(|e| normalize_cassandra_error(e))?;
            let ks = match row.columns.first().and_then(|c| c.as_ref()) {
                Some(CqlValue::Text(s)) => s.clone(),
                _ => continue,
            };
            let name = match row.columns.get(1).and_then(|c| c.as_ref()) {
                Some(CqlValue::Text(s)) => s.clone(),
                _ => continue,
            };
            tables.push(TableInfo {
                schema: ks,
                name,
                r#type: "table".to_string(),
            });
        }

        tables.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(tables)
    }

    async fn list_routines(&self, _schema: Option<String>) -> Result<Vec<RoutineInfo>, String> {
        Ok(vec![])
    }

    async fn get_table_structure(
        &self,
        schema: String,
        table: String,
    ) -> Result<TableStructure, String> {
        let result = self
            .session
            .query_unpaged(
                "SELECT column_name, type, kind FROM system_schema.columns WHERE keyspace_name = ? AND table_name = ?",
                (&schema, &table),
            )
            .await
            .map_err(|e| normalize_cassandra_error(e))?;

        let rows_result = result
            .into_rows_result()
            .map_err(|e| normalize_cassandra_error(e))?;

        let mut columns = Vec::new();
        let mut iter = rows_result
            .rows::<scylla::value::Row>()
            .map_err(|e| normalize_cassandra_error(e))?;

        while let Some(row) = iter.next() {
            let row = row.map_err(|e| normalize_cassandra_error(e))?;
            let column_name = match row.columns.first().and_then(|c| c.as_ref()) {
                Some(CqlValue::Text(s)) => s.clone(),
                _ => continue,
            };
            let type_name = match row.columns.get(1).and_then(|c| c.as_ref()) {
                Some(CqlValue::Text(s)) => s.clone(),
                _ => "unknown".to_string(),
            };
            let kind = match row.columns.get(2).and_then(|c| c.as_ref()) {
                Some(CqlValue::Text(s)) => s.clone(),
                _ => "regular".to_string(),
            };

            let is_primary = kind == "partition_key" || kind == "clustering";

            columns.push(ColumnInfo {
                name: column_name,
                r#type: type_name,
                nullable: !is_primary,
                default_value: None,
                primary_key: is_primary,
                comment: Some(format!("kind: {}", kind)),
                default_constraint_name: None,
            });
        }

        Ok(TableStructure { columns })
    }

    async fn get_table_metadata(
        &self,
        schema: String,
        table: String,
    ) -> Result<TableMetadata, String> {
        let columns = self.get_table_structure(schema.clone(), table.clone()).await?;

        let indexes = self.get_table_indexes(&schema, &table).await?;

        let table_extra = self.get_table_extra(&schema, &table).await?;

        Ok(TableMetadata {
            columns: columns.columns,
            indexes,
            foreign_keys: vec![],
            clickhouse_extra: None,
            cassandra_extra: Some(table_extra),
            special_type_summaries: vec![],
        })
    }

    async fn get_table_ddl(&self, schema: String, table: String) -> Result<String, String> {
        let columns_result = self
            .session
            .query_unpaged(
                "SELECT column_name, type, kind, position FROM system_schema.columns WHERE keyspace_name = ? AND table_name = ?",
                (&schema, &table),
            )
            .await
            .map_err(|e| normalize_cassandra_error(e))?;

        let columns_rows = columns_result
            .into_rows_result()
            .map_err(|e| normalize_cassandra_error(e))?;

        let mut partition_keys: Vec<(i32, String)> = Vec::new();
        let mut clustering_columns: Vec<(i32, String)> = Vec::new();
        let mut all_columns: Vec<String> = Vec::new();

        let mut iter = columns_rows
            .rows::<scylla::value::Row>()
            .map_err(|e| normalize_cassandra_error(e))?;

        while let Some(row) = iter.next() {
            let row = row.map_err(|e| normalize_cassandra_error(e))?;
            let column_name = match row.columns.first().and_then(|c| c.as_ref()) {
                Some(CqlValue::Text(s)) => s.clone(),
                _ => continue,
            };
            let type_name = match row.columns.get(1).and_then(|c| c.as_ref()) {
                Some(CqlValue::Text(s)) => s.clone(),
                _ => "text".to_string(),
            };
            let kind = match row.columns.get(2).and_then(|c| c.as_ref()) {
                Some(CqlValue::Text(s)) => s.clone(),
                _ => "regular".to_string(),
            };
            let position = match row.columns.get(3).and_then(|c| c.as_ref()) {
                Some(CqlValue::Int(n)) => *n,
                _ => 0,
            };

            all_columns.push(format!("    {} {}", column_name, type_name));

            match kind.as_str() {
                "partition_key" => partition_keys.push((position, column_name)),
                "clustering" => clustering_columns.push((position, column_name)),
                _ => {}
            }
        }

        partition_keys.sort_by_key(|k| k.0);
        clustering_columns.sort_by_key(|k| k.0);

        let pk_cols: Vec<String> = partition_keys.iter().map(|(_, name)| name.clone()).collect();

        let mut ddl = format!("CREATE TABLE {}.{} (\n", schema, table);

        for (i, col) in all_columns.iter().enumerate() {
            let comma = if i < all_columns.len() - 1 { "," } else { "" };
            ddl.push_str(&format!("{}{}\n", col, comma));
        }

        if clustering_columns.is_empty() && partition_keys.len() == 1 {
            ddl.push_str(&format!("    PRIMARY KEY ({})\n", pk_cols[0]));
        } else {
            let clustering_names: Vec<String> =
                clustering_columns.iter().map(|(_, name)| name.clone()).collect();
            if clustering_names.is_empty() {
                ddl.push_str(&format!("    PRIMARY KEY (({}))\n", pk_cols.join(", ")));
            } else {
                ddl.push_str(&format!(
                    "    PRIMARY KEY (({}), {})\n",
                    pk_cols.join(", "),
                    clustering_names.join(", ")
                ));
            }
        }

        ddl.push_str(");\n");

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
        _filter: Option<String>,
        order_by: Option<String>,
    ) -> Result<TableDataResponse, String> {
        let start = Instant::now();
        let safe_page = page.max(1);
        let safe_limit = limit.clamp(1, 10_000);

        let query = if let Some(ref col) = sort_column {
            let direction = match sort_direction.as_deref() {
                Some("desc") => "DESC",
                _ => "ASC",
            };
            format!(
                "SELECT * FROM {}.{} ORDER BY {} {} LIMIT {}",
                schema, table, col, direction, safe_limit
            )
        } else if let Some(ref ob) = order_by {
            format!(
                "SELECT * FROM {}.{} ORDER BY {} LIMIT {}",
                schema, table, ob, safe_limit
            )
        } else {
            format!(
                "SELECT * FROM {}.{} LIMIT {} ALLOW FILTERING",
                schema, table, safe_limit
            )
        };

        let result = self
            .session
            .query_unpaged(query, &[])
            .await
            .map_err(|e| normalize_cassandra_error(e))?;

        let rows_result = result
            .into_rows_result()
            .map_err(|e| normalize_cassandra_error(e))?;

        let column_specs: Vec<(String, String)> = rows_result
            .column_specs()
            .iter()
            .map(|spec| (spec.name().to_string(), column_type_to_string(spec.typ())))
            .collect();

        let mut data = Vec::new();
        let mut iter = rows_result
            .rows::<scylla::value::Row>()
            .map_err(|e| normalize_cassandra_error(e))?;

        while let Some(row) = iter.next() {
            let row = row.map_err(|e| normalize_cassandra_error(e))?;
            let mut obj = serde_json::Map::new();
            for (i, (col_name, _)) in column_specs.iter().enumerate() {
                let value = cql_value_to_json(row.columns.get(i).and_then(|c| c.as_ref()));
                obj.insert(col_name.clone(), value);
            }
            data.push(Value::Object(obj));
        }

        let row_count = data.len() as i64;
        let _columns: Vec<QueryColumn> = column_specs
            .into_iter()
            .map(|(name, r#type)| QueryColumn { name, r#type })
            .collect();

        let duration = start.elapsed();
        Ok(TableDataResponse {
            data,
            total: row_count,
            page: safe_page,
            limit: safe_limit,
            execution_time_ms: duration.as_millis() as i64,
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
        let start = Instant::now();
        let trimmed = sql.trim();

        if trimmed.is_empty() {
            return Err("[QUERY_ERROR] Empty query".to_string());
        }

        let result = self
            .session
            .query_unpaged(trimmed, &[])
            .await
            .map_err(|e| format!("[QUERY_ERROR] {}", e))?;

        let duration = start.elapsed();

        let rows_result = match result.into_rows_result() {
            Ok(r) => r,
            Err(_) => {
                // DDL/DML statements that don't return rows
                return Ok(QueryResult {
                    data: vec![],
                    row_count: 0,
                    columns: vec![],
                    time_taken_ms: duration.as_millis() as i64,
                    success: true,
                    error: None,
                    result_sets: None,
                });
            }
        };

        let column_specs: Vec<(String, String)> = rows_result
            .column_specs()
            .iter()
            .map(|spec| (spec.name().to_string(), column_type_to_string(spec.typ())))
            .collect();

        let columns: Vec<QueryColumn> = column_specs
            .iter()
            .map(|(name, r#type)| QueryColumn {
                name: name.clone(),
                r#type: r#type.clone(),
            })
            .collect();

        let mut data = Vec::new();
        let mut iter = rows_result
            .rows::<scylla::value::Row>()
            .map_err(|e| format!("[QUERY_ERROR] {}", e))?;

        while let Some(row) = iter.next() {
            let row = row.map_err(|e| format!("[QUERY_ERROR] {}", e))?;
            let mut obj = serde_json::Map::new();
            for (i, (col_name, _)) in column_specs.iter().enumerate() {
                let value = cql_value_to_json(row.columns.get(i).and_then(|c| c.as_ref()));
                obj.insert(col_name.clone(), value);
            }
            data.push(Value::Object(obj));
        }

        let row_count = data.len() as i64;

        Ok(QueryResult {
            data,
            row_count,
            columns,
            time_taken_ms: duration.as_millis() as i64,
            success: true,
            error: None,
            result_sets: None,
        })
    }

    async fn get_schema_overview(&self, schema: Option<String>) -> Result<SchemaOverview, String> {
        let keyspace = schema
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| self.default_keyspace.clone());

        if keyspace.is_empty() {
            return Err("[VALIDATION_ERROR] keyspace is required".to_string());
        }

        let tables = self.list_tables(Some(keyspace.clone())).await?;

        let mut table_schemas = Vec::new();
        for table in tables {
            let structure = self
                .get_table_structure(keyspace.clone(), table.name.clone())
                .await?;
            table_schemas.push(TableSchema {
                schema: keyspace.clone(),
                name: table.name,
                columns: structure
                    .columns
                    .into_iter()
                    .map(|c| ColumnSchema {
                        name: c.name,
                        r#type: c.r#type,
                    })
                    .collect(),
            });
        }

        Ok(SchemaOverview {
            tables: table_schemas,
        })
    }
}

impl CassandraDriver {
    async fn get_table_indexes(
        &self,
        keyspace: &str,
        table: &str,
    ) -> Result<Vec<IndexInfo>, String> {
        let result = self
            .session
            .query_unpaged(
                "SELECT index_name, kind, options FROM system_schema.indexes WHERE keyspace_name = ? AND table_name = ?",
                (keyspace, table),
            )
            .await
            .map_err(|e| normalize_cassandra_error(e))?;

        let rows_result = result
            .into_rows_result()
            .map_err(|e| normalize_cassandra_error(e))?;

        let mut indexes = Vec::new();
        let mut iter = rows_result
            .rows::<scylla::value::Row>()
            .map_err(|e| normalize_cassandra_error(e))?;

        while let Some(row) = iter.next() {
            let row = row.map_err(|e| normalize_cassandra_error(e))?;
            let index_name = match row.columns.first().and_then(|c| c.as_ref()) {
                Some(CqlValue::Text(s)) => s.clone(),
                _ => continue,
            };
            let kind = match row.columns.get(1).and_then(|c| c.as_ref()) {
                Some(CqlValue::Text(s)) => s.clone(),
                _ => "unknown".to_string(),
            };
            // Extract target column from options map (key: "target")
            let index_columns = match row.columns.get(2).and_then(|c| c.as_ref()) {
                Some(CqlValue::Map(pairs)) => {
                    pairs.iter()
                        .find(|(k, _)| matches!(k, CqlValue::Text(s) if s == "target"))
                        .and_then(|(_, v)| match v {
                            CqlValue::Text(target) => Some(vec![target.clone()]),
                            _ => None,
                        })
                        .unwrap_or_default()
                }
                _ => vec![],
            };

            indexes.push(IndexInfo {
                name: index_name,
                unique: false,
                index_type: Some(kind),
                columns: index_columns,
            });
        }

        Ok(indexes)
    }

    async fn get_table_extra(
        &self,
        keyspace: &str,
        table: &str,
    ) -> Result<CassandraTableExtra, String> {
        // Get partition key and clustering columns
        let columns_result = self
            .session
            .query_unpaged(
                "SELECT column_name, kind, position FROM system_schema.columns WHERE keyspace_name = ? AND table_name = ?",
                (keyspace, table),
            )
            .await
            .map_err(|e| normalize_cassandra_error(e))?;

        let columns_rows = columns_result
            .into_rows_result()
            .map_err(|e| normalize_cassandra_error(e))?;

        let mut partition_key: Vec<(i32, String)> = Vec::new();
        let mut clustering_columns: Vec<(i32, String)> = Vec::new();

        let mut col_iter = columns_rows
            .rows::<scylla::value::Row>()
            .map_err(|e| normalize_cassandra_error(e))?;

        while let Some(row) = col_iter.next() {
            let row = row.map_err(|e| normalize_cassandra_error(e))?;
            let column_name = match row.columns.first().and_then(|c| c.as_ref()) {
                Some(CqlValue::Text(s)) => s.clone(),
                _ => continue,
            };
            let kind = match row.columns.get(1).and_then(|c| c.as_ref()) {
                Some(CqlValue::Text(s)) => s.clone(),
                _ => continue,
            };
            let position = match row.columns.get(2).and_then(|c| c.as_ref()) {
                Some(CqlValue::Int(n)) => *n,
                _ => 0,
            };

            match kind.as_str() {
                "partition_key" => partition_key.push((position, column_name)),
                "clustering" => clustering_columns.push((position, column_name)),
                _ => {}
            }
        }

        partition_key.sort_by_key(|k| k.0);
        clustering_columns.sort_by_key(|k| k.0);

        let pk_names: Vec<String> = partition_key.iter().map(|(_, n)| n.clone()).collect();
        let ck_names: Vec<String> = clustering_columns.iter().map(|(_, n)| n.clone()).collect();

        // Get table properties
        let props_result = self
            .session
            .query_unpaged(
                "SELECT bloom_filter_fp_chance, gc_grace_seconds, default_time_to_live, compaction, caching FROM system_schema.tables WHERE keyspace_name = ? AND table_name = ?",
                (keyspace, table),
            )
            .await
            .map_err(|e| normalize_cassandra_error(e))?;

        let props_rows = props_result
            .into_rows_result()
            .map_err(|e| normalize_cassandra_error(e))?;

        let mut iter = props_rows
            .rows::<scylla::value::Row>()
            .map_err(|e| normalize_cassandra_error(e))?;

        if let Some(row) = iter.next() {
            let row = row.map_err(|e| normalize_cassandra_error(e))?;

            let bloom_filter_fp_chance = match row.columns.first().and_then(|c| c.as_ref()) {
                Some(CqlValue::Double(d)) => *d,
                _ => 0.01,
            };
            let gc_grace_seconds = match row.columns.get(1).and_then(|c| c.as_ref()) {
                Some(CqlValue::Int(n)) => *n as i64,
                Some(CqlValue::BigInt(n)) => *n,
                _ => 864000,
            };
            let default_time_to_live = match row.columns.get(2).and_then(|c| c.as_ref()) {
                Some(CqlValue::Int(n)) => *n as i64,
                Some(CqlValue::BigInt(n)) => *n,
                _ => 0,
            };
            let compaction_strategy = match row.columns.get(3).and_then(|c| c.as_ref()) {
                Some(CqlValue::Map(pairs)) => {
                    pairs.iter()
                        .find(|(k, _)| matches!(k, CqlValue::Text(s) if s == "class"))
                        .and_then(|(_, v)| match v {
                            CqlValue::Text(s) => {
                                Some(s.rsplit('.').next().unwrap_or(s).to_string())
                            }
                            _ => None,
                        })
                        .unwrap_or_else(|| "unknown".to_string())
                }
                _ => "unknown".to_string(),
            };
            let caching = match row.columns.get(4).and_then(|c| c.as_ref()) {
                Some(v) => cql_value_to_json(Some(v)),
                None => serde_json::Value::Null,
            };

            Ok(CassandraTableExtra {
                partition_key: pk_names,
                clustering_columns: ck_names,
                compaction_strategy,
                bloom_filter_fp_chance,
                caching,
                gc_grace_seconds,
                default_time_to_live,
            })
        } else {
            Err(format!(
                "[NOT_FOUND] Table '{}.{}' not found",
                keyspace, table
            ))
        }
    }
}
