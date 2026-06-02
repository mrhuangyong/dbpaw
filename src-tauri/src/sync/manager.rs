use crate::db::local::LocalDb;
use crate::sync::crypto;
use crate::sync::provider::{
    build_provider, ProviderType, SyncConfig, SyncResult, SyncSnapshot, SyncSnapshotData,
    SyncStatus,
};
use std::sync::Arc;
use tokio::sync::Mutex;

const SNAPSHOT_KEY: &str = "sync_snapshot.enc";

pub struct SyncManager {
    local_db: Arc<Mutex<Option<Arc<LocalDb>>>>,
}

impl SyncManager {
    pub fn new(local_db: Arc<Mutex<Option<Arc<LocalDb>>>>) -> Self {
        Self { local_db }
    }

    async fn get_db(&self) -> Result<Arc<LocalDb>, String> {
        let lock = self.local_db.lock().await;
        lock.clone()
            .ok_or_else(|| "[SYNC_CONFIG_ERROR] Local DB not initialized".to_string())
    }

    /// Test connection to the remote provider.
    pub async fn test_connection(&self, config: &SyncConfig) -> Result<(), String> {
        let provider = build_provider(config)?;
        provider.test_connection().await
    }

    /// Get current sync status.
    pub async fn get_status(&self) -> Result<SyncStatus, String> {
        let db = self.get_db().await?;

        let enabled = db
            .get_sync_state("sync_enabled")
            .await?
            .unwrap_or_else(|| "false".to_string());
        let provider_type_str = db.get_sync_state("provider_type").await?;
        let endpoint = db.get_sync_state("endpoint").await?;
        let last_sync_at = db.get_sync_state("last_sync_at").await?;
        let last_sync_result = db.get_sync_state("last_sync_result").await?;
        let device_id = db.get_sync_state("device_id").await?;

        Ok(SyncStatus {
            enabled: enabled == "true",
            provider_type: provider_type_str.and_then(|s| match s.as_str() {
                "S3" => Some(ProviderType::S3),
                "WebDAV" => Some(ProviderType::WebDAV),
                _ => None,
            }),
            endpoint,
            last_sync_at,
            last_sync_result,
            device_id,
        })
    }

    /// Configure and enable sync. Saves config, generates device_id, does first upload.
    pub async fn configure(
        &self,
        config: &SyncConfig,
        sync_password: &str,
    ) -> Result<(), String> {
        let db = self.get_db().await?;

        // Validate connection first
        let provider = build_provider(config)?;
        provider.test_connection().await?;

        // Generate device_id if not exists
        let device_id = match db.get_sync_state("device_id").await? {
            Some(id) => id,
            None => {
                let id = uuid::Uuid::new_v4().to_string();
                db.set_sync_state("device_id", &id).await?;
                id
            }
        };

        // Save config
        let config_json = serde_json::to_string(config)
            .map_err(|e| format!("[SYNC_CONFIG_ERROR] Serialize config: {e}"))?;

        db.set_sync_state("sync_config", &config_json).await?;
        db.set_sync_state(
            "provider_type",
            &format!("{:?}", config.provider_type),
        )
        .await?;

        // Store endpoint for display
        let display_endpoint = match config.provider_type {
            ProviderType::S3 => config.endpoint.clone().unwrap_or_default(),
            ProviderType::WebDAV => config.server_url.clone().unwrap_or_default(),
        };
        db.set_sync_state("endpoint", &display_endpoint).await?;

        // Store sync password encrypted with master key for automatic sync
        let encrypted_pw = db.encrypt_sync_password(sync_password)?;
        db.set_sync_state("sync_password_enc", &encrypted_pw).await?;

        // Export and upload initial snapshot
        let snapshot = self.export_snapshot(&db, &device_id).await?;
        let plaintext = serde_json::to_vec(&snapshot)
            .map_err(|e| format!("[SYNC_CONFIG_ERROR] Serialize snapshot: {e}"))?;
        let encrypted = crypto::encrypt(sync_password, &plaintext)?;
        provider.put_object(SNAPSHOT_KEY, &encrypted).await?;

        // Update state
        db.set_sync_state("sync_enabled", "true").await?;
        db.set_sync_state("last_synced_hash", &snapshot.snapshot_hash)
            .await?;
        db.set_sync_state(
            "last_sync_at",
            &chrono::Utc::now().to_rfc3339(),
        )
        .await?;
        db.set_sync_state("last_sync_result", "success").await?;

        Ok(())
    }

    /// Disable sync (keep config for re-enable).
    pub async fn disable(&self) -> Result<(), String> {
        let db = self.get_db().await?;
        db.set_sync_state("sync_enabled", "false").await?;
        Ok(())
    }

    /// Get saved sync config (for form echo-back).
    pub async fn get_config(&self) -> Result<Option<SyncConfig>, String> {
        let db = self.get_db().await?;
        match db.get_sync_state("sync_config").await? {
            Some(json) => {
                let config: SyncConfig = serde_json::from_str(&json)
                    .map_err(|e| format!("[SYNC_CONFIG_ERROR] Parse config: {e}"))?;
                Ok(Some(config))
            }
            None => Ok(None),
        }
    }

    /// Sync now: pull remote, then push local if changed.
    pub async fn sync_now(&self) -> Result<SyncResult, String> {
        let db = self.get_db().await?;
        let sync_password = self.get_sync_password(&db).await?;
        let config = self.load_config(&db).await?;
        let provider = build_provider(&config)?;

        let local_device_id = self.get_device_id(&db).await?;
        let now = chrono::Utc::now().to_rfc3339();

        // Pull remote
        let remote_result = provider.get_object(SNAPSHOT_KEY).await?;
        if let Some(remote_encrypted) = remote_result {
            let remote_plaintext = crypto::decrypt(&sync_password, &remote_encrypted)?;
            let remote_snapshot: SyncSnapshot = serde_json::from_slice(&remote_plaintext)
                .map_err(|e| format!("[SYNC_MERGE_ERROR] Invalid remote snapshot: {e}"))?;

            // Verify remote hash integrity
            let data_json = serde_json::to_vec(&remote_snapshot.data)
                .map_err(|e| format!("[SYNC_MERGE_ERROR] Serialize: {e}"))?;
            let computed_hash = crypto::snapshot_hash(&data_json);
            if computed_hash != remote_snapshot.snapshot_hash {
                return Err(
                    "[SYNC_CRYPTO_ERROR] Remote snapshot hash mismatch (corrupted data?)"
                        .to_string(),
                );
            }

            // Import remote data if it's from a different device
            if remote_snapshot.device_id != local_device_id {
                self.import_snapshot(&db, &remote_snapshot).await?;
                db.set_sync_state("last_sync_result", "success").await?;
                db.set_sync_state("last_sync_at", &now).await?;

                return Ok(SyncResult {
                    action: "pulled".to_string(),
                    timestamp: now,
                    remote_device_id: Some(remote_snapshot.device_id),
                });
            }
        }

        // No remote or same device — push local
        let snapshot = self.export_snapshot(&db, &local_device_id).await?;
        let plaintext = serde_json::to_vec(&snapshot)
            .map_err(|e| format!("[SYNC_CONFIG_ERROR] Serialize: {e}"))?;
        let encrypted = crypto::encrypt(&sync_password, &plaintext)?;
        provider.put_object(SNAPSHOT_KEY, &encrypted).await?;

        db.set_sync_state("last_synced_hash", &snapshot.snapshot_hash)
            .await?;
        db.set_sync_state("last_sync_result", "success").await?;
        db.set_sync_state("last_sync_at", &now).await?;

        Ok(SyncResult {
            action: "pushed".to_string(),
            timestamp: now,
            remote_device_id: None,
        })
    }

    /// Force push: upload local data, overwriting remote.
    pub async fn force_push(&self) -> Result<(), String> {
        let db = self.get_db().await?;
        let sync_password = self.get_sync_password(&db).await?;
        let config = self.load_config(&db).await?;
        let provider = build_provider(&config)?;
        let device_id = self.get_device_id(&db).await?;

        let snapshot = self.export_snapshot(&db, &device_id).await?;
        let plaintext = serde_json::to_vec(&snapshot)
            .map_err(|e| format!("[SYNC_CONFIG_ERROR] Serialize: {e}"))?;
        let encrypted = crypto::encrypt(&sync_password, &plaintext)?;
        provider.put_object(SNAPSHOT_KEY, &encrypted).await?;

        db.set_sync_state("last_synced_hash", &snapshot.snapshot_hash)
            .await?;
        db.set_sync_state(
            "last_sync_at",
            &chrono::Utc::now().to_rfc3339(),
        )
        .await?;
        db.set_sync_state("last_sync_result", "success").await?;

        Ok(())
    }

    /// Force pull: download remote data, overwriting local.
    pub async fn force_pull(&self) -> Result<(), String> {
        let db = self.get_db().await?;
        let sync_password = self.get_sync_password(&db).await?;
        let config = self.load_config(&db).await?;
        let provider = build_provider(&config)?;

        let remote_encrypted = provider
            .get_object(SNAPSHOT_KEY)
            .await?
            .ok_or_else(|| "[SYNC_CONNECTION_ERROR] No remote snapshot found".to_string())?;

        let remote_plaintext = crypto::decrypt(&sync_password, &remote_encrypted)?;
        let remote_snapshot: SyncSnapshot = serde_json::from_slice(&remote_plaintext)
            .map_err(|e| format!("[SYNC_MERGE_ERROR] Invalid remote snapshot: {e}"))?;

        // Verify hash
        let data_json = serde_json::to_vec(&remote_snapshot.data)
            .map_err(|e| format!("[SYNC_MERGE_ERROR] Serialize: {e}"))?;
        let computed_hash = crypto::snapshot_hash(&data_json);
        if computed_hash != remote_snapshot.snapshot_hash {
            return Err("[SYNC_CRYPTO_ERROR] Remote snapshot hash mismatch".to_string());
        }

        self.import_snapshot(&db, &remote_snapshot).await?;
        db.set_sync_state("last_synced_hash", &computed_hash).await?;
        db.set_sync_state(
            "last_sync_at",
            &chrono::Utc::now().to_rfc3339(),
        )
        .await?;
        db.set_sync_state("last_sync_result", "success").await?;

        Ok(())
    }

    /// Update sync password (re-encrypt and re-upload).
    pub async fn update_password(
        &self,
        old_password: &str,
        new_password: &str,
    ) -> Result<(), String> {
        let db = self.get_db().await?;
        let config = self.load_config(&db).await?;
        let provider = build_provider(&config)?;

        // Download with old password
        let remote_encrypted = provider
            .get_object(SNAPSHOT_KEY)
            .await?
            .ok_or_else(|| "[SYNC_CONNECTION_ERROR] No remote snapshot found".to_string())?;

        let remote_plaintext = crypto::decrypt(old_password, &remote_encrypted)?;
        let _: SyncSnapshot = serde_json::from_slice(&remote_plaintext)
            .map_err(|e| format!("[SYNC_PASSWORD_ERROR] Old password incorrect: {e}"))?;

        // Re-encrypt with new password and upload
        let encrypted = crypto::encrypt(new_password, &remote_plaintext)?;
        provider.put_object(SNAPSHOT_KEY, &encrypted).await?;

        // Update stored encrypted password
        let encrypted_pw = db.encrypt_sync_password(new_password)?;
        db.set_sync_state("sync_password_enc", &encrypted_pw).await?;

        Ok(())
    }

    /// Check if local data has changed since last sync.
    pub async fn has_local_changes(&self) -> Result<bool, String> {
        let db = self.get_db().await?;
        let last_hash = db.get_sync_state("last_synced_hash").await?;
        if last_hash.is_none() {
            return Ok(true);
        }

        let device_id = self.get_device_id(&db).await?;
        let snapshot = self.export_snapshot(&db, &device_id).await?;
        Ok(Some(snapshot.snapshot_hash) != last_hash)
    }

    /// Auto-sync push if local has changes.
    pub async fn auto_sync_push(&self) -> Result<(), String> {
        if !self.has_local_changes().await? {
            return Ok(());
        }
        let db = self.get_db().await?;
        let sync_password = self.get_sync_password(&db).await?;
        let config = self.load_config(&db).await?;
        let provider = build_provider(&config)?;
        let device_id = self.get_device_id(&db).await?;

        let snapshot = self.export_snapshot(&db, &device_id).await?;
        let plaintext = serde_json::to_vec(&snapshot)
            .map_err(|e| format!("[SYNC_CONFIG_ERROR] Serialize: {e}"))?;
        let encrypted = crypto::encrypt(&sync_password, &plaintext)?;
        provider.put_object(SNAPSHOT_KEY, &encrypted).await?;

        db.set_sync_state("last_synced_hash", &snapshot.snapshot_hash)
            .await?;
        db.set_sync_state(
            "last_sync_at",
            &chrono::Utc::now().to_rfc3339(),
        )
        .await?;
        db.set_sync_state("last_sync_result", "success").await?;

        Ok(())
    }

    // ── Private helpers ──────────────────────────────────

    /// Retrieve the stored sync password (decrypted).
    async fn get_sync_password(&self, db: &LocalDb) -> Result<String, String> {
        let encrypted = db
            .get_sync_state("sync_password_enc")
            .await?
            .ok_or_else(|| {
                "[SYNC_CONFIG_ERROR] Sync password not stored. Please reconfigure sync."
                    .to_string()
            })?;
        db.decrypt_sync_password(&encrypted)
    }

    async fn load_config(&self, db: &LocalDb) -> Result<SyncConfig, String> {
        let config_json = db
            .get_sync_state("sync_config")
            .await?
            .ok_or_else(|| "[SYNC_CONFIG_ERROR] Sync not configured".to_string())?;
        serde_json::from_str(&config_json)
            .map_err(|e| format!("[SYNC_CONFIG_ERROR] Parse config: {e}"))
    }

    async fn get_device_id(&self, db: &LocalDb) -> Result<String, String> {
        db.get_sync_state("device_id")
            .await?
            .ok_or_else(|| "[SYNC_CONFIG_ERROR] Device ID not set".to_string())
    }

    /// Export current local data to a SyncSnapshot.
    async fn export_snapshot(
        &self,
        db: &LocalDb,
        device_id: &str,
    ) -> Result<SyncSnapshot, String> {
        let connections = db.list_connections().await?;
        let saved_queries = db.list_saved_queries().await?;
        let ai_providers = db.list_ai_providers().await?;

        // Decrypt AI API keys for transport (will be re-encrypted on import)
        let ai_providers_json: Vec<serde_json::Value> = ai_providers
            .iter()
            .map(|p| {
                let mut val = serde_json::to_value(p).unwrap_or_default();
                if let Some(api_key) = val.get("apiKey").and_then(|v| v.as_str()) {
                    if LocalDb::has_encrypted_ai_api_key(api_key) {
                        if let Ok(decrypted) = db.decrypt_ai_api_key(api_key) {
                            val["apiKey"] = serde_json::Value::String(decrypted);
                        }
                    }
                }
                val
            })
            .collect();

        let data = SyncSnapshotData {
            connections: serde_json::to_value(&connections)
                .unwrap_or_default()
                .as_array()
                .cloned()
                .unwrap_or_default(),
            saved_queries: serde_json::to_value(&saved_queries)
                .unwrap_or_default()
                .as_array()
                .cloned()
                .unwrap_or_default(),
            ai_providers: ai_providers_json,
            settings: serde_json::Value::Object(serde_json::Map::new()),
        };

        let data_json = serde_json::to_vec(&data)
            .map_err(|e| format!("[SYNC_CONFIG_ERROR] Serialize data: {e}"))?;
        let hash = crypto::snapshot_hash(&data_json);

        Ok(SyncSnapshot {
            version: 1,
            device_id: device_id.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            snapshot_hash: hash,
            data,
        })
    }

    /// Import a remote snapshot, overwriting local data.
    async fn import_snapshot(
        &self,
        db: &LocalDb,
        snapshot: &SyncSnapshot,
    ) -> Result<(), String> {
        // Clear and re-import connections
        let existing_connections = db.list_connections().await?;
        for conn in &existing_connections {
            db.delete_connection(conn.id).await?;
        }
        for conn_val in &snapshot.data.connections {
            let form: crate::models::ConnectionForm = serde_json::from_value(conn_val.clone())
                .map_err(|e| format!("[SYNC_MERGE_ERROR] Parse connection: {e}"))?;
            db.create_connection(form).await?;
        }

        // Clear and re-import saved queries
        let existing_queries = db.list_saved_queries().await?;
        for q in &existing_queries {
            db.delete_saved_query(q.id).await?;
        }
        for q_val in &snapshot.data.saved_queries {
            let name = q_val
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let query = q_val
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let description = q_val
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from);
            let connection_id = q_val
                .get("connectionId")
                .or_else(|| q_val.get("connection_id"))
                .and_then(|v| v.as_i64());
            let database = q_val
                .get("database")
                .and_then(|v| v.as_str())
                .map(String::from);
            db.create_saved_query(name, query, description, connection_id, database)
                .await?;
        }

        // Clear and re-import AI providers
        let existing_providers = db.list_ai_providers().await?;
        for p in &existing_providers {
            db.delete_ai_provider(p.id).await?;
        }
        for p_val in &snapshot.data.ai_providers {
            let form: crate::models::AiProviderForm =
                serde_json::from_value(p_val.clone())
                    .map_err(|e| format!("[SYNC_MERGE_ERROR] Parse AI provider: {e}"))?;
            // API key is plaintext from snapshot, will be encrypted by create_ai_provider
            db.create_ai_provider(form).await?;
        }

        Ok(())
    }
}
