use super::super::types::{JsonRpcRequest, JsonRpcResponse};
use super::{Transport, TransportError};
use async_trait::async_trait;

pub struct HttpTransport;

impl HttpTransport {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn receive(&mut self) -> Result<Option<JsonRpcRequest>, TransportError> {
        Err(TransportError::Other("Not implemented yet".to_string()))
    }

    async fn send(&mut self, _response: &JsonRpcResponse) -> Result<(), TransportError> {
        Err(TransportError::Other("Not implemented yet".to_string()))
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        Ok(())
    }
}
