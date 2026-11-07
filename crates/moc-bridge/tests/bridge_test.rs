use std::time::Duration;

use futures_util::StreamExt;
use tokio_tungstenite::connect_async;

use moc_bridge::frame::{BridgeFrame, StatusMessage, SyntheticGenerator};
use moc_bridge::BridgeServer;

#[test]
fn test_synthetic_generator_produces_valid_frames() {
    let mut gen = SyntheticGenerator::new(999);
    for _ in 0..50 {
        let frame = gen.next_frame(0.1);
        let b = &frame.bands;
        let sum = b.delta + b.theta + b.alpha + b.beta + b.gamma;
        assert!(
            (sum - 1.0).abs() < 1e-4,
            "Band powers must normalize to 1.0, got {}",
            sum
        );

        if let Some(hr) = frame.hr {
            assert!(hr >= 50.0 && hr <= 120.0, "HR out of reasonable range: {}", hr);
        }
    }
}

#[tokio::test]
async fn test_bridge_websocket_handshake_and_frames() {
    // Pick an available loopback port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let server = BridgeServer::new(addr);
    let server_task = tokio::spawn(async move {
        let _ = server.run().await;
    });

    // Give server a moment to bind
    tokio::time::sleep(Duration::from_millis(100)).await;

    let url = format!("ws://{}", addr);
    let (ws_stream, _) = connect_async(&url).await.expect("Failed to connect to bridge");
    let (_, mut read) = ws_stream.split();

    // 1. First message must be status
    let first_msg = tokio::time::timeout(Duration::from_secs(2), read.next())
        .await
        .expect("Timeout waiting for status")
        .expect("No message received")
        .expect("WebSocket read error");

    let status_text = first_msg.to_text().unwrap();
    let status: StatusMessage = serde_json::from_str(status_text).expect("Status JSON parse error");
    assert_eq!(status.r#type, "status");
    assert!(status.text.contains("Streaming"));

    // 2. Next message must be a telemetry frame
    let second_msg = tokio::time::timeout(Duration::from_secs(2), read.next())
        .await
        .expect("Timeout waiting for frame")
        .expect("No message received")
        .expect("WebSocket read error");

    let frame_text = second_msg.to_text().unwrap();
    let frame: BridgeFrame = serde_json::from_str(frame_text).expect("BridgeFrame JSON parse error");
    assert!(frame.t >= 0.0);
    assert!(frame.bands.alpha > 0.0);
    assert_eq!(frame.quality, 1.0);

    server_task.abort();
}
