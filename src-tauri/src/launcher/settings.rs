use super::net;
use super::paths::{self, Paths};
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// Recommended G1 garbage collector tuning for Minecraft.
/// The flags Peak used by default before presets existed. Settings that still
/// have exactly these are moved to the "balanced" preset, which is the same set.
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
    /// Extra Java arguments of the player's own, added after the preset.
    pub jvm_args: String,
    /// Java flag preset: "none" (default), "balanced", "aikar" or "zgc".
    pub jvm_preset: String,
    /// Use the Java runtime Mojang provides for each version (recommended).
    pub use_bundled_java: bool,
    /// Used when `use_bundled_java` is off.
    pub java_path: String,
    /// Which Java build automatic Java uses: "mojang" (default), "temurin" or
    /// "graalvm". The right version is downloaded when a game needs it.
    pub java_vendor: String,
    /// The player has picked a Java build (asked once, the first time they play).
    pub java_vendor_chosen: bool,

    // Game window & launching
    pub window_width: u32,
    pub window_height: u32,
    pub fullscreen: bool,
    /// "open" | "minimize" | "hide" (hide, then come back when the game closes)
    pub after_launch: String,
    pub open_console: bool,
    /// Look for a new version of Peak on start-up and every few hours.
    pub auto_update_check: bool,
    /// The "add Peak to your applications menu" offer (AppImage) was dismissed.
    pub appimage_menu_dismissed: bool,
    /// Tell the player when a modpack they installed has a new version.
    pub modpack_update_check: bool,
    /// Show what the player is doing on their Discord profile.
    pub discord_presence: bool,
    /// Skip the blur and moving background glow, for slower PCs (and Linux's
    /// web view, where they're costly). Off unless the player turns it on.
    pub light_effects: bool,
    /// The aurora glow drifts slowly. Off by default on Linux, where WebKitGTK
    /// redraws the glow and every frosted panel over it on each frame.
    pub moving_glow: bool,

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
            jvm_args: String::new(),
            jvm_preset: "none".into(),
            use_bundled_java: true,
            java_path: String::new(),
            java_vendor: "mojang".into(),
            java_vendor_chosen: false,
            window_width: 1280,
            window_height: 720,
            fullscreen: false,
            after_launch: "open".into(),
            open_console: true,
            auto_update_check: true,
            appimage_menu_dismissed: false,
            modpack_update_check: true,
            discord_presence: true,
            light_effects: false,
            moving_glow: !cfg!(target_os = "linux"),
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
    let raw: Option<serde_json::Value> = paths::read_json(&paths.settings_file()).await?;
    let Some(raw) = raw else { return Ok(Settings::default()) };
    let had_preset = raw.get("jvmPreset").is_some();
    let mut s: Settings = serde_json::from_value(raw)?;
    migrate_jvm(&mut s, had_preset);
    Ok(s)
}

/// Settings from before presets: the old default flags become the matching
/// preset, so nothing changes for the player.
fn migrate_jvm(s: &mut Settings, had_preset: bool) {
    if !had_preset && s.jvm_args.trim() == DEFAULT_JVM_ARGS {
        s.jvm_args.clear();
        s.jvm_preset = "balanced".into();
    }
    if s.java_vendor != "mojang" && !super::distros::is_vendor(&s.java_vendor) {
        s.java_vendor = "mojang".into();
    }
    if !super::jvmflags::is_known(&s.jvm_preset) {
        s.jvm_preset = "none".into();
    }
}

#[cfg(test)]
mod jvm_migration_tests {
    use super::*;

    #[test]
    fn old_default_flags_become_the_balanced_preset() {
        let mut s = Settings { jvm_args: DEFAULT_JVM_ARGS.into(), jvm_preset: "none".into(), ..Settings::default() };
        migrate_jvm(&mut s, false);
        assert_eq!((s.jvm_args.as_str(), s.jvm_preset.as_str()), ("", "balanced"));
    }

    #[test]
    fn custom_flags_and_chosen_presets_are_left_alone() {
        let mut custom = Settings { jvm_args: "-XX:+UseG1GC -Dfoo=1".into(), jvm_preset: "none".into(), ..Settings::default() };
        migrate_jvm(&mut custom, false);
        assert_eq!((custom.jvm_args.as_str(), custom.jvm_preset.as_str()), ("-XX:+UseG1GC -Dfoo=1", "none"));
        let mut chosen = Settings { jvm_args: DEFAULT_JVM_ARGS.into(), jvm_preset: "none".into(), ..Settings::default() };
        migrate_jvm(&mut chosen, true);
        assert_eq!(chosen.jvm_preset, "none", "a preset the player picked stays");
        let mut odd = Settings { jvm_preset: "made-up".into(), ..Settings::default() };
        migrate_jvm(&mut odd, true);
        assert_eq!(odd.jvm_preset, "none");
    }

    #[test]
    fn new_installs_use_no_preset() {
        let s = Settings::default();
        assert_eq!((s.jvm_args.as_str(), s.jvm_preset.as_str()), ("", "none"));
    }
}

/// Everything back to the defaults, except what the player would have to go
/// and find again: their own CurseForge key.
pub fn defaults_keeping(current: &Settings) -> Settings {
    Settings {
        curseforge_api_key: current.curseforge_api_key.clone(),
        appimage_menu_dismissed: current.appimage_menu_dismissed,
        // Reset puts the build back to Mojang's without asking again.
        java_vendor_chosen: current.java_vendor_chosen,
        ..Settings::default()
    }
}

#[cfg(test)]
mod reset_tests {
    use super::*;

    #[test]
    fn reset_keeps_only_the_key() {
        let mine = Settings { memory_mb: 12000, accent: "ember".into(), curseforge_api_key: "abc".into(), ..Settings::default() };
        let reset = defaults_keeping(&mine);
        assert_eq!(reset.memory_mb, Settings::default().memory_mb);
        assert_eq!(reset.accent, "glacier");
        assert_eq!(reset.curseforge_api_key, "abc");
    }
}

pub async fn save(paths: &Paths, s: &Settings) -> Result<()> {
    net::configure(&s.net_config())?;
    paths::write_json(&paths.settings_file(), s).await
}
