use super::handler::RequestHandler;
use super::transport::stdio::StdioTransport;
use super::transport::Transport;
use crate::state::AppState;
use std::sync::Arc;

pub struct McpServer {
    handler: RequestHandler,
    transport: Box<dyn Transport>,
}

impl McpServer {
    pub fn new(state: Arc<AppState>) -> Self {
        let handler = RequestHandler::new(state);
        let transport = Box::new(StdioTransport::new());
        Self { handler, transport }
    }

    pub fn with_transport(state: Arc<AppState>, transport: Box<dyn Transport>) -> Self {
        let handler = RequestHandler::new(state);
        Self { handler, transport }
    }

    pub async fn run(&mut self) -> Result<(), String> {
        eprintln!("DbPaw MCP Server started");

        loop {
            match self.transport.receive().await {
                Ok(Some(request)) => {
                    let response = self.handler.handle(request).await;
                    if let Some(resp) = response {
                        self.transport.send(&resp).await.map_err(|e| e.to_string())?;
                    }
                }
                Ok(None) => {
                    break;
                }
                Err(e) => {
                    eprintln!("Error receiving request: {}", e);
                    break;
                }
            }
        }

        eprintln!("DbPaw MCP Server stopped");
        Ok(())
    }
}
