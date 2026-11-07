use std::net::SocketAddr;
use moc_bridge::BridgeServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr: SocketAddr = "127.0.0.1:8765".parse()?;
    let server = BridgeServer::new(addr);
    server.run().await
}
