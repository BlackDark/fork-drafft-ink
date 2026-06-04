//! DrafftInk WebSocket relay server with durable room storage.

pub mod config;
pub mod store;

use std::collections::HashSet;
use std::sync::Arc;

use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
    routing::get,
};
use dashmap::DashMap;
use drafftink_protocol::{ClientMessage, ServerMessage, base64_encode};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tracing::{info, warn};
use uuid::Uuid;

use crate::config::ServerConfig;
use crate::store::RoomStore;

const CHANNEL_CAPACITY: usize = 256;

struct Room {
    tx: broadcast::Sender<(String, ServerMessage)>,
    peers: HashSet<String>,
}

impl Room {
    fn new() -> Self {
        let (tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self {
            tx,
            peers: HashSet::new(),
        }
    }
}

/// Shared application state.
pub struct AppState {
    rooms: DashMap<String, Room>,
    store: Arc<dyn RoomStore>,
    max_ws_message_bytes: usize,
    #[cfg(feature = "redis")]
    redis: Option<Arc<crate::store::RedisRoomStore>>,
    #[cfg(feature = "redis")]
    redis_subscriptions: DashMap<String, ()>,
}

impl AppState {
    pub fn new(
        store: Arc<dyn RoomStore>,
        max_ws_message_bytes: usize,
        #[cfg(feature = "redis")] redis: Option<Arc<crate::store::RedisRoomStore>>,
    ) -> Self {
        Self {
            rooms: DashMap::new(),
            store,
            max_ws_message_bytes,
            #[cfg(feature = "redis")]
            redis,
            #[cfg(feature = "redis")]
            redis_subscriptions: DashMap::new(),
        }
    }

    async fn join_room(
        &self,
        room_id: &str,
        peer_id: &str,
    ) -> (
        broadcast::Receiver<(String, ServerMessage)>,
        Option<String>,
        usize,
    ) {
        let mut room = self
            .rooms
            .entry(room_id.to_string())
            .or_insert_with(Room::new);
        room.peers.insert(peer_id.to_string());
        let rx = room.tx.subscribe();
        let peer_count = room.peers.len();
        drop(room);

        #[cfg(feature = "redis")]
        self.ensure_redis_subscription(room_id);

        let initial_sync = self.store.load_snapshot(room_id).await;
        (rx, initial_sync, peer_count)
    }

    #[cfg(feature = "redis")]
    fn ensure_redis_subscription(&self, room_id: &str) {
        if self.redis.is_none() {
            return;
        }
        if self.redis_subscriptions.contains_key(room_id) {
            return;
        }
        let redis_url = self.redis.as_ref().unwrap().redis_url().to_string();
        let room_tx = self
            .rooms
            .get(room_id)
            .map(|room| room.tx.clone());
        let Some(room_tx) = room_tx else {
            return;
        };
        let room_id = room_id.to_string();
        self.redis_subscriptions.insert(room_id.clone(), ());

        tokio::spawn(async move {
            let Ok(client) = redis::Client::open(redis_url.as_str()) else {
                return;
            };
            let Ok(mut pubsub) = client.get_async_pubsub().await else {
                return;
            };
            let channel = crate::store::RedisRoomStore::events_channel(&room_id);
            if pubsub.subscribe(&channel).await.is_err() {
                return;
            }
            let mut stream = pubsub.on_message();
            while let Some(msg) = stream.next().await {
                let Ok(payload): Result<String, _> = msg.get_payload() else {
                    continue;
                };
                let Ok((from, server_msg)) =
                    serde_json::from_str::<(String, ServerMessage)>(&payload)
                else {
                    continue;
                };
                let _ = room_tx.send((from, server_msg));
            }
        });
    }

    fn leave_room(&self, room_id: &str, peer_id: &str) {
        if let Some(mut room) = self.rooms.get_mut(room_id) {
            room.peers.remove(peer_id);
            if room.peers.is_empty() {
                drop(room);
                self.rooms.remove(room_id);
                #[cfg(feature = "redis")]
                self.redis_subscriptions.remove(room_id);
            }
        }
    }

    async fn persist_sync(&self, room_id: &str, data_b64: &str) {
        let _ = self.store.save_snapshot(room_id, data_b64).await;
    }

    fn broadcast(&self, room_id: &str, from: &str, msg: ServerMessage) {
        if let Some(room) = self.rooms.get(room_id) {
            let _ = room.tx.send((from.to_string(), msg.clone()));
        }
        #[cfg(feature = "redis")]
        if let Some(redis) = &self.redis {
            if let Ok(payload) = serde_json::to_string(&(from.to_string(), msg)) {
                let redis = redis.clone();
                let room_id = room_id.to_string();
                tokio::spawn(async move {
                    let _ = redis.publish_event(&room_id, &payload).await;
                });
            }
        }
    }

    pub async fn is_ready(&self) -> bool {
        self.store.is_ready().await
    }
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/ws", get(ws_handler))
        .route("/health", get(health))
        .route("/ready", get(ready))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub async fn run(config: ServerConfig) -> std::io::Result<()> {
    #[cfg(feature = "redis")]
    let (store, redis) = {
        if matches!(config.store, crate::config::StoreKind::Redis) {
            let url = config
                .redis_url
                .clone()
                .unwrap_or_else(|| "redis://127.0.0.1:6379".to_string());
            let redis = Arc::new(
                crate::store::RedisRoomStore::connect(&url, config.max_room_bytes)
                    .await
                    .map_err(|e| std::io::Error::other(e.to_string()))?,
            );
            let store: Arc<dyn RoomStore> = redis.clone();
            (store, Some(redis))
        } else {
            let store = config.build_store().await;
            (store, None)
        }
    };

    #[cfg(not(feature = "redis"))]
    let store = config.build_store().await;

    if !store.is_ready().await {
        return Err(std::io::Error::other("room store not ready"));
    }

    let state = Arc::new(AppState::new(
        store,
        config.max_ws_message_bytes,
        #[cfg(feature = "redis")]
        redis,
    ));
    let app = router(state);
    let addr = config.socket_addr();
    info!("DrafftInk relay listening on {}", addr);
    info!("WebSocket: ws://{}:{}/ws", config.host, config.port);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index() -> &'static str {
    "DrafftInk Relay Server - WebSocket at /ws"
}

async fn health() -> &'static str {
    "ok"
}

async fn ready(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    if state.is_ready().await {
        "ok"
    } else {
        "unavailable"
    }
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn push_room_snapshot(
    state: &AppState,
    room_id: &str,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
) -> bool {
    let data = state.store.load_snapshot(room_id).await;
    let msg = ServerMessage::RoomSnapshot { data };
    let json = serde_json::to_string(&msg).unwrap();
    sender
        .send(Message::Text(json.into()))
        .await
        .is_ok()
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let peer_id = Uuid::new_v4().to_string();
    info!("New connection: {}", peer_id);

    let (mut sender, mut receiver) = socket.split();
    let mut current_room: Option<String> = None;
    let mut room_rx: Option<broadcast::Receiver<(String, ServerMessage)>> = None;
    let mut pending_room_snapshot = false;

    loop {
        tokio::select! {
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if text.len() > state.max_ws_message_bytes {
                            warn!("Message too large from {}", peer_id);
                            break;
                        }
                        match serde_json::from_str::<ClientMessage>(&text) {
                            Ok(client_msg) => {
                                if !handle_client_message(
                                    &state,
                                    &mut sender,
                                    &peer_id,
                                    &mut current_room,
                                    &mut room_rx,
                                    client_msg,
                                ).await {
                                    break;
                                }
                            }
                            Err(e) => {
                                warn!("Invalid message from {}: {}", peer_id, e);
                                let err = ServerMessage::Error {
                                    message: format!("Invalid message: {}", e),
                                };
                                let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                            }
                        }
                    }
                    Some(Ok(Message::Binary(data))) => {
                        if data.len() > state.max_ws_message_bytes {
                            break;
                        }
                        if let Some(ref room) = current_room {
                            let data_b64 = base64_encode(&data);
                            state.persist_sync(room, &data_b64).await;
                            state.broadcast(room, &peer_id, ServerMessage::Sync {
                                from: peer_id.clone(),
                                data: data_b64,
                            });
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        warn!("WebSocket error for {}: {}", peer_id, e);
                        break;
                    }
                }
            }
            msg = async {
                match &mut room_rx {
                    Some(rx) => match rx.recv().await {
                        Ok(m) => Some(Ok(m)),
                        Err(broadcast::error::RecvError::Lagged(n)) => Some(Err(n)),
                        Err(broadcast::error::RecvError::Closed) => None,
                    },
                    None => std::future::pending::<Option<Result<(String, ServerMessage), u64>>>().await,
                }
            } => {
                match msg {
                    Some(Err(lagged)) => {
                        warn!(
                            "Peer {} lagged {} broadcast messages in room {:?}",
                            peer_id, lagged, current_room
                        );
                        pending_room_snapshot = true;
                    }
                    Some(Ok((from, server_msg))) => {
                        if from != peer_id {
                            let json = serde_json::to_string(&server_msg).unwrap();
                            if sender.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    None => {}
                }
                if pending_room_snapshot {
                    if let Some(ref room) = current_room {
                        if !push_room_snapshot(&state, room, &mut sender).await {
                            break;
                        }
                    }
                    pending_room_snapshot = false;
                }
            }
        }
    }

    if let Some(ref room) = current_room {
        state.leave_room(room, &peer_id);
        state.broadcast(
            room,
            &peer_id,
            ServerMessage::PeerLeft {
                peer_id: peer_id.clone(),
            },
        );
    }
    info!("Connection closed: {}", peer_id);
}

async fn handle_client_message(
    state: &AppState,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    peer_id: &str,
    current_room: &mut Option<String>,
    room_rx: &mut Option<broadcast::Receiver<(String, ServerMessage)>>,
    client_msg: ClientMessage,
) -> bool {
    match client_msg {
        ClientMessage::Join { room } => {
            if let Some(ref old_room) = *current_room {
                state.leave_room(old_room, peer_id);
                state.broadcast(
                    old_room,
                    peer_id,
                    ServerMessage::PeerLeft {
                        peer_id: peer_id.to_string(),
                    },
                );
            }

            let (rx, initial_sync, peer_count) = state.join_room(&room, peer_id).await;
            *room_rx = Some(rx);
            *current_room = Some(room.clone());

            let joined = ServerMessage::Joined {
                room: room.clone(),
                peer_count,
                initial_sync,
            };
            if sender
                .send(Message::Text(serde_json::to_string(&joined).unwrap().into()))
                .await
                .is_err()
            {
                return false;
            }

            state.broadcast(
                &room,
                peer_id,
                ServerMessage::PeerJoined {
                    peer_id: peer_id.to_string(),
                },
            );
            info!("Peer {} joined room {}", peer_id, room);
        }
        ClientMessage::Leave => {
            if let Some(ref room) = *current_room {
                state.leave_room(room, peer_id);
                state.broadcast(
                    room,
                    peer_id,
                    ServerMessage::PeerLeft {
                        peer_id: peer_id.to_string(),
                    },
                );
                info!("Peer {} left room {}", peer_id, room);
            }
            *current_room = None;
            *room_rx = None;
        }
        ClientMessage::Sync { data } => {
            if let Some(ref room) = *current_room {
                state.broadcast(
                    room,
                    peer_id,
                    ServerMessage::Sync {
                        from: peer_id.to_string(),
                        data,
                    },
                );
            }
        }
        ClientMessage::SyncSnapshot { data } => {
            if let Some(ref room) = *current_room {
                state.persist_sync(room, &data).await;
                state.broadcast(
                    room,
                    peer_id,
                    ServerMessage::Sync {
                        from: peer_id.to_string(),
                        data,
                    },
                );
            }
        }
        ClientMessage::RequestSnapshot => {
            if let Some(ref room) = *current_room {
                if !push_room_snapshot(state, room, sender).await {
                    return false;
                }
            }
        }
        ClientMessage::Awareness {
            peer_id: awareness_peer_id,
            state: awareness_state,
        } => {
            if let Some(ref room) = *current_room {
                state.broadcast(
                    room,
                    peer_id,
                    ServerMessage::Awareness {
                        from: peer_id.to_string(),
                        peer_id: awareness_peer_id,
                        state: awareness_state,
                    },
                );
            }
        }
    }
    true
}
