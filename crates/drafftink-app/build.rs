fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn main() {
    println!(
        "cargo:rustc-env=DRAFFTINK_DEFAULT_WS={}",
        env_or("DRAFFTINK_DEFAULT_WS", "/ws")
    );
    println!(
        "cargo:rustc-env=DRAFFTINK_HIDE_SERVER_URL={}",
        env_or("DRAFFTINK_HIDE_SERVER_URL", "false")
    );
    println!(
        "cargo:rustc-env=DRAFFTINK_LOCK_SERVER_URL={}",
        env_or("DRAFFTINK_LOCK_SERVER_URL", "false")
    );
    println!(
        "cargo:rustc-env=DRAFFTINK_AUTO_CONNECT={}",
        env_or("DRAFFTINK_AUTO_CONNECT", "false")
    );
    println!("cargo:rerun-if-env-changed=DRAFFTINK_DEFAULT_WS");
    println!("cargo:rerun-if-env-changed=DRAFFTINK_HIDE_SERVER_URL");
    println!("cargo:rerun-if-env-changed=DRAFFTINK_LOCK_SERVER_URL");
    println!("cargo:rerun-if-env-changed=DRAFFTINK_AUTO_CONNECT");
}
