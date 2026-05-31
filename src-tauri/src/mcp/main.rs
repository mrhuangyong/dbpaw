use dbpaw_lib::mcp::McpServer;
use dbpaw_lib::mcp::transport::http::HttpTransport;
use dbpaw_lib::state::AppState;
use std::net::SocketAddr;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let mut transport_mode = "stdio";
    let mut port: u16 = 3000;
    let mut host = "127.0.0.1";

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--transport" => {
                i += 1;
                if i < args.len() {
                    transport_mode = &args[i];
                }
            }
            "--port" => {
                i += 1;
                if i < args.len() {
                    port = args[i].parse().unwrap_or(3000);
                }
            }
            "--host" => {
                i += 1;
                if i < args.len() {
                    host = &args[i];
                }
            }
            "--help" | "-h" => {
                eprintln!("Usage: dbpaw-mcp [OPTIONS]");
                eprintln!();
                eprintln!("Options:");
                eprintln!("  --transport <stdio|http|both>  Transport mode (default: stdio)");
                eprintln!("  --port <PORT>                  HTTP port (default: 3000)");
                eprintln!("  --host <HOST>                  HTTP bind address (default: 127.0.0.1)");
                eprintln!("  --help, -h                     Show this help");
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    let state = Arc::new(AppState::new());

    match transport_mode {
        "stdio" => {
            let mut server = McpServer::new(state);
            server.run().await?;
        }
        "http" => {
            let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
            let http_transport = HttpTransport::new();
            let mut server = McpServer::with_transport(state, Box::new(http_transport));
            tokio::select! {
                result = server.run() => { result?; }
            }
        }
        "both" => {
            let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
            eprintln!("Starting in dual mode: stdio + http://{}", addr);
            let mut server = McpServer::new(state);
            server.run().await?;
        }
        _ => {
            eprintln!("Unknown transport mode: {}", transport_mode);
            eprintln!("Valid modes: stdio, http, both");
            std::process::exit(1);
        }
    }

    Ok(())
}
