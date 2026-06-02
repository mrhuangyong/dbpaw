use crate::sync::provider::SyncProvider;
use async_trait::async_trait;
use hmac::{Hmac, Mac};
use reqwest::Client;
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

pub struct S3Provider {
    endpoint: String,
    region: String,
    bucket: String,
    access_key_id: String,
    secret_access_key: String,
    path_prefix: String,
    client: Client,
}

impl S3Provider {
    pub fn new(
        endpoint: String,
        region: String,
        bucket: String,
        access_key_id: String,
        secret_access_key: String,
        path_prefix: String,
    ) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            region: if region.is_empty() {
                "us-east-1".to_string()
            } else {
                region
            },
            bucket,
            access_key_id,
            secret_access_key,
            path_prefix: if path_prefix.is_empty() {
                "dbpaw/".to_string()
            } else {
                path_prefix
            },
            client: Client::new(),
        }
    }

    fn object_url(&self, key: &str) -> String {
        format!(
            "{}/{}/{}{}",
            self.endpoint, self.bucket, self.path_prefix, key
        )
    }

    fn sign_request(
        &self,
        method: &str,
        url: &url::Url,
        headers: &mut Vec<(String, String)>,
        payload_hash: &str,
        date: &str,
        datetime: &str,
    ) {
        let host = url.host_str().unwrap_or("");
        let path = url.path();
        let query = url.query().unwrap_or("");

        headers.push(("host".to_string(), host.to_string()));
        headers.push((
            "x-amz-content-sha256".to_string(),
            payload_hash.to_string(),
        ));
        headers.push(("x-amz-date".to_string(), datetime.to_string()));
        headers.sort_by(|a, b| a.0.cmp(&b.0));

        let signed_headers: String = headers
            .iter()
            .map(|(k, _)| k.as_str())
            .collect::<Vec<_>>()
            .join(";");
        let canonical_headers: String = headers
            .iter()
            .map(|(k, v)| format!("{}:{}", k.to_lowercase(), v.trim()))
            .collect::<Vec<_>>()
            .join("\n");

        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n\n{}\n{}",
            method, path, query, canonical_headers, signed_headers, payload_hash
        );

        let credential_scope = format!("{}/{}/s3/aws4_request", self.region, date);
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            datetime,
            credential_scope,
            hex::encode(sha256(canonical_request.as_bytes()))
        );

        let signing_key = self.derive_signing_key(date);
        let signature = hex::encode(hmac_sha256_bytes(&signing_key, string_to_sign.as_bytes()));

        headers.push((
            "Authorization".to_string(),
            format!(
                "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
                self.access_key_id, credential_scope, signed_headers, signature
            ),
        ));
    }

    fn derive_signing_key(&self, date: &str) -> Vec<u8> {
        let k_date = hmac_sha256_bytes(
            format!("AWS4{}", self.secret_access_key).as_bytes(),
            date.as_bytes(),
        );
        let k_region = hmac_sha256_bytes(&k_date, self.region.as_bytes());
        let k_service = hmac_sha256_bytes(&k_region, b"s3");
        hmac_sha256_bytes(&k_service, b"aws4_request")
    }

    fn now_timestamps() -> (String, String) {
        let now = chrono::Utc::now();
        let date = now.format("%Y%m%d").to_string();
        let datetime = now.format("%Y%m%dT%H%M%SZ").to_string();
        (date, datetime)
    }
}

fn sha256(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

fn hmac_sha256_bytes(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac =
        HmacSha256::new_from_slice(key).expect("HMAC key length is valid");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

#[async_trait]
impl SyncProvider for S3Provider {
    async fn test_connection(&self) -> Result<(), String> {
        let url: url::Url = format!("{}/{}/", self.endpoint, self.bucket)
            .parse()
            .map_err(|e: url::ParseError| {
                format!("[SYNC_CONNECTION_ERROR] Invalid URL: {e}")
            })?;

        let empty_payload_hash = hex::encode(sha256(b""));
        let (date, datetime) = Self::now_timestamps();

        let mut headers = vec![(
            "Content-Type".to_string(),
            "application/octet-stream".to_string(),
        )];
        self.sign_request("GET", &url, &mut headers, &empty_payload_hash, &date, &datetime);

        let mut req = self.client.get(url.as_str());
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("[SYNC_CONNECTION_ERROR] {e}"))?;
        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else {
            let body = resp.text().await.unwrap_or_default();
            Err(format!(
                "[SYNC_CONNECTION_ERROR] S3 returned {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            ))
        }
    }

    async fn put_object(&self, key: &str, data: &[u8]) -> Result<(), String> {
        let url: url::Url = self
            .object_url(key)
            .parse()
            .map_err(|e: url::ParseError| {
                format!("[SYNC_CONNECTION_ERROR] Invalid URL: {e}")
            })?;

        let payload_hash = hex::encode(sha256(data));
        let (date, datetime) = Self::now_timestamps();

        let mut headers = vec![(
            "Content-Type".to_string(),
            "application/octet-stream".to_string(),
        )];
        self.sign_request("PUT", &url, &mut headers, &payload_hash, &date, &datetime);

        let mut req = self.client.put(url.as_str()).body(data.to_vec());
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("[SYNC_CONNECTION_ERROR] {e}"))?;
        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else {
            let body = resp.text().await.unwrap_or_default();
            Err(format!(
                "[SYNC_CONNECTION_ERROR] S3 PUT failed {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            ))
        }
    }

    async fn get_object(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let url: url::Url = self
            .object_url(key)
            .parse()
            .map_err(|e: url::ParseError| {
                format!("[SYNC_CONNECTION_ERROR] Invalid URL: {e}")
            })?;

        let empty_payload_hash = hex::encode(sha256(b""));
        let (date, datetime) = Self::now_timestamps();

        let mut headers = vec![(
            "Content-Type".to_string(),
            "application/octet-stream".to_string(),
        )];
        self.sign_request(
            "GET",
            &url,
            &mut headers,
            &empty_payload_hash,
            &date,
            &datetime,
        );

        let mut req = self.client.get(url.as_str());
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let resp = req
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
            let body = resp.text().await.unwrap_or_default();
            Err(format!(
                "[SYNC_CONNECTION_ERROR] S3 GET failed {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            ))
        }
    }

    async fn delete_object(&self, key: &str) -> Result<(), String> {
        let url: url::Url = self
            .object_url(key)
            .parse()
            .map_err(|e: url::ParseError| {
                format!("[SYNC_CONNECTION_ERROR] Invalid URL: {e}")
            })?;

        let empty_payload_hash = hex::encode(sha256(b""));
        let (date, datetime) = Self::now_timestamps();

        let mut headers = vec![(
            "Content-Type".to_string(),
            "application/octet-stream".to_string(),
        )];
        self.sign_request(
            "DELETE",
            &url,
            &mut headers,
            &empty_payload_hash,
            &date,
            &datetime,
        );

        let mut req = self.client.delete(url.as_str());
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("[SYNC_CONNECTION_ERROR] {e}"))?;
        let status = resp.status();
        if status.is_success() || status.as_u16() == 204 {
            Ok(())
        } else {
            let body = resp.text().await.unwrap_or_default();
            Err(format!(
                "[SYNC_CONNECTION_ERROR] S3 DELETE failed {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            ))
        }
    }
}
