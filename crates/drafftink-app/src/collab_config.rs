//! Deploy-time collaboration UI configuration.

/// Collaboration UI and connection defaults (build-time + optional runtime override on WASM).
#[derive(Debug, Clone)]
pub struct CollabConfig {
    pub default_server_url: String,
    pub hide_server_url: bool,
    pub lock_server_url: bool,
    pub auto_connect: bool,
}

impl Default for CollabConfig {
    fn default() -> Self {
        Self::from_build()
    }
}

impl CollabConfig {
    pub fn from_build() -> Self {
        Self {
            default_server_url: env!("DRAFFTINK_DEFAULT_WS").to_string(),
            hide_server_url: env_bool("DRAFFTINK_HIDE_SERVER_URL", false),
            lock_server_url: env_bool("DRAFFTINK_LOCK_SERVER_URL", false),
            auto_connect: env_bool("DRAFFTINK_AUTO_CONNECT", false),
        }
    }

    pub fn effective_server_url(&self, current: &str) -> String {
        if self.lock_server_url || current.is_empty() {
            self.default_server_url.clone()
        } else {
            current.to_string()
        }
    }

    pub fn show_server_url_field(&self) -> bool {
        !self.hide_server_url
    }

    #[cfg(target_arch = "wasm32")]
    pub fn apply_window_override(&mut self) {
        if let Some(js) = read_window_collab_config() {
            if let Some(url) = js.default_server_url {
                self.default_server_url = url;
            }
            if let Some(v) = js.hide_server_url {
                self.hide_server_url = v;
            }
            if let Some(v) = js.lock_server_url {
                self.lock_server_url = v;
            }
            if let Some(v) = js.auto_connect {
                self.auto_connect = v;
            }
        }
    }
}

fn env_bool(key: &str, default: bool) -> bool {
    let v = std::env::var(key).unwrap_or_default();
    match v.as_str() {
        "true" | "1" | "yes" => true,
        "false" | "0" | "no" => false,
        _ if v.is_empty() => default,
        _ => default,
    }
}

#[cfg(target_arch = "wasm32")]
struct WindowCollabJs {
    default_server_url: Option<String>,
    hide_server_url: Option<bool>,
    lock_server_url: Option<bool>,
    auto_connect: Option<bool>,
}

#[cfg(target_arch = "wasm32")]
fn read_window_collab_config() -> Option<WindowCollabJs> {
    use wasm_bindgen::JsValue;
    use wasm_bindgen::prelude::*;

    let window = web_sys::window()?;
    let key = JsValue::from_str("__DRAFFTINK_COLLAB__");
    let cfg = js_sys::Reflect::get(&window, &key).ok()?;
    if cfg.is_undefined() || cfg.is_null() {
        return None;
    }

    fn get_str(obj: &JsValue, field: &str) -> Option<String> {
        let v = js_sys::Reflect::get(obj, &JsValue::from_str(field)).ok()?;
        v.as_string()
    }
    fn get_bool(obj: &JsValue, field: &str) -> Option<bool> {
        js_sys::Reflect::get(obj, &JsValue::from_str(field))
            .ok()
            .and_then(|v| v.as_bool())
    }

    Some(WindowCollabJs {
        default_server_url: get_str(&cfg, "default_server_url"),
        hide_server_url: get_bool(&cfg, "hide_server_url"),
        lock_server_url: get_bool(&cfg, "lock_server_url"),
        auto_connect: get_bool(&cfg, "auto_connect"),
    })
}
