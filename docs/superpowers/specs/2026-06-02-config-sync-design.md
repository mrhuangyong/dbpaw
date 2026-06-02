# Config Sync Design — DbPaw

## Summary

Add configuration synchronization across devices via S3 or WebDAV, with end-to-end encryption. Supports manual and automatic sync modes with Last-Write-Wins conflict resolution.

## Scope

### Synced Data
- Database connection configurations (connections table)
- Saved queries (saved_queries table)
- AI provider configurations (ai_providers table, excluding conversations/messages)
- User settings (settings.json via tauri-plugin-store)
- Keyboard shortcuts (stored in settings)

### NOT Synced
- AI conversations and messages — device-local, high volume
- SQL/Redis execution logs — device-local, transient
- AI master key — per-device encryption key
- Connection pool state — runtime-only

## Architecture

```
Frontend (React)
  SettingsDialog → Sync Tab (SyncSettings.tsx)
       │
       ▼ invoke()
Backend (Rust)
  SyncManager
    ├── CryptoEngine (PBKDF2 + AES-256-GCM)
    ├── Snapshot export/import
    ├── Change detection (SHA-256 hash)
    └── Auto-sync timer (tokio interval)
         │
         ▼ SyncProvider trait
    ┌────────┬──────────┐
    │  S3    │  WebDAV  │
    └────────┴──────────┘
```

## SyncProvider Trait

```rust
#[async_trait]
pub trait SyncProvider: Send + Sync {
    async fn test_connection(&self) -> Result<(), String>;
    async fn put_object(&self, key: &str, data: &[u8]) -> Result<(), String>;
    async fn get_object(&self, key: &str) -> Result<Option<Vec<u8>>, String>;
    async fn delete_object(&self, key: &str) -> Result<(), String>;
}
```

### S3 Provider
- Config: endpoint, region, bucket, access_key_id, secret_access_key, path_prefix
- Uses `reqwest` + manual AWS Signature V4 (no heavy AWS SDK dependency)
- Remote path: `s3://{bucket}/{path_prefix}sync_snapshot.enc`

### WebDAV Provider
- Config: server_url, username, password
- Uses `reqwest` with standard HTTP PUT/GET/DELETE
- Remote path: `{server_url}/sync_snapshot.enc`

## Encryption

```
User password → PBKDF2-SHA256 (600k iterations, random 16-byte salt) → AES-256-GCM key
Plaintext snapshot JSON → AES-256-GCM (random 12-byte nonce) → ciphertext

File format: [16 bytes salt][12 bytes nonce][ciphertext + GCM tag]
```

- Snapshot includes a `snapshot_hash` (SHA-256 of plaintext) for integrity verification after decryption
- Wrong password → GCM tag verification fails → user-friendly error

## Snapshot Format

```json
{
  "version": 1,
  "device_id": "uuid",
  "timestamp": "2026-06-02T10:30:00Z",
  "snapshot_hash": "sha256-of-plaintext",
  "data": {
    "connections": [...],
    "saved_queries": [...],
    "ai_providers": [...],
    "settings": { "key": "value" }
  }
}
```

### Sensitive Field Handling
- Connection passwords: plaintext inside encrypted snapshot (E2E encryption protects in transit/at rest)
- AI API Keys: plaintext inside snapshot; on import, re-encrypted with local `ai_master.key`
- SSH key paths: synced as-is (users may need to verify paths on different OS)
- Provider credentials (S3 secret key / WebDAV password): stored locally encrypted with `ai_master.key`

## Sync Mode: Hybrid

### Auto Sync (default)
- App startup → 30s delay (wait for LocalDb init) → first pull
- Every 5 minutes → hash comparison → push if changed
- Configurable interval
- Silent failure on network errors, recorded in `last_sync_result`

### Manual Sync
- "Sync Now" → pull + push
- "Force Push" → local overwrites remote
- "Force Pull" → remote overwrites local

## Conflict Resolution: Last-Write-Wins

```
Pull remote → compare timestamps:
  - remote.timestamp > local.timestamp AND remote.device_id != local.device_id → apply remote
  - otherwise → skip (local is newer or same device)
```

Applying remote data:
1. Clear local tables (connections, saved_queries, ai_providers)
2. Insert remote data
3. Update settings.json keys
4. Re-encrypt AI API keys with local `ai_master.key`
5. Update `last_synced_hash`

## Change Detection

```
local data → export to JSON → SHA-256 hash → compare with last_synced_hash
  - different → local has changes → push
  - same → no changes → skip
```

`last_synced_hash` stored in `sync_state` table in SQLite.

## Database: sync_state Table

```sql
CREATE TABLE IF NOT EXISTS sync_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Keys: `device_id`, `sync_config` (JSON, provider params without passwords), `sync_enabled`, `last_synced_hash`, `last_sync_at`, `last_sync_result`, `sync_password_hash` (for verification without storing plaintext)

## Tauri Commands

| Command | Purpose |
|---------|---------|
| `sync_test_connection(config)` | Validate provider connectivity |
| `sync_configure(config, sync_password)` | Save config + first upload |
| `sync_get_status()` | Return current sync state |
| `sync_now()` | Manual pull + push |
| `sync_force_push()` | Local overwrites remote |
| `sync_force_pull()` | Remote overwrites local |
| `sync_disable()` | Turn off sync, keep config |
| `sync_update_password(old, new)` | Re-encrypt with new password |

## Frontend

### New Files
- `src/components/settings/SyncSettings.tsx` — Sync tab in Settings dialog
- Type definitions for SyncConfig, SyncStatus, SyncResult

### Modified Files
- `src/services/api.ts` — Add `syncApi` namespace
- `src/components/settings/SettingsDialog.tsx` — Add Sync tab

### UI Layout
- Provider selector dropdown (S3 / WebDAV)
- Dynamic form fields based on provider
- Sync password + confirmation inputs
- Test Connection button with status indicator
- Auto-sync toggle + interval selector
- Sync status display (device ID, last sync time, result)
- Action buttons: Sync Now, Force Push, Force Pull, Disable

## Backend Files

### New Files
| File | Purpose |
|------|---------|
| `src-tauri/src/sync/mod.rs` | Module entry |
| `src-tauri/src/sync/provider.rs` | SyncProvider trait |
| `src-tauri/src/sync/crypto.rs` | PBKDF2 + AES-256-GCM |
| `src-tauri/src/sync/manager.rs` | SyncManager (export/import/timer/hash) |
| `src-tauri/src/sync/s3.rs` | S3 implementation (reqwest + Sig V4) |
| `src-tauri/src/sync/webdav.rs` | WebDAV implementation (reqwest) |
| `src-tauri/src/commands/sync.rs` | Tauri command handlers |
| `src-tauri/migrations/017_sync_state.sql` | sync_state table migration |

### Modified Files
| File | Change |
|------|--------|
| `src-tauri/src/lib.rs` | Register sync commands, start/stop auto-sync on app lifecycle |
| `src-tauri/src/state.rs` | Add `sync_manager` to AppState |
| `src-tauri/src/db/local.rs` | Add sync_state CRUD methods |
| `src-tauri/Cargo.toml` | Add `sha2`, `hmac`, `pbkdf2` dependencies |

## New Rust Dependencies

```toml
sha2 = "0.10"
hmac = "0.12"
pbkdf2 = "0.12"
# aes-gcm, reqwest, serde_json, chrono — already present
```

## Error Prefixes

- `[SYNC_CONFIG_ERROR]` — Invalid configuration
- `[SYNC_CONNECTION_ERROR]` — Remote connection failure
- `[SYNC_CRYPTO_ERROR]` — Encryption/decryption failure
- `[SYNC_MERGE_ERROR]` — Data merge failure
- `[SYNC_PASSWORD_ERROR]` — Wrong sync password

## Edge Cases

| Scenario | Handling |
|----------|----------|
| Remote has no snapshot | First sync, push local data |
| Fresh install (no local data) | Pull remote data |
| Wrong sync password | GCM tag verification fails, show error |
| Remote unreachable | Auto-sync silent fail, record error, no impact on normal usage |
| App exit during sync | Cancel in-progress sync, don't block exit |
| Concurrent sync (two windows) | Mutex ensures single operation at a time |
| Schema version mismatch | Snapshot `version` field check on import |
| Rapid multi-device edits | Last-Write-Wins may lose intermediate changes (user-accepted tradeoff) |

## Auto-Sync Lifecycle

```
App startup → LocalDb init complete
  → SyncManager::new(state)
  → Read sync_state: enabled?
    → Yes: delay 30s → pull → start interval timer (5min, configurable)
    → No: idle

App exit (RunEvent::Exit):
  → Cancel timer
  → Don't wait for in-progress sync
```
