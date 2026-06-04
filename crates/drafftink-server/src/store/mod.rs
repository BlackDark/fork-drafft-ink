//! Durable room document storage.

mod file;
mod memory;

#[cfg(feature = "redis")]
mod redis;

pub use file::FileRoomStore;
pub use memory::MemoryRoomStore;

#[cfg(feature = "redis")]
pub use redis::RedisRoomStore;

use async_trait::async_trait;

#[async_trait]
pub trait RoomStore: Send + Sync {
    /// Load persisted snapshot (base64 Loro bytes) for a room.
    async fn load_snapshot(&self, room_id: &str) -> Option<String>;

    /// Persist snapshot (base64). Returns false if over size limit.
    async fn save_snapshot(&self, room_id: &str, data_b64: &str) -> bool;

    /// Check store is usable (for readiness).
    async fn is_ready(&self) -> bool;
}
