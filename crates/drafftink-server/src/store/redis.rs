//! Redis-backed room storage and cross-replica pub/sub.

use std::sync::Arc;

use async_trait::async_trait;
use redis::AsyncCommands;
use tracing::warn;

use super::RoomStore;

const SNAPSHOT_KEY_PREFIX: &str = "drafftink:room:";
const SNAPSHOT_SUFFIX: &str = ":snapshot";
const EVENTS_SUFFIX: &str = ":events";

pub struct RedisRoomStore {
    client: redis::aio::ConnectionManager,
    redis_url: String,
    max_room_bytes: usize,
}

impl RedisRoomStore {
    pub async fn connect(redis_url: &str, max_room_bytes: usize) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(redis_url)?;
        let manager = client.get_connection_manager().await?;
        Ok(Self {
            client: manager,
            redis_url: redis_url.to_string(),
            max_room_bytes,
        })
    }

    pub fn redis_url(&self) -> &str {
        &self.redis_url
    }

    fn snapshot_key(room_id: &str) -> String {
        format!("{SNAPSHOT_KEY_PREFIX}{room_id}{SNAPSHOT_SUFFIX}")
    }

    pub fn events_channel(room_id: &str) -> String {
        format!("{SNAPSHOT_KEY_PREFIX}{room_id}{EVENTS_SUFFIX}")
    }

    pub async fn publish_event(
        &self,
        room_id: &str,
        payload: &str,
    ) -> Result<(), redis::RedisError> {
        let mut conn = self.client.clone();
        let _: () = conn
            .publish(Self::events_channel(room_id), payload)
            .await?;
        Ok(())
    }

    pub fn connection(&self) -> redis::aio::ConnectionManager {
        self.client.clone()
    }
}

#[async_trait]
impl RoomStore for RedisRoomStore {
    async fn load_snapshot(&self, room_id: &str) -> Option<String> {
        let mut conn = self.client.clone();
        let key = Self::snapshot_key(room_id);
        match conn.get::<_, Option<String>>(key).await {
            Ok(v) => v,
            Err(e) => {
                warn!("Redis load_snapshot {}: {}", room_id, e);
                None
            }
        }
    }

    async fn save_snapshot(&self, room_id: &str, data_b64: &str) -> bool {
        if data_b64.len() > self.max_room_bytes {
            warn!("Refusing Redis save for {}: too large", room_id);
            return false;
        }
        let mut conn = self.client.clone();
        let key = Self::snapshot_key(room_id);
        match conn.set::<_, _, ()>(key, data_b64).await {
            Ok(()) => true,
            Err(e) => {
                warn!("Redis save_snapshot {}: {}", room_id, e);
                false
            }
        }
    }

    async fn is_ready(&self) -> bool {
        let mut conn = self.client.clone();
        redis::cmd("PING")
            .query_async::<()>(&mut conn)
            .await
            .is_ok()
    }
}

pub type SharedRedisStore = Arc<RedisRoomStore>;
