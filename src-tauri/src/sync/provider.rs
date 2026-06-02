use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ProviderType {
    S3,
    WebDAV,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConfig {
    pub provider_type: ProviderType,
    // S3 fields
    pub endpoint: Option<String>,
    pub region: Option<String>,
    pub bucket: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub path_prefix: Option<String>,
    // WebDAV fields
    pub server_url: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub enabled: bool,
    pub provider_type: Option<ProviderType>,
    pub endpoint: Option<String>,
    pub last_sync_at: Option<String>,
    pub last_sync_result: Option<String>,
    pub device_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub action: String,
    pub timestamp: String,
    pub remote_device_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSnapshot {
    pub version: u32,
    pub device_id: String,
    pub timestamp: String,
    pub snapshot_hash: String,
    pub data: SyncSnapshotData,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncSnapshotData {
    pub connections: Vec<serde_json::Value>,
    pub saved_queries: Vec<serde_json::Value>,
    pub ai_providers: Vec<serde_json::Value>,
    pub settings: serde_json::Value,
}

#[async_trait]
pub trait SyncProvider: Send + Sync {
    async fn test_connection(&self) -> Result<(), String>;
    async fn put_object(&self, key: &str, data: &[u8]) -> Result<(), String>;
    async fn get_object(&self, key: &str) -> Result<Option<Vec<u8>>, String>;
    async fn delete_object(&self, key: &str) -> Result<(), String>;
}

/// Build a SyncProvider from a SyncConfig.
pub fn build_provider(config: &SyncConfig) -> Result<Box<dyn SyncProvider>, String> {
    match config.provider_type {
        ProviderType::S3 => {
            let endpoint = config.endpoint.as_deref().unwrap_or("").trim();
            let region = config.region.as_deref().unwrap_or("").trim();
            let bucket = config.bucket.as_deref().unwrap_or("").trim();
            let access_key_id = config.access_key_id.as_deref().unwrap_or("").trim();
            let secret_access_key = config.secret_access_key.as_deref().unwrap_or("").trim();
            let path_prefix = config.path_prefix.as_deref().unwrap_or("dbpaw/").trim();

            if endpoint.is_empty()
                || bucket.is_empty()
                || access_key_id.is_empty()
                || secret_access_key.is_empty()
            {
                return Err(
                    "[SYNC_CONFIG_ERROR] S3 endpoint, bucket, accessKeyId and secretAccessKey are required"
                        .to_string(),
                );
            }

            Ok(Box::new(crate::sync::s3::S3Provider::new(
                endpoint.to_string(),
                region.to_string(),
                bucket.to_string(),
                access_key_id.to_string(),
                secret_access_key.to_string(),
                path_prefix.to_string(),
            )))
        }
        ProviderType::WebDAV => {
            let server_url = config.server_url.as_deref().unwrap_or("").trim();
            let username = config.username.as_deref().unwrap_or("").trim();
            let password = config.password.as_deref().unwrap_or("").trim();

            if server_url.is_empty() || username.is_empty() || password.is_empty() {
                return Err(
                    "[SYNC_CONFIG_ERROR] WebDAV serverUrl, username and password are required"
                        .to_string(),
                );
            }

            Ok(Box::new(crate::sync::webdav::WebdavProvider::new(
                server_url.to_string(),
                username.to_string(),
                password.to_string(),
            )))
        }
    }
}
