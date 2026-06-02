use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{watch, Mutex};

const DEFAULT_INTERVAL_MINUTES: u32 = 5;

/// Background sync scheduler that runs periodic auto-sync and
/// responds to data-change events with debounced push.
pub struct SyncScheduler {
    local_db: Arc<Mutex<Option<Arc<crate::db::local::LocalDb>>>>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
}

impl SyncScheduler {
    pub fn new(local_db: Arc<Mutex<Option<Arc<crate::db::local::LocalDb>>>>) -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        Self {
            local_db,
            shutdown_tx,
            shutdown_rx,
        }
    }

    /// Start the periodic sync loop. Call once during app startup.
    pub fn start(&self) {
        let local_db = self.local_db.clone();
        let mut shutdown = self.shutdown_rx.clone();

        tokio::spawn(async move {
            loop {
                // Read configured interval from DB
                let interval_secs = Self::get_sync_interval_secs(&local_db).await;

                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(interval_secs)) => {
                        let _ = Self::try_auto_sync(&local_db).await;
                    }
                    _ = shutdown.changed() => {
                        if *shutdown.borrow() {
                            println!("[SYNC_SCHEDULER] Shutting down");
                            return;
                        }
                    }
                }
            }
        });

        println!("[SYNC_SCHEDULER] Started");
    }

    /// Notify the scheduler that local data has changed.
    /// Triggers a debounced push after a short delay.
    pub fn notify_data_changed(&self) {
        let local_db = self.local_db.clone();

        tokio::spawn(async move {
            // Debounce: wait 3 seconds before syncing
            tokio::time::sleep(Duration::from_secs(3)).await;
            let _ = Self::try_auto_sync(&local_db).await;
        });
    }

    /// Shut down the scheduler.
    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    /// Read configured sync interval from DB (in seconds).
    async fn get_sync_interval_secs(
        local_db: &Arc<Mutex<Option<Arc<crate::db::local::LocalDb>>>>,
    ) -> u64 {
        let lock = local_db.lock().await;
        if let Some(db) = lock.as_ref() {
            if let Ok(Some(val)) = db.get_sync_state("sync_interval_minutes").await {
                if let Ok(mins) = val.parse::<u32>() {
                    return (mins.max(1) as u64) * 60;
                }
            }
        }
        (DEFAULT_INTERVAL_MINUTES as u64) * 60
    }

    /// Attempt auto-sync if conditions are met (enabled + password stored).
    async fn try_auto_sync(
        local_db: &Arc<Mutex<Option<Arc<crate::db::local::LocalDb>>>>,
    ) -> Result<(), String> {
        let manager = crate::sync::manager::SyncManager::new(local_db.clone());

        // Check if sync is enabled
        let status = manager.get_status().await?;
        if !status.enabled || !status.password_stored {
            return Ok(());
        }

        // Check if local data has changed
        if !manager.has_local_changes().await? {
            return Ok(());
        }

        match manager.auto_sync_push().await {
            Ok(()) => {
                println!("[SYNC_SCHEDULER] Auto-sync push succeeded");
                Ok(())
            }
            Err(e) => {
                eprintln!("[SYNC_SCHEDULER] Auto-sync failed: {}", e);
                Err(e)
            }
        }
    }
}
