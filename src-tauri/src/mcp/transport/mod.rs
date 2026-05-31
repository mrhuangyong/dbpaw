pub mod http;
pub mod stdio;

use super::types::{JsonRpcRequest, JsonRpcResponse};
use async_trait::async_trait;

#[derive(Debug)]
pub enum TransportError {
    Io(std::io::Error),
    Parse(String),
    Closed,
    Other(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::Io(e) => write!(f, "IO error: {}", e),
            TransportError::Parse(s) => write!(f, "Parse error: {}", s),
            TransportError::Closed => write!(f, "Transport closed"),
            TransportError::Other(s) => write!(f, "{}", s),
        }
    }
}

impl From<std::io::Error> for TransportError {
    fn from(e: std::io::Error) -> Self {
        TransportError::Io(e)
    }
}

#[async_trait]
pub trait Transport: Send + Sync {
    async fn receive(&mut self) -> Result<Option<JsonRpcRequest>, TransportError>;
    async fn send(&mut self, response: &JsonRpcResponse) -> Result<(), TransportError>;
    async fn close(&mut self) -> Result<(), TransportError>;
}
