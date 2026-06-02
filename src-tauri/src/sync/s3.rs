// Stub — will be implemented in Task 4
use crate::sync::provider::SyncProvider;
use async_trait::async_trait;

pub struct S3Provider;

impl S3Provider {
    pub fn new(
        _endpoint: String,
        _region: String,
        _bucket: String,
        _access_key_id: String,
        _secret_access_key: String,
        _path_prefix: String,
    ) -> Self {
        Self
    }
}

#[async_trait]
impl SyncProvider for S3Provider {
    async fn test_connection(&self) -> Result<(), String> { todo!() }
    async fn put_object(&self, _key: &str, _data: &[u8]) -> Result<(), String> { todo!() }
    async fn get_object(&self, _key: &str) -> Result<Option<Vec<u8>>, String> { todo!() }
    async fn delete_object(&self, _key: &str) -> Result<(), String> { todo!() }
}
