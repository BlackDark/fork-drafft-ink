//! Server configuration from environment.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use crate::store::{FileRoomStore, MemoryRoomStore, RoomStore};
#[cfg(feature = "redis")]
use crate::store::RedisRoomStore;

#[derive(Debug, Clone)]
pub enum StoreKind {
    Memory,
    File,
    #[cfg(feature = "redis")]
    Redis,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub store: StoreKind,
    pub persistence_dir: PathBuf,
    pub room_ttl_secs: Option<u64>,
    pub max_room_bytes: usize,
    pub max_ws_message_bytes: usize,
    #[cfg(feature = "redis")]
    pub redis_url: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 3030,
            store: StoreKind::Memory,
            persistence_dir: PathBuf::from("./data/rooms"),
            room_ttl_secs: None,
            max_room_bytes: 32 * 1024 * 1024,
            max_ws_message_bytes: 16 * 1024 * 1024,
            #[cfg(feature = "redis")]
            redis_url: None,
        }
    }
}

impl ServerConfig {
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Ok(host) = std::env::var("HOST") {
            cfg.host = host;
        }
        if let Ok(port) = std::env::var("PORT") {
            if let Ok(p) = port.parse() {
                cfg.port = p;
            }
        }
        if let Ok(store) = std::env::var("STORE") {
            cfg.store = match store.to_lowercase().as_str() {
                "file" => StoreKind::File,
                #[cfg(feature = "redis")]
                "redis" => StoreKind::Redis,
                _ => StoreKind::Memory,
            };
        }
        #[cfg(feature = "redis")]
        if let Ok(url) = std::env::var("REDIS_URL") {
            cfg.redis_url = Some(url);
            if matches!(cfg.store, StoreKind::Memory) {
                cfg.store = StoreKind::Redis;
            }
        }
        if let Ok(dir) = std::env::var("PERSISTENCE_DIR") {
            cfg.persistence_dir = PathBuf::from(dir);
        }
        if let Ok(ttl) = std::env::var("ROOM_TTL_SECS") {
            cfg.room_ttl_secs = ttl.parse().ok();
        }
        if let Ok(max) = std::env::var("MAX_ROOM_BYTES") {
            if let Ok(n) = max.parse() {
                cfg.max_room_bytes = n;
            }
        }
        if let Ok(max) = std::env::var("MAX_WS_MESSAGE_BYTES") {
            if let Ok(n) = max.parse() {
                cfg.max_ws_message_bytes = n;
            }
        }
        cfg
    }

    pub fn socket_addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.port)
            .parse()
            .expect("invalid HOST/PORT")
    }

    pub async fn build_store(&self) -> Arc<dyn RoomStore> {
        match self.store {
            StoreKind::Memory => Arc::new(MemoryRoomStore::new()),
            StoreKind::File => Arc::new(FileRoomStore::new(
                self.persistence_dir.clone(),
                self.room_ttl_secs,
                self.max_room_bytes,
            )),
            #[cfg(feature = "redis")]
            StoreKind::Redis => {
                let url = self
                    .redis_url
                    .clone()
                    .unwrap_or_else(|| "redis://127.0.0.1:6379".to_string());
                Arc::new(
                    RedisRoomStore::connect(&url, self.max_room_bytes)
                        .await
                        .expect("REDIS_URL connection failed"),
                )
            }
        }
    }
}
