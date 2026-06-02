// Stub — will be implemented in Task 5
use crate::sync::provider::SyncProvider;
use async_trait::async_trait;

pub struct WebdavProvider;

impl WebdavProvider {
    pub fn new(_server_url: String, _username: String, _password: String) -> Self {
        Self
    }
}

#[async_trait]
impl SyncProvider for WebdavProvider {
    async fn test_connection(&self) -> Result<(), String> { todo!() }
    async fn put_object(&self, _key: &str, _data: &[u8]) -> Result<(), String> { todo!() }
    async fn get_object(&self, _key: &str) -> Result<Option<Vec<u8>>, String> { todo!() }
    async fn delete_object(&self, _key: &str) -> Result<(), String> { todo!() }
}
