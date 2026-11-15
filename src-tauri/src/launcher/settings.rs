use super::net;
use super::paths::{self, Paths};
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// Recommended G1 garbage collector tuning for Minecraft.
pub const DEFAULT_JVM_ARGS: &str = "-XX:+UnlockExperimentalVMOptions -XX:+UseG1GC -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    // Appearance
    pub accent: String,
    /// "aurora" (colourful glow) | "contours" (topographic map) | "both"
    pub background: String,

    // Java & memory
    pub memory_mb: u32,
    pub metaspace_mb: u32,
    pub jvm_args: String,
    /// Use the Java runtime Mojang provides for each version (recommended).
    pub use_bundled_java: bool,
    /// Used when `use_bundled_java` is off.
    pub java_path: String,

    // Game window & launching
    pub window_width: u32,
    pub window_height: u32,
    pub fullscreen: bool,
    /// "open" | "minimize" | "hide" (hide, then come back when the game closes)
    pub after_launch: String,
    pub open_console: bool,

    // Commands
    pub enable_commands: bool,
    pub pre_launch_command: String,
    pub post_exit_command: String,
    pub wrapper_command: String,

    // Advanced (mostly Linux)
    pub gamemode: bool,
    pub use_system_glfw: bool,
    pub use_system_openal: bool,

    // Network
    pub concurrent_downloads: u32,
    pub connection_timeout: u32,
    pub proxy_enabled: bool,
    /// "http" | "socks5"
    pub proxy_type: String,
    pub proxy_host: String,
    pub proxy_port: u16,

    // Integrations
    /// Your own key from console.curseforge.com (overrides a built-in one).
    pub curseforge_api_key: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            accent: "glacier".into(),
            background: "aurora".into(),
            memory_mb: 4096,
            metaspace_mb: 256,
            jvm_args: DEFAULT_JVM_ARGS.into(),
            use_bundled_java: true,
            java_path: String::new(),
            window_width: 1280,
            window_height: 720,
            fullscreen: false,
            after_launch: "open".into(),
            open_console: true,
            enable_commands: false,
            pre_launch_command: String::new(),
            post_exit_command: String::new(),
            wrapper_command: String::new(),
            gamemode: false,
            use_system_glfw: false,
            use_system_openal: false,
            concurrent_downloads: 24,
            connection_timeout: 30,
            proxy_enabled: false,
            proxy_type: "http".into(),
            proxy_host: String::new(),
            proxy_port: 8080,
            curseforge_api_key: String::new(),
        }
    }
}

impl Settings {
    pub fn net_config(&self) -> net::NetConfig {
        let proxy = (self.proxy_enabled && !self.proxy_host.trim().is_empty())
            .then(|| format!("{}://{}:{}", self.proxy_type, self.proxy_host.trim(), self.proxy_port));
        net::NetConfig {
            timeout_secs: self.connection_timeout as u64,
            concurrency: self.concurrent_downloads as usize,
            proxy,
        }
    }
}

pub async fn load(paths: &Paths) -> Result<Settings> {
    Ok(paths::read_json(&paths.settings_file()).await?.unwrap_or_default())
}

pub async fn save(paths: &Paths, s: &Settings) -> Result<()> {
    net::configure(&s.net_config())?;
    paths::write_json(&paths.settings_file(), s).await
}
