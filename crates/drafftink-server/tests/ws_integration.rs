//! WebSocket integration tests for room persistence.

use std::sync::Arc;
use std::time::Duration;

use drafftink_protocol::ClientMessage;
use drafftink_server::config::{ServerConfig, StoreKind};
use drafftink_server::store::FileRoomStore;
use drafftink_server::{AppState, router};
use futures_util::{SinkExt, StreamExt};
use serde_json;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};

async fn spawn_test_server(store: Arc<dyn drafftink_server::store::RoomStore>) -> (String, tokio::task::JoinHandle<()>) {
    let state = Arc::new(AppState::new(store, 16 * 1024 * 1024));
    let app = router(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("ws://{}/ws", addr);

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (url, handle)
}

async fn recv_json<S>(read: &mut S) -> serde_json::Value
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    for _ in 0..50 {
        if let Some(Ok(Message::Text(t))) = read.next().await {
            return serde_json::from_str(&t).unwrap();
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timeout waiting for message");
}

#[tokio::test]
async fn join_and_sync_persisted_after_reconnect() {
    let dir = TempDir::new().unwrap();
    let store: Arc<dyn drafftink_server::store::RoomStore> = Arc::new(FileRoomStore::new(
        dir.path().to_path_buf(),
        None,
        32 * 1024 * 1024,
    ));

    let (url, server) = spawn_test_server(store.clone()).await;

    let sync_payload = "dGVzdA=="; // "test" in base64

    // Client 1: join and sync
    let (ws1, _) = connect_async(&url).await.unwrap();
    let (mut write1, mut read1) = ws1.split();
    let join = serde_json::to_string(&ClientMessage::Join {
        room: "persist-room".to_string(),
    })
    .unwrap();
    write1.send(Message::Text(join.into())).await.unwrap();
    let joined = recv_json(&mut read1).await;
    assert_eq!(joined["type"], "joined");

    let sync = serde_json::to_string(&ClientMessage::SyncSnapshot {
        data: sync_payload.to_string(),
    })
    .unwrap();
    write1.send(Message::Text(sync.into())).await.unwrap();
    drop(write1);
    drop(read1);

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Client 2: join should get initial_sync
    let (ws2, _) = connect_async(&url).await.unwrap();
    let (mut write2, mut read2) = ws2.split();
    write2
        .send(
            Message::Text(
                serde_json::to_string(&ClientMessage::Join {
                    room: "persist-room".to_string(),
                })
                .unwrap()
                .into(),
            ),
        )
        .await
        .unwrap();
    let joined2 = recv_json(&mut read2).await;
    assert_eq!(joined2["type"], "joined");
    assert_eq!(joined2["initial_sync"].as_str(), Some(sync_payload));

    drop(write2);
    drop(read2);
    server.abort();
}

#[tokio::test]
async fn request_snapshot_returns_persisted_data() {
    let dir = TempDir::new().unwrap();
    let store: Arc<dyn drafftink_server::store::RoomStore> = Arc::new(FileRoomStore::new(
        dir.path().to_path_buf(),
        None,
        32 * 1024 * 1024,
    ));
    store
        .save_snapshot("snap-room", "YWJj")
        .await;

    let (url, server) = spawn_test_server(store).await;
    let (ws, _) = connect_async(&url).await.unwrap();
    let (mut write, mut read) = ws.split();

    write
        .send(
            Message::Text(
                serde_json::to_string(&ClientMessage::Join {
                    room: "snap-room".to_string(),
                })
                .unwrap()
                .into(),
            ),
        )
        .await
        .unwrap();
    let _ = recv_json(&mut read).await;

    write
        .send(Message::Text(
            serde_json::to_string(&ClientMessage::RequestSnapshot).unwrap().into(),
        ))
        .await
        .unwrap();

    let msg = recv_json(&mut read).await;
    assert_eq!(msg["type"], "room_snapshot");
    assert_eq!(msg["data"].as_str(), Some("YWJj"));

    server.abort();
}

#[test]
fn config_from_env_defaults() {
    let cfg = ServerConfig::default();
    assert_eq!(cfg.port, 3030);
    assert!(matches!(cfg.store, StoreKind::Memory));
}
