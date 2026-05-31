use super::super::types::{JsonRpcRequest, JsonRpcResponse};
use super::{Transport, TransportError};
use async_trait::async_trait;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

pub struct StdioTransport {
    reader: BufReader<io::Stdin>,
    stdout: io::Stdout,
}

impl StdioTransport {
    pub fn new() -> Self {
        Self {
            reader: BufReader::new(io::stdin()),
            stdout: io::stdout(),
        }
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn receive(&mut self) -> Result<Option<JsonRpcRequest>, TransportError> {
        let mut line = String::new();
        match self.reader.read_line(&mut line).await {
            Ok(0) => Ok(None),
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    return Ok(None);
                }
                serde_json::from_str(trimmed)
                    .map(Some)
                    .map_err(|e| TransportError::Parse(format!("{}", e)))
            }
            Err(e) => Err(TransportError::Io(e)),
        }
    }

    async fn send(&mut self, response: &JsonRpcResponse) -> Result<(), TransportError> {
        let json = serde_json::to_string(response)
            .map_err(|e| TransportError::Parse(format!("{}", e)))?;
        self.stdout.write_all(json.as_bytes()).await?;
        self.stdout.write_all(b"\n").await?;
        self.stdout.flush().await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        Ok(())
    }
}
