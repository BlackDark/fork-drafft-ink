//! URL helpers for collaboration share links (platform-agnostic).

#![allow(dead_code)]

/// Parsed collaboration URL parameters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UrlParams {
    pub room: Option<String>,
    pub server: Option<String>,
}

/// Parse `?room=` and `?server=` from a query or hash fragment.
pub fn parse_params(s: &str) -> UrlParams {
    let s = s.trim_start_matches(|c| c == '?' || c == '#');
    let mut room = None;
    let mut server = None;

    for pair in s.split('&') {
        let mut parts = pair.splitn(2, '=');
        let Some(key) = parts.next() else { continue };
        let Some(raw_value) = parts.next() else { continue };
        if raw_value.is_empty() {
            continue;
        }
        let value = percent_decode(raw_value);
        match key {
            "room" => room = Some(value),
            "server" => server = Some(value),
            _ => {}
        }
    }

    UrlParams { room, server }
}

fn percent_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                16,
            ) {
                out.push(hex as char);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Whether `?server=` is usable (filters broken values like `ws` from unencoded URLs).
pub fn is_valid_server_query_param(param: &str) -> bool {
    let p = param.trim();
    if p.is_empty() || p == "ws" || p == "/ws" {
        return false;
    }
    if p.contains("://") {
        return false;
    }
    p.len() >= 2
}

/// Host[:port] for `?server=` from a WebSocket URL (relative `/ws` → same-origin, omit param).
pub fn ws_url_to_share_server(ws_url: &str) -> Option<String> {
    let ws_url = ws_url.trim();
    if ws_url.is_empty() || ws_url == "/ws" || ws_url.starts_with('/') {
        return None;
    }
    let host = ws_url
        .strip_prefix("wss://")
        .or_else(|| ws_url.strip_prefix("ws://"))
        .unwrap_or(ws_url);
    let host = host.trim_end_matches('/').trim_end_matches("ws").trim_end_matches('/');
    if host.is_empty() || host == "ws" {
        return None;
    }
    Some(host.to_string())
}

/// Build a share path+query string for the current page.
pub fn build_share_query(room: &str, server: Option<&str>, include_server: bool) -> String {
    let room = url_encode(room);
    if include_server {
        if let Some(server) = server
            .filter(|s| !s.is_empty())
            .and_then(|s| ws_url_to_share_server(s).or_else(|| {
                if is_valid_server_query_param(s) {
                    Some(s.to_string())
                } else {
                    None
                }
            }))
        {
            return format!("?room={room}&server={}", url_encode(&server));
        }
    }
    format!("?room={room}")
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_query_room_and_server() {
        let p = parse_params("?room=abc&server=host%3A3030");
        assert_eq!(p.room.as_deref(), Some("abc"));
        assert_eq!(p.server.as_deref(), Some("host:3030"));
    }

    #[test]
    fn parse_hash_fragment() {
        let p = parse_params("#room=xyz");
        assert_eq!(p.room.as_deref(), Some("xyz"));
    }

    #[test]
    fn build_share_same_origin() {
        assert_eq!(build_share_query("room-1", None, false), "?room=room-1");
    }

    #[test]
    fn build_share_with_server() {
        assert_eq!(
            build_share_query("r", Some("relay.example:3030"), true),
            "?room=r&server=relay.example%3A3030"
        );
    }

    #[test]
    fn build_share_from_ws_url_uses_host_only() {
        assert_eq!(
            build_share_query("r", Some("ws://localhost:8080/ws"), true),
            "?room=r&server=localhost%3A8080"
        );
    }

    #[test]
    fn rejects_broken_server_param() {
        assert!(!is_valid_server_query_param("ws"));
    }
}
