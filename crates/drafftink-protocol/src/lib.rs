//! Collaboration relay protocol (JSON over WebSocket).

use serde::{Deserialize, Serialize};

/// Messages sent to the server
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Join a room
    Join { room: String },
    /// Leave current room
    Leave,
    /// Incremental Loro update (base64)
    Sync { data: String },
    /// Full Loro snapshot (base64) — join bootstrap / compaction
    SyncSnapshot { data: String },
    /// Request the server's persisted room snapshot
    RequestSnapshot,
    /// Awareness update (cursor position, selection, etc.)
    Awareness {
        peer_id: u64,
        #[serde(flatten)]
        state: AwarenessState,
    },
}

/// Messages received from the server
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Confirm room join with current state
    Joined {
        room: String,
        peer_count: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        initial_sync: Option<String>,
    },
    PeerJoined { peer_id: String },
    PeerLeft { peer_id: String },
    /// Incremental or snapshot sync from another peer (Loro bytes, base64)
    Sync { from: String, data: String },
    /// Full room snapshot from server (recovery / request_snapshot)
    RoomSnapshot {
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<String>,
    },
    Awareness {
        from: String,
        peer_id: u64,
        #[serde(flatten)]
        state: AwarenessState,
    },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct AwarenessState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<CursorPosition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<UserInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CursorPosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserInfo {
    pub name: String,
    pub color: String,
}

pub fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

pub fn base64_decode(input: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(input.trim()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_roundtrip() {
        let data = b"Hello, Loro!";
        let enc = base64_encode(data);
        assert_eq!(base64_decode(&enc).as_deref(), Some(data.as_slice()));
    }

    #[test]
    fn client_sync_snapshot_serialize() {
        let msg = ClientMessage::SyncSnapshot {
            data: "YWJj".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("sync_snapshot"));
    }

    #[test]
    fn server_room_snapshot_deserialize() {
        let json = r#"{"type":"room_snapshot","data":"abc"}"#;
        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ServerMessage::RoomSnapshot { .. }));
    }

    #[test]
    fn request_snapshot_roundtrip() {
        let msg = ClientMessage::RequestSnapshot;
        let json = serde_json::to_string(&msg).unwrap();
        let back: ClientMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ClientMessage::RequestSnapshot);
    }
}
