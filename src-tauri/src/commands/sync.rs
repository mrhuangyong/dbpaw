use crate::state::AppState;
use crate::sync::manager::SyncManager;
use crate::sync::provider::{SyncConfig, SyncResult, SyncStatus};
use tauri::State;

#[tauri::command]
pub async fn sync_test_connection(config: SyncConfig) -> Result<(), String> {
    let provider = crate::sync::provider::build_provider(&config)?;
    provider.test_connection().await
}

#[tauri::command]
pub async fn sync_configure(
    state: State<'_, AppState>,
    config: SyncConfig,
    sync_password: String,
) -> Result<(), String> {
    let manager = SyncManager::new(state.local_db.clone());
    manager.configure(&config, &sync_password).await
}

#[tauri::command]
pub async fn sync_get_status(state: State<'_, AppState>) -> Result<SyncStatus, String> {
    let manager = SyncManager::new(state.local_db.clone());
    manager.get_status().await
}

#[tauri::command]
pub async fn sync_now(
    state: State<'_, AppState>,
    sync_password: String,
) -> Result<SyncResult, String> {
    let manager = SyncManager::new(state.local_db.clone());
    manager.sync_now(&sync_password).await
}

#[tauri::command]
pub async fn sync_force_push(
    state: State<'_, AppState>,
    sync_password: String,
) -> Result<(), String> {
    let manager = SyncManager::new(state.local_db.clone());
    manager.force_push(&sync_password).await
}

#[tauri::command]
pub async fn sync_force_pull(
    state: State<'_, AppState>,
    sync_password: String,
) -> Result<(), String> {
    let manager = SyncManager::new(state.local_db.clone());
    manager.force_pull(&sync_password).await
}

#[tauri::command]
pub async fn sync_disable(state: State<'_, AppState>) -> Result<(), String> {
    let manager = SyncManager::new(state.local_db.clone());
    manager.disable().await
}

#[tauri::command]
pub async fn sync_update_password(
    state: State<'_, AppState>,
    old_password: String,
    new_password: String,
) -> Result<(), String> {
    let manager = SyncManager::new(state.local_db.clone());
    manager.update_password(&old_password, &new_password).await
}
