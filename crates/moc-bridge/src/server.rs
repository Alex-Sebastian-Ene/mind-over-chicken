//! Asynchronous WebSocket server for the headset bridge.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tokio_tungstenite::tungstenite::Message;

use crate::frame::{BridgeFrame, StatusMessage, SyntheticGenerator};

pub struct BridgeServer {
    pub bind_addr: SocketAddr,
    tx: broadcast::Sender<String>,
}

impl BridgeServer {
    pub fn new(bind_addr: SocketAddr) -> Self {
        let (tx, _) = broadcast::channel(100);
        Self { bind_addr, tx }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(self.bind_addr).await?;
        println!("Native Rust Headset Bridge listening on ws://{}", self.bind_addr);

        let tx = self.tx.clone();
        let generator = Arc::new(Mutex::new(SyntheticGenerator::new(42)));

        // Background telemetry generation task (~10 Hz / 100 ms)
        let tx_broadcast = tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            loop {
                interval.tick().await;
                let frame: BridgeFrame = {
                    let mut gen = generator.lock().await;
                    gen.next_frame(0.1)
                };

                if let Ok(json) = serde_json::to_string(&frame) {
                    let _ = tx_broadcast.send(json);
                }
            }
        });

        // Connection acceptance loop
        while let Ok((stream, peer_addr)) = listener.accept().await {
            let tx_sub = self.tx.clone();
            tokio::spawn(handle_connection(stream, peer_addr, tx_sub));
        }

        Ok(())
    }
}

async fn handle_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    tx: broadcast::Sender<String>,
) {
    let ws_stream = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("Failed to complete WebSocket handshake with {}: {}", peer_addr, e);
            return;
        }
    };

    println!("Client connected: {}", peer_addr);
    let (mut write, mut read) = ws_stream.split();
    let mut rx = tx.subscribe();

    // Send initial status message
    let status = StatusMessage::new("Streaming from native Rust bridge");
    if let Ok(status_json) = serde_json::to_string(&status) {
        let _ = write.send(Message::Text(status_json.into())).await;
    }

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(frame_json) => {
                        if write.send(Message::Text(frame_json.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            client_msg = read.next() => {
                match client_msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }

    println!("Client disconnected: {}", peer_addr);
}
