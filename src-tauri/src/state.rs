use crate::datasources::redis::RedisConnectionCache;
use crate::db::local::LocalDb;
use crate::db::pool_manager::PoolManager;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AppState {
    pub local_db: Arc<Mutex<Option<Arc<LocalDb>>>>,
    pub pool_manager: Arc<PoolManager>,
    pub redis_cache: Mutex<RedisConnectionCache>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            local_db: Arc::new(Mutex::new(None)),
            pool_manager: Arc::new(PoolManager::new()),
            redis_cache: Mutex::new(RedisConnectionCache::new()),
        }
    }
}
