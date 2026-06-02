use crate::sync::provider::SyncProvider;
use async_trait::async_trait;
use reqwest::Client;

pub struct WebdavProvider {
    server_url: String,
    username: String,
    password: String,
    client: Client,
}

impl WebdavProvider {
    pub fn new(server_url: String, username: String, password: String) -> Self {
        let server_url = if server_url.ends_with('/') {
            server_url
        } else {
            format!("{}/", server_url)
        };
        Self {
            server_url,
            username,
            password,
            client: Client::new(),
        }
    }

    fn object_url(&self, key: &str) -> String {
        format!("{}{}", self.server_url, key)
    }
}

#[async_trait]
impl SyncProvider for WebdavProvider {
    async fn test_connection(&self) -> Result<(), String> {
        let url = self.object_url("");
        let resp = self
            .client
            .request(
                reqwest::Method::from_bytes(b"PROPFIND").unwrap(),
                &url,
            )
            .basic_auth(&self.username, Some(&self.password))
            .header("Depth", "0")
            .send()
            .await
            .map_err(|e| format!("[SYNC_CONNECTION_ERROR] {e}"))?;

        let status = resp.status();
        if status.is_success() || status.as_u16() == 207 {
            Ok(())
        } else {
            Err(format!(
                "[SYNC_CONNECTION_ERROR] WebDAV returned {}",
                status
            ))
        }
    }

    async fn put_object(&self, key: &str, data: &[u8]) -> Result<(), String> {
        let url = self.object_url(key);
        let resp = self
            .client
            .put(&url)
            .basic_auth(&self.username, Some(&self.password))
            .body(data.to_vec())
            .send()
            .await
            .map_err(|e| format!("[SYNC_CONNECTION_ERROR] {e}"))?;

        let status = resp.status();
        if status.is_success() || status.as_u16() == 201 || status.as_u16() == 204 {
            Ok(())
        } else {
            let body = resp.text().await.unwrap_or_default();
            Err(format!(
                "[SYNC_CONNECTION_ERROR] WebDAV PUT failed {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            ))
        }
    }

    async fn get_object(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let url = self.object_url(key);
        let resp = self
            .client
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .map_err(|e| format!("[SYNC_CONNECTION_ERROR] {e}"))?;

        let status = resp.status();
        if status.as_u16() == 404 {
            return Ok(None);
        }
        if status.is_success() {
            let bytes = resp
                .bytes()
                .await
                .map_err(|e| format!("[SYNC_CONNECTION_ERROR] Read body: {e}"))?;
            Ok(Some(bytes.to_vec()))
        } else {
            Err(format!(
                "[SYNC_CONNECTION_ERROR] WebDAV GET failed {}",
                status
            ))
        }
    }

    async fn delete_object(&self, key: &str) -> Result<(), String> {
        let url = self.object_url(key);
        let resp = self
            .client
            .delete(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .map_err(|e| format!("[SYNC_CONNECTION_ERROR] {e}"))?;

        let status = resp.status();
        if status.is_success() || status.as_u16() == 204 || status.as_u16() == 404 {
            Ok(())
        } else {
            let body = resp.text().await.unwrap_or_default();
            Err(format!(
                "[SYNC_CONNECTION_ERROR] WebDAV DELETE failed {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            ))
        }
    }
}
