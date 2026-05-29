use self::cassandra::CassandraDriver;
use self::clickhouse::ClickHouseDriver;
use self::db2::Db2Driver;
use self::duckdb::DuckdbDriver;
use self::mongodb::MongoDBDriver;
use self::mssql::MssqlDriver;
use self::mysql::MysqlDriver;
use self::oracle::OracleDriver;
use self::postgres::PostgresDriver;
use self::sqlite::SqliteDriver;
use crate::models::{
    ConnectionForm, EventInfo, QueryResult, RoutineInfo, SchemaForeignKey, SchemaOverview,
    SequenceInfo, TableDataResponse, TableInfo, TableMetadata, TableStructure, TypeInfo,
};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};

pub mod cassandra;
pub mod clickhouse;
pub mod db2;
pub mod duckdb;
pub mod mongodb;
pub mod mssql;
pub mod mysql;
pub mod oracle;
pub mod postgres;
pub mod sqlite;

pub fn is_mysql_family_driver(driver: &str) -> bool {
    matches!(driver, "mysql" | "mariadb" | "tidb" | "starrocks" | "doris")
}

// --- Chrono formatting helpers -------------------------------------------------
// Centralised formatting for temporal values so that every driver emits the same
// human-friendly representation.  The `%.f` specifier outputs fractional seconds
// only when they are non-zero (e.g. `15` vs `15.123456`).

/// Format a `NaiveDateTime` as `YYYY-MM-DD HH:MM:SS[.f]`.
pub(crate) fn format_naive_datetime(dt: &NaiveDateTime) -> String {
    dt.format("%Y-%m-%d %H:%M:%S%.f").to_string()
}

/// Format a `NaiveDate` as `YYYY-MM-DD`.
pub(crate) fn format_naive_date(d: &NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

/// Format a `NaiveTime` as `HH:MM:SS[.f]`.
pub(crate) fn format_naive_time(t: &NaiveTime) -> String {
    t.format("%H:%M:%S%.f").to_string()
}

/// Format a `DateTime<Utc>` as `YYYY-MM-DD HH:MM:SS[.f]+HH:MM`.
pub(crate) fn format_datetime_utc(dt: &DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S%.f%:z").to_string()
}

/// Build a `[CONN_FAILED]` error message with a context-aware hint derived from the
/// underlying error text, so users are not misled by a generic credential warning
/// when the actual problem is TLS incompatibility, a network issue, etc.
pub(crate) fn conn_failed_error(e: &dyn std::fmt::Display) -> String {
    let raw = e.to_string();
    let lower = raw.to_ascii_lowercase();

    let hint = if lower.contains("dpi-1047")
        || lower.contains("cannot locate a 64-bit oracle client")
    {
        "hint: Oracle Instant Client is not installed — download it from \
         https://www.oracle.com/database/technologies/instant-client/downloads.html \
         and add the directory containing libclntsh to your library path \
         (macOS: DYLD_LIBRARY_PATH; Linux: LD_LIBRARY_PATH)"
    } else if lower.contains("handshake")
        || lower.contains("fatal alert")
        || lower.contains("tls")
        || lower.contains("ssl")
        || lower.contains("certificate")
    {
        "hint: TLS/SSL handshake failed — the server may use a TLS version or cipher suite \
         incompatible with the client (TLS 1.2+ required); try disabling SSL in the connection settings"
    } else if lower.contains("access denied")
        || lower.contains("authentication")
        || lower.contains("password")
        || lower.contains("login failed")
        || lower.contains("invalid password")
        || lower.contains("1045")
    {
        "hint: authentication failed — verify the username/password are correct; \
         if they contain special characters they must be URL-encoded"
    } else if lower.contains("connection refused")
        || lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("broken pipe")
        || lower.contains("network unreachable")
    {
        "hint: could not reach the server — check host, port, firewall rules, and SSH tunnel settings"
    } else if lower.contains("name resolution")
        || lower.contains("no such host")
        || lower.contains("failed to lookup")
        || lower.contains("dns")
    {
        "hint: hostname could not be resolved — check that the host address is correct"
    } else {
        "hint: check host, port, credentials, and SSL settings"
    };

    format!("[CONN_FAILED] {raw} ({hint})")
}

pub(crate) fn strip_trailing_statement_terminator(sql: &str) -> &str {
    let mut out = sql.trim_end();
    while let Some(stripped) = out.strip_suffix(';') {
        out = stripped.trim_end();
    }
    out
}

// SQL statement splitting utilities

pub(crate) fn skip_single_quote(bytes: &[u8], mut i: usize) -> usize {
    i += 1;
    while i < bytes.len() {
        if bytes[i] == b'\'' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    i
}

pub(crate) fn skip_double_quote(bytes: &[u8], mut i: usize) -> usize {
    i += 1;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'"' {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    i
}

pub(crate) fn skip_backtick_quote(bytes: &[u8], mut i: usize) -> usize {
    i += 1;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'`' {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    i
}

pub(crate) fn parse_dollar_quote_tag(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'$') {
        return None;
    }
    let mut i = start + 1;
    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
    }
    if bytes.get(i) == Some(&b'$') {
        Some(i)
    } else {
        None
    }
}

pub(crate) fn skip_dollar_quote(bytes: &[u8], start: usize) -> usize {
    let Some(tag_end) = parse_dollar_quote_tag(bytes, start) else {
        return start + 1;
    };
    let tag = &bytes[start..=tag_end];
    let tag_len = tag.len();
    let mut i = tag_end + 1;

    while i + tag_len <= bytes.len() {
        if &bytes[i..i + tag_len] == tag {
            return i + tag_len;
        }
        i += 1;
    }

    bytes.len()
}

pub(crate) fn skip_line_comment(bytes: &[u8], mut i: usize) -> usize {
    i += 2;
    while i < bytes.len() && bytes[i] != b'\n' {
        i += 1;
    }
    i
}

pub(crate) fn skip_block_comment(bytes: &[u8], mut i: usize) -> usize {
    i += 2;
    while i + 1 < bytes.len() {
        if bytes[i] == b'*' && bytes[i + 1] == b'/' {
            return i + 2;
        }
        i += 1;
    }
    i
}

pub(crate) fn skip_ignorable_sql_prefix(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            i = skip_line_comment(bytes, i);
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i = skip_block_comment(bytes, i);
            continue;
        }
        break;
    }
    i
}

pub(crate) fn split_sql_statements(sql: &str) -> Vec<String> {
    let bytes = sql.as_bytes();
    let mut statements = Vec::new();
    let mut i = 0;
    let mut depth = 0_i32;
    let mut start = skip_ignorable_sql_prefix(bytes, 0);

    while i < bytes.len() {
        let b = bytes[i];
        if i + 1 < bytes.len() && b == b'-' && bytes[i + 1] == b'-' {
            i = skip_line_comment(bytes, i);
            continue;
        }
        if i + 1 < bytes.len() && b == b'/' && bytes[i + 1] == b'*' {
            i = skip_block_comment(bytes, i);
            continue;
        }
        if b == b'\'' {
            i = skip_single_quote(bytes, i);
            continue;
        }
        if b == b'"' {
            i = skip_double_quote(bytes, i);
            continue;
        }
        if b == b'`' {
            i = skip_backtick_quote(bytes, i);
            continue;
        }
        if b == b'$' {
            let next = skip_dollar_quote(bytes, i);
            if next != i + 1 {
                i = next;
                continue;
            }
        }
        if b == b'(' {
            depth += 1;
            i += 1;
            continue;
        }
        if b == b')' {
            depth = (depth - 1).max(0);
            i += 1;
            continue;
        }
        if b == b';' && depth == 0 {
            let stmt = sql[start..i].trim();
            if !stmt.is_empty() {
                statements.push(stmt.to_string());
            }
            start = skip_ignorable_sql_prefix(bytes, i + 1);
        }
        i += 1;
    }

    let tail = sql[start..].trim();
    if !tail.is_empty() {
        statements.push(tail.to_string());
    }

    statements
}

pub(crate) fn first_sql_keyword(sql: &str) -> Option<String> {
    let bytes = sql.as_bytes();
    let start = skip_ignorable_sql_prefix(bytes, 0);
    if start >= bytes.len() {
        return None;
    }
    let mut end = start;
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        end += 1;
    }
    if end == start {
        return None;
    }
    Some(sql[start..end].to_ascii_uppercase())
}

#[async_trait]
pub trait DatabaseDriver: Send + Sync {
    async fn test_connection(&self) -> Result<(), String>;
    async fn list_databases(&self) -> Result<Vec<String>, String>;
    async fn list_tables(&self, schema: Option<String>) -> Result<Vec<TableInfo>, String>;
    async fn list_routines(&self, schema: Option<String>) -> Result<Vec<RoutineInfo>, String> {
        let _ = schema;
        Ok(vec![])
    }
    async fn list_events(&self, _schema: Option<String>) -> Result<Vec<EventInfo>, String> {
        Ok(vec![])
    }
    async fn list_sequences(&self, _schema: Option<String>) -> Result<Vec<SequenceInfo>, String> {
        Ok(vec![])
    }
    async fn list_types(&self, _schema: Option<String>) -> Result<Vec<TypeInfo>, String> {
        Ok(vec![])
    }
    async fn get_routine_ddl(
        &self,
        schema: String,
        name: String,
        routine_type: String,
    ) -> Result<String, String> {
        let _ = (schema, name, routine_type);
        Err("[UNSUPPORTED] Routines are not supported for this driver".to_string())
    }
    async fn get_table_structure(
        &self,
        schema: String,
        table: String,
    ) -> Result<TableStructure, String>;
    async fn get_table_metadata(
        &self,
        schema: String,
        table: String,
    ) -> Result<TableMetadata, String>;
    async fn get_table_ddl(&self, schema: String, table: String) -> Result<String, String>;
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
    ) -> Result<TableDataResponse, String>;
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
    ) -> Result<TableDataResponse, String>;
    async fn execute_query(&self, sql: String) -> Result<QueryResult, String>;
    async fn execute_query_with_id(
        &self,
        sql: String,
        query_id: Option<&str>,
    ) -> Result<QueryResult, String> {
        let _ = query_id;
        self.execute_query(sql).await
    }
    async fn get_schema_overview(&self, schema: Option<String>) -> Result<SchemaOverview, String>;
    async fn get_schema_foreign_keys(
        &self,
        _database: Option<&str>,
    ) -> Result<Vec<SchemaForeignKey>, String> {
        Ok(vec![])
    }
    async fn close(&self);
}

/// Normalize macOS smart quotes (U+2018/U+2019/U+201C/U+201D) to ASCII equivalents.
/// WKWebView on macOS inherits the system "Smart Quotes" setting and may
/// automatically replace straight quotes typed by the user.
pub fn normalize_quotes(s: &str) -> String {
    s.replace('\u{2018}', "'")
        .replace('\u{2019}', "'")
        .replace('\u{201C}', "\"")
        .replace('\u{201D}', "\"")
}

pub async fn connect(form: &ConnectionForm) -> Result<Box<dyn DatabaseDriver>, String> {
    match form.driver.as_str() {
        "postgres" => {
            let driver = PostgresDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        driver if is_mysql_family_driver(driver) => {
            let driver = MysqlDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "sqlite" => {
            let driver = SqliteDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "duckdb" => {
            let driver = DuckdbDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "clickhouse" => {
            let driver = ClickHouseDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "mssql" => {
            let driver = MssqlDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "oracle" => {
            let driver = OracleDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "db2" => {
            let driver = Db2Driver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "mongodb" => {
            let driver = MongoDBDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "cassandra" => {
            let driver = CassandraDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        _ => Err(format!(
            "[UNSUPPORTED] Driver {} not supported",
            form.driver
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        conn_failed_error, first_sql_keyword, is_mysql_family_driver, split_sql_statements,
        strip_trailing_statement_terminator,
    };

    #[test]
    fn conn_failed_error_oracle_client_hint() {
        let msg = conn_failed_error(
            &"DPI-1047: Cannot locate a 64-bit Oracle Client library: \"dlopen(libclntsh.dylib, 0x0001): tried: '/usr/local/lib/libclntsh.dylib' (no such file)\"",
        );
        assert!(msg.starts_with("[CONN_FAILED]"));
        assert!(msg.contains("Oracle Instant Client is not installed"));
        assert!(msg.contains("DYLD_LIBRARY_PATH"));
        assert!(!msg.contains("TLS/SSL handshake failed"));
    }

    #[test]
    fn conn_failed_error_tls_hint() {
        let msg = conn_failed_error(
            &"error communicating with database: received fatal alert: HandshakeFailure",
        );
        assert!(msg.starts_with("[CONN_FAILED]"));
        assert!(msg.contains("TLS/SSL handshake failed"));
        assert!(!msg.contains("username/password"));
    }

    #[test]
    fn conn_failed_error_auth_hint() {
        let msg = conn_failed_error(&"Access denied for user 'root'@'localhost'");
        assert!(msg.contains("authentication failed"));
        assert!(msg.contains("URL-encoded"));
    }

    #[test]
    fn conn_failed_error_connection_refused_hint() {
        let msg = conn_failed_error(&"Connection refused (os error 111)");
        assert!(msg.contains("could not reach the server"));
    }

    #[test]
    fn conn_failed_error_timeout_hint() {
        let msg = conn_failed_error(&"connection timed out");
        assert!(msg.contains("could not reach the server"));
    }

    #[test]
    fn conn_failed_error_dns_hint() {
        let msg = conn_failed_error(&"failed to lookup address information: no such host");
        assert!(msg.contains("hostname could not be resolved"));
    }

    #[test]
    fn conn_failed_error_generic_hint() {
        let msg = conn_failed_error(&"some unknown database error");
        assert!(msg.starts_with("[CONN_FAILED]"));
        assert!(msg.contains("hint:"));
        assert!(!msg.contains("username/password"));
    }

    #[test]
    fn strip_trailing_statement_terminator_removes_single_semicolon() {
        assert_eq!(strip_trailing_statement_terminator("SELECT 1;"), "SELECT 1");
    }

    #[test]
    fn strip_trailing_statement_terminator_removes_multiple_semicolons_and_spaces() {
        assert_eq!(
            strip_trailing_statement_terminator("SELECT 1;;;   "),
            "SELECT 1"
        );
    }

    #[test]
    fn strip_trailing_statement_terminator_keeps_sql_without_semicolon() {
        assert_eq!(strip_trailing_statement_terminator("SELECT 1"), "SELECT 1");
    }

    #[test]
    fn mysql_family_helper_includes_doris_and_starrocks() {
        assert!(is_mysql_family_driver("mysql"));
        assert!(is_mysql_family_driver("starrocks"));
        assert!(is_mysql_family_driver("doris"));
        assert!(!is_mysql_family_driver("postgres"));
    }

    // split_sql_statements tests

    #[test]
    fn split_sql_statements_single_statement() {
        let stmts = split_sql_statements("SELECT 1");
        assert_eq!(stmts, vec!["SELECT 1"]);
    }

    #[test]
    fn split_sql_statements_multiple_statements() {
        let stmts = split_sql_statements("INSERT INTO t VALUES (1); INSERT INTO t VALUES (2);");
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "INSERT INTO t VALUES (1)");
        assert_eq!(stmts[1], "INSERT INTO t VALUES (2)");
    }

    #[test]
    fn split_sql_statements_ignores_semicolon_in_string_literal() {
        let stmts = split_sql_statements("SELECT ';'; SELECT 1");
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "SELECT ';'");
        assert_eq!(stmts[1], "SELECT 1");
    }

    #[test]
    fn split_sql_statements_ignores_semicolon_in_double_quotes() {
        let stmts = split_sql_statements("SELECT \";\"; SELECT 1");
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "SELECT \";\"");
    }

    #[test]
    fn split_sql_statements_ignores_semicolon_in_backtick() {
        let stmts = split_sql_statements("SELECT `col;name` FROM t; SELECT 1");
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "SELECT `col;name` FROM t");
    }

    #[test]
    fn split_sql_statements_ignores_semicolon_in_line_comment() {
        let stmts = split_sql_statements("SELECT 1; -- comment;\nSELECT 2");
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "SELECT 1");
        assert_eq!(stmts[1], "SELECT 2");
    }

    #[test]
    fn split_sql_statements_ignores_semicolon_in_block_comment() {
        let stmts = split_sql_statements("SELECT 1 /* ; */ ; SELECT 2");
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "SELECT 1 /* ; */");
        assert_eq!(stmts[1], "SELECT 2");
    }

    #[test]
    fn split_sql_statements_ignores_semicolon_in_parens() {
        let stmts = split_sql_statements("SELECT (1;2); SELECT 1");
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "SELECT (1;2)");
    }

    #[test]
    fn split_sql_statements_skips_leading_whitespace_and_comments() {
        let stmts = split_sql_statements("  -- comment\n  SELECT 1;  SELECT 2");
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "SELECT 1");
        assert_eq!(stmts[1], "SELECT 2");
    }

    #[test]
    fn split_sql_statements_empty_input() {
        let stmts = split_sql_statements("");
        assert!(stmts.is_empty());
    }

    #[test]
    fn split_sql_statements_only_whitespace_and_comments() {
        let stmts = split_sql_statements("  -- just a comment\n  ");
        assert!(stmts.is_empty());
    }

    #[test]
    fn split_sql_statements_trailing_semicolon() {
        let stmts = split_sql_statements("SELECT 1;");
        assert_eq!(stmts, vec!["SELECT 1"]);
    }

    #[test]
    fn split_sql_statements_mysql_insert_values() {
        let sql =
            "INSERT INTO t (id, name) VALUES (1, 'a'); INSERT INTO t (id, name) VALUES (2, 'b')";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "INSERT INTO t (id, name) VALUES (1, 'a')");
        assert_eq!(stmts[1], "INSERT INTO t (id, name) VALUES (2, 'b')");
    }

    // first_sql_keyword tests

    #[test]
    fn first_sql_keyword_returns_uppercase() {
        assert_eq!(first_sql_keyword("select 1"), Some("SELECT".to_string()));
        assert_eq!(
            first_sql_keyword("INSERT INTO t"),
            Some("INSERT".to_string())
        );
    }

    #[test]
    fn first_sql_keyword_skips_whitespace() {
        assert_eq!(first_sql_keyword("  SELECT 1"), Some("SELECT".to_string()));
    }

    #[test]
    fn first_sql_keyword_skips_line_comment() {
        assert_eq!(
            first_sql_keyword("-- comment\nSELECT 1"),
            Some("SELECT".to_string())
        );
    }

    #[test]
    fn first_sql_keyword_skips_block_comment() {
        assert_eq!(
            first_sql_keyword("/* comment */ SELECT 1"),
            Some("SELECT".to_string())
        );
    }

    #[test]
    fn first_sql_keyword_empty_input() {
        assert_eq!(first_sql_keyword(""), None);
    }

    #[test]
    fn first_sql_keyword_only_comments() {
        assert_eq!(first_sql_keyword("-- just a comment\n"), None);
    }

    // Chrono formatting helper tests

    #[test]
    fn format_naive_datetime_without_fractional_seconds() {
        use chrono::NaiveDateTime;
        let dt = NaiveDateTime::parse_from_str("2026-05-12 06:52:15", "%Y-%m-%d %H:%M:%S").unwrap();
        assert_eq!(super::format_naive_datetime(&dt), "2026-05-12 06:52:15");
    }

    #[test]
    fn format_naive_datetime_with_fractional_seconds() {
        use chrono::NaiveDateTime;
        let dt =
            NaiveDateTime::parse_from_str("2026-05-12 06:52:15.123456", "%Y-%m-%d %H:%M:%S%.f")
                .unwrap();
        assert_eq!(
            super::format_naive_datetime(&dt),
            "2026-05-12 06:52:15.123456"
        );
    }

    #[test]
    fn format_naive_date_basic() {
        use chrono::NaiveDate;
        let d = NaiveDate::from_ymd_opt(2026, 5, 12).unwrap();
        assert_eq!(super::format_naive_date(&d), "2026-05-12");
    }

    #[test]
    fn format_naive_time_without_fractional_seconds() {
        use chrono::NaiveTime;
        let t = NaiveTime::from_hms_opt(6, 52, 15).unwrap();
        assert_eq!(super::format_naive_time(&t), "06:52:15");
    }

    #[test]
    fn format_naive_time_with_fractional_seconds() {
        use chrono::NaiveTime;
        let t = NaiveTime::from_hms_micro_opt(6, 52, 15, 123456).unwrap();
        assert_eq!(super::format_naive_time(&t), "06:52:15.123456");
    }

    #[test]
    fn format_datetime_utc_without_fractional_seconds() {
        use chrono::{TimeZone, Utc};
        let dt = Utc.with_ymd_and_hms(2026, 5, 12, 6, 52, 15).unwrap();
        assert_eq!(super::format_datetime_utc(&dt), "2026-05-12 06:52:15+00:00");
    }

    #[test]
    fn format_datetime_utc_with_fractional_seconds() {
        use chrono::{DateTime, NaiveDate, Utc};
        let dt = DateTime::<Utc>::from_naive_utc_and_offset(
            NaiveDate::from_ymd_opt(2026, 5, 12)
                .unwrap()
                .and_hms_micro_opt(6, 52, 15, 123456)
                .unwrap(),
            Utc,
        );
        assert_eq!(
            super::format_datetime_utc(&dt),
            "2026-05-12 06:52:15.123456+00:00"
        );
    }
}
