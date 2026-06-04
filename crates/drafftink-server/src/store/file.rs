use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use tokio::fs;
use tracing::warn;

use super::RoomStore;

pub struct FileRoomStore {
    dir: PathBuf,
    room_ttl_secs: Option<u64>,
    max_room_bytes: usize,
}

impl FileRoomStore {
    pub fn new(dir: PathBuf, room_ttl_secs: Option<u64>, max_room_bytes: usize) -> Self {
        Self {
            dir,
            room_ttl_secs,
            max_room_bytes,
        }
    }

    fn room_path(&self, room_id: &str) -> Option<PathBuf> {
        let safe = sanitize_room_id(room_id)?;
        Some(self.dir.join(format!("{safe}.snapshot")))
    }
}

fn sanitize_room_id(room_id: &str) -> Option<String> {
    if room_id.is_empty() || room_id.len() > 128 {
        return None;
    }
    let safe: String = room_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if safe.is_empty() {
        None
    } else {
        Some(safe)
    }
}

async fn file_expired(path: &Path, ttl_secs: u64) -> bool {
    let Ok(meta) = fs::metadata(path).await else {
        return true;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return false;
    };
    let Ok(m) = modified.duration_since(UNIX_EPOCH) else {
        return false;
    };
    now.as_secs().saturating_sub(m.as_secs()) > ttl_secs
}

#[async_trait]
impl RoomStore for FileRoomStore {
    async fn load_snapshot(&self, room_id: &str) -> Option<String> {
        let path = self.room_path(room_id)?;
        if fs::metadata(&path).await.is_err() {
            return None;
        }
        if let Some(ttl) = self.room_ttl_secs {
            if file_expired(&path, ttl).await {
                let _ = fs::remove_file(&path).await;
                return None;
            }
        }
        let data = fs::read_to_string(&path).await.ok()?;
        if data.len() > self.max_room_bytes {
            warn!("Room {} snapshot exceeds max size on load", room_id);
            return None;
        }
        Some(data)
    }

    async fn save_snapshot(&self, room_id: &str, data_b64: &str) -> bool {
        if data_b64.len() > self.max_room_bytes {
            warn!("Refusing to save room {}: snapshot too large", room_id);
            return false;
        }
        let Some(path) = self.room_path(room_id) else {
            return false;
        };
        if let Err(e) = fs::create_dir_all(&self.dir).await {
            warn!("Failed to create persistence dir: {}", e);
            return false;
        }
        match fs::write(&path, data_b64).await {
            Ok(()) => true,
            Err(e) => {
                warn!("Failed to persist room {}: {}", room_id, e);
                false
            }
        }
    }

    async fn is_ready(&self) -> bool {
        fs::create_dir_all(&self.dir).await.is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_empty() {
        assert!(sanitize_room_id("").is_none());
    }

    #[test]
    fn sanitize_maps_special_chars() {
        assert_eq!(sanitize_room_id("a/b").as_deref(), Some("a_b"));
    }
}
