//! WebAssembly entry point and platform-specific code.

use wasm_bindgen::prelude::*;

pub use crate::share_url::{UrlParams, parse_params};

/// Parse URL parameters from the current page location.
pub fn get_url_params() -> UrlParams {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return UrlParams::default(),
    };
    let location = window.location();

    let mut params = UrlParams::default();

    if let Ok(search) = location.search() {
        let p = parse_params(&search);
        if params.room.is_none() {
            params.room = p.room;
        }
        if params.server.is_none() {
            params.server = p.server;
        }
    }

    if let Ok(hash) = location.hash() {
        let p = parse_params(&hash);
        if params.room.is_none() {
            params.room = p.room;
        }
        if params.server.is_none() {
            params.server = p.server;
        }
    }

    params
}

/// Legacy function for backward compatibility.
pub fn get_room_from_url() -> Option<String> {
    get_url_params().room
}

/// Get the WebSocket server URL.
pub fn get_server_url(server_param: Option<&str>) -> Option<String> {
    if let Some(server) = server_param {
        let server = server.trim();
        if !crate::share_url::is_valid_server_query_param(server) {
            // Ignore broken ?server= values (e.g. truncated `ws` from unencoded ws://…)
        } else if server.starts_with("ws://") || server.starts_with("wss://") {
            if server.ends_with("/ws") {
                return Some(server.to_string());
            }
            return Some(format!("{}/ws", server.trim_end_matches('/')));
        } else {
            return Some(format!("ws://{}/ws", server.trim_end_matches('/')));
        }
    }

    let window = web_sys::window()?;
    let location = window.location();
    let protocol = location.protocol().ok()?;
    let host = location.host().ok()?;
    let ws_protocol = if protocol == "https:" { "wss:" } else { "ws:" };
    Some(format!("{}//{}/ws", ws_protocol, host))
}

/// Update the browser URL with room (and optionally server) for sharing.
pub fn set_share_url(room: &str, server: Option<&str>, include_server: bool) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(history) = window.history() else {
        return;
    };
    let query = crate::share_url::build_share_query(room, server, include_server);
    let path = window
        .location()
        .pathname()
        .unwrap_or_else(|_| "/".to_string());
    let new_url = format!("{path}{query}");
    let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&new_url));
}

const DISPLAY_NAME_KEY: &str = "drafftink_display_name";

pub fn load_display_name() -> Option<String> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok()??;
    storage.get_item(DISPLAY_NAME_KEY).ok()?
}

pub fn save_display_name(name: &str) {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            let _ = storage.set_item(DISPLAY_NAME_KEY, name);
        }
    }
}

/// Initialize and run the WASM application.
#[wasm_bindgen(start)]
pub async fn run_wasm() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).expect("Failed to initialize logger");

    log::info!("Starting DrafftInk (WASM)");

    let params = get_url_params();
    if let Some(ref room) = params.room {
        log::info!("Room from URL: {}", room);
    }

    crate::App::run().await;
}
