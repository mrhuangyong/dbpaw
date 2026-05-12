use crate::models::ConnectionForm;
use mongodb::bson::doc;
use mongodb::options::{ClientOptions, Tls, TlsOptions};
use mongodb::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const DEFAULT_MONGODB_PORT: i64 = 27017;
const DEFAULT_CONNECT_TIMEOUT_MS: i64 = 5000;

#[derive(Clone)]
pub struct MongodbClient {
    client: Client,
    /// Held to keep the SSH tunnel alive for the lifetime of this client.
    #[allow(dead_code)]
    ssh_tunnel: Option<crate::ssh::SshTunnel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MongodbConnectionInfo {
    pub version: Option<String>,
    pub node_count: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MongodbDatabaseInfo {
    pub name: String,
    pub size_on_disk: Option<i64>,
    pub empty: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MongodbCollectionInfo {
    pub name: String,
    pub database: String,
    pub document_count: Option<i64>,
    pub size: Option<i64>,
}

fn trim_to_option(value: Option<&String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .and_then(|v| if v.is_empty() { None } else { Some(v) })
}

fn normalize_mongo_error(e: impl std::fmt::Display) -> String {
    let msg = e.to_string();
    if msg.contains("authentication") || msg.contains("auth") {
        format!("[MONGODB_ERROR] Authentication failed: {}", msg)
    } else if msg.contains("dns") || msg.contains("resolve") || msg.contains("lookup") {
        format!("[MONGODB_ERROR] DNS resolution failed: {}", msg)
    } else if msg.contains("timeout") || msg.contains("timed out") {
        format!("[MONGODB_ERROR] Connection timed out: {}", msg)
    } else if msg.contains("refused") {
        format!("[MONGODB_ERROR] Connection refused: {}", msg)
    } else {
        format!("[MONGODB_ERROR] {}", msg)
    }
}

fn build_connection_uri(form: &ConnectionForm) -> Result<String, String> {
    let host = trim_to_option(form.host.as_ref())
        .ok_or_else(|| "[VALIDATION_ERROR] host cannot be empty".to_string())?;
    let port = form.port.unwrap_or(DEFAULT_MONGODB_PORT);
    if !(1..=65535).contains(&port) {
        return Err("[VALIDATION_ERROR] port must be between 1 and 65535".to_string());
    }

    let username = trim_to_option(form.username.as_ref());
    let password = trim_to_option(form.password.as_ref());
    let database = trim_to_option(form.database.as_ref());
    let auth_source = trim_to_option(form.auth_source.as_ref());

    let mut uri = String::from("mongodb://");

    if let Some(user) = &username {
        uri.push_str(&urlencoding::encode(user));
        if let Some(pass) = &password {
            uri.push(':');
            uri.push_str(&urlencoding::encode(pass));
        }
        uri.push('@');
    }

    uri.push_str(&host);
    uri.push(':');
    uri.push_str(&port.to_string());

    if let Some(db) = &database {
        uri.push('/');
        uri.push_str(db);
    }

    let mut params: Vec<String> = Vec::new();

    if let Some(src) = &auth_source {
        params.push(format!("authSource={}", urlencoding::encode(src)));
    }

    if form.ssl.unwrap_or(false) {
        params.push("ssl=true".to_string());
    }

    if let Some(timeout) = form.connect_timeout_ms {
        params.push(format!("connectTimeoutMS={}", timeout));
    }

    if !params.is_empty() {
        uri.push('?');
        uri.push_str(&params.join("&"));
    }

    Ok(uri)
}

impl MongodbClient {
    pub async fn connect(form: &ConnectionForm) -> Result<Self, String> {
        let timeout_ms = form
            .connect_timeout_ms
            .unwrap_or(DEFAULT_CONNECT_TIMEOUT_MS);
        if timeout_ms <= 0 {
            return Err("[VALIDATION_ERROR] connect timeout must be greater than 0".to_string());
        }

        let mut effective_form = form.clone();
        let ssh_tunnel = if let Some(true) = form.ssh_enabled {
            let tunnel = crate::ssh::start_ssh_tunnel(form)?;
            effective_form.host = Some("127.0.0.1".to_string());
            effective_form.port = Some(tunnel.local_port as i64);
            Some(tunnel)
        } else {
            None
        };

        let uri = build_connection_uri(&effective_form)?;

        let mut options = ClientOptions::parse(&uri)
            .await
            .map_err(|e| normalize_mongo_error(e))?;

        options.connect_timeout = Some(Duration::from_millis(timeout_ms as u64));

        if effective_form.ssl.unwrap_or(false) {
            let ssl_mode = trim_to_option(effective_form.ssl_mode.as_ref());
            if ssl_mode.as_deref() == Some("verify_ca") {
                let ca_cert = trim_to_option(effective_form.ssl_ca_cert.as_ref()).ok_or_else(|| {
                    "[VALIDATION_ERROR] sslCaCert cannot be empty in verify_ca mode".to_string()
                })?;
                let tls_options = TlsOptions::builder()
                    .ca_file_path(std::path::PathBuf::from(ca_cert))
                    .build();
                options.tls = Some(Tls::Enabled(tls_options));
            }
        }

        let client = Client::with_options(options).map_err(|e| normalize_mongo_error(e))?;

        Ok(Self {
            client,
            ssh_tunnel,
        })
    }

    pub async fn test_connection(&self) -> Result<MongodbConnectionInfo, String> {
        let db = self.client.default_database().unwrap_or_else(|| {
            self.client.database("admin")
        });

        let result = db
            .run_command(doc! { "serverStatus": 1 })
            .await
            .map_err(|e| normalize_mongo_error(e))?;

        let version = result
            .get_str("version")
            .ok()
            .map(|s| s.to_string());

        let node_count = result
            .get_document("connections")
            .ok()
            .and_then(|c| c.get_i32("current").ok());

        Ok(MongodbConnectionInfo {
            version,
            node_count,
        })
    }

    pub async fn list_databases(&self) -> Result<Vec<MongodbDatabaseInfo>, String> {
        let databases = self
            .client
            .list_databases()
            .await
            .map_err(|e| normalize_mongo_error(e))?;

        Ok(databases
            .into_iter()
            .map(|db| MongodbDatabaseInfo {
                name: db.name,
                size_on_disk: Some(db.size_on_disk as i64),
                empty: Some(db.empty),
            })
            .collect())
    }

    pub async fn list_collections(
        &self,
        database: &str,
    ) -> Result<Vec<MongodbCollectionInfo>, String> {
        let db = self.client.database(database);
        let mut cursor = db
            .list_collections()
            .await
            .map_err(|e| normalize_mongo_error(e))?;

        let mut result = Vec::new();
        while cursor
            .advance()
            .await
            .map_err(|e| normalize_mongo_error(e))?
        {
            let collection = cursor
                .deserialize_current()
                .map_err(|e| normalize_mongo_error(e))?;

            let name = collection.name.clone();
            let db_name = database.to_string();

            result.push(MongodbCollectionInfo {
                name,
                database: db_name,
                document_count: None,
                size: None,
            });
        }

        Ok(result)
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_form(driver: &str, host: Option<&str>, port: Option<i64>) -> ConnectionForm {
        ConnectionForm {
            driver: driver.to_string(),
            host: host.map(|s| s.to_string()),
            port,
            ..Default::default()
        }
    }

    #[test]
    fn build_uri_basic() {
        let form = make_form("mongodb", Some("localhost"), Some(27017));
        let uri = build_connection_uri(&form).unwrap();
        assert_eq!(uri, "mongodb://localhost:27017");
    }

    #[test]
    fn build_uri_with_auth() {
        let form = ConnectionForm {
            driver: "mongodb".to_string(),
            host: Some("localhost".to_string()),
            port: Some(27017),
            username: Some("admin".to_string()),
            password: Some("pass word".to_string()),
            ..Default::default()
        };
        let uri = build_connection_uri(&form).unwrap();
        assert!(uri.starts_with("mongodb://admin:pass%20word@localhost:27017"));
    }

    #[test]
    fn build_uri_with_database() {
        let form = ConnectionForm {
            driver: "mongodb".to_string(),
            host: Some("localhost".to_string()),
            port: Some(27017),
            database: Some("mydb".to_string()),
            ..Default::default()
        };
        let uri = build_connection_uri(&form).unwrap();
        assert_eq!(uri, "mongodb://localhost:27017/mydb");
    }

    #[test]
    fn build_uri_with_auth_source() {
        let form = ConnectionForm {
            driver: "mongodb".to_string(),
            host: Some("localhost".to_string()),
            port: Some(27017),
            username: Some("admin".to_string()),
            password: Some("pass".to_string()),
            auth_source: Some("admin".to_string()),
            ..Default::default()
        };
        let uri = build_connection_uri(&form).unwrap();
        assert!(uri.contains("authSource=admin"));
    }

    #[test]
    fn build_uri_with_ssl() {
        let form = ConnectionForm {
            driver: "mongodb".to_string(),
            host: Some("localhost".to_string()),
            port: Some(27017),
            ssl: Some(true),
            ..Default::default()
        };
        let uri = build_connection_uri(&form).unwrap();
        assert!(uri.contains("ssl=true"));
    }

    #[test]
    fn build_uri_default_port() {
        let form = make_form("mongodb", Some("localhost"), None);
        let uri = build_connection_uri(&form).unwrap();
        assert!(uri.contains("localhost:27017"));
    }

    #[test]
    fn build_uri_missing_host() {
        let form = make_form("mongodb", None, None);
        assert!(build_connection_uri(&form).is_err());
    }

    #[test]
    fn build_uri_invalid_port() {
        let form = make_form("mongodb", Some("localhost"), Some(99999));
        assert!(build_connection_uri(&form).is_err());
    }

    #[test]
    fn normalize_error_categorization() {
        assert!(normalize_mongo_error("authentication failed").contains("Authentication failed"));
        assert!(normalize_mongo_error("dns resolve error").contains("DNS resolution failed"));
        assert!(normalize_mongo_error("connection timed out").contains("Connection timed out"));
        assert!(normalize_mongo_error("connection refused").contains("Connection refused"));
        assert!(normalize_mongo_error("some other error").starts_with("[MONGODB_ERROR]"));
    }
}
