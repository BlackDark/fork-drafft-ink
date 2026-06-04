use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;

use super::RoomStore;

pub struct MemoryRoomStore {
    rooms: RwLock<HashMap<String, String>>,
}

impl MemoryRoomStore {
    pub fn new() -> Self {
        Self {
            rooms: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl RoomStore for MemoryRoomStore {
    async fn load_snapshot(&self, room_id: &str) -> Option<String> {
        self.rooms.read().ok()?.get(room_id).cloned()
    }

    async fn save_snapshot(&self, room_id: &str, data_b64: &str) -> bool {
        if let Ok(mut rooms) = self.rooms.write() {
            rooms.insert(room_id.to_string(), data_b64.to_string());
            true
        } else {
            false
        }
    }

    async fn is_ready(&self) -> bool {
        true
    }
}
