use crate::datasources::redis::RedisConnectionCache;
use crate::db::local::LocalDb;
use crate::db::pool_manager::PoolManager;
use crate::sync::scheduler::SyncScheduler;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AppState {
    pub local_db: Arc<Mutex<Option<Arc<LocalDb>>>>,
    pub pool_manager: Arc<PoolManager>,
    pub redis_cache: Mutex<RedisConnectionCache>,
    pub sync_scheduler: SyncScheduler,
}

impl AppState {
    pub fn new() -> Self {
        let local_db: Arc<Mutex<Option<Arc<LocalDb>>>> =
            Arc::new(Mutex::new(None));
        let sync_scheduler = SyncScheduler::new(local_db.clone());
        Self {
            local_db,
            pool_manager: Arc::new(PoolManager::new()),
            redis_cache: Mutex::new(RedisConnectionCache::new()),
            sync_scheduler,
        }
    }
}
