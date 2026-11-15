use super::paths::{self, Paths};
use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub game_version: String,
    #[serde(default = "vanilla")]
    pub loader: String,
    #[serde(default)]
    pub loader_version: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub last_played: Option<DateTime<Utc>>,
    #[serde(default)]
    pub playtime_secs: u64,
    /// Overrides the global memory setting when set.
    #[serde(default)]
    pub memory_mb: Option<u32>,
    #[serde(default)]
    pub jvm_args: Option<String>,
    #[serde(default)]
    pub installed: bool,
    /// Version profile used to launch (differs from game_version with a loader).
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub notes: String,

    // ---- per-instance overrides (None = use the global setting) ----
    #[serde(default)]
    pub metaspace_mb: Option<u32>,
    /// Custom java executable.
    #[serde(default)]
    pub java_path: Option<String>,
    /// Mojang Java runtime to use instead of the one the version asks for.
    #[serde(default)]
    pub java_runtime: Option<String>,
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default)]
    pub quick_play: QuickPlay,
    #[serde(default)]
    pub window_width: Option<u32>,
    #[serde(default)]
    pub window_height: Option<u32>,
    #[serde(default)]
    pub fullscreen: Option<bool>,
    #[serde(default)]
    pub enable_commands: Option<bool>,
    #[serde(default)]
    pub pre_launch_command: Option<String>,
    #[serde(default)]
    pub post_exit_command: Option<String>,
    #[serde(default)]
    pub wrapper_command: Option<String>,
    #[serde(default)]
    pub use_system_glfw: Option<bool>,
    #[serde(default)]
    pub use_system_openal: Option<bool>,
    /// Set when the instance was created from a modpack.
    #[serde(default)]
    pub pack: Option<PackInfo>,
    /// Folder in another launcher this instance was copied from.
    #[serde(default)]
    pub imported_from: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackInfo {
    pub name: String,
    #[serde(default)]
    pub version_number: String,
    /// Modrinth ids (missing for packs imported from a file).
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub icon_url: Option<String>,
    /// Wide artwork for cards and the instance banner.
    #[serde(default)]
    pub banner_url: Option<String>,
    /// Short description of the pack.
    #[serde(default)]
    pub description: Option<String>,
}

/// Join a world, server or realm straight from the launcher.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickPlay {
    /// "none" | "world" | "server" | "realm"
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub value: String,
}

fn vanilla() -> String {
    "vanilla".into()
}

const FILE: &str = "instance.json";

pub fn game_dir(paths: &Paths, id: &str) -> PathBuf {
    paths.instance(id).join("minecraft")
}

fn slug(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "instance".into() } else { s }
}

pub async fn list(paths: &Paths) -> Result<Vec<Instance>> {
    let dir = paths.instances();
    let mut out = Vec::new();
    let Ok(mut rd) = tokio::fs::read_dir(&dir).await else { return Ok(out) };
    while let Some(entry) = rd.next_entry().await? {
        if let Ok(Some(inst)) = paths::read_json::<Instance>(&entry.path().join(FILE)).await {
            out.push(inst);
        }
    }
    // Most recently played first, then newest created.
    out.sort_by(|a, b| b.last_played.cmp(&a.last_played).then(b.created_at.cmp(&a.created_at)));
    Ok(out)
}

pub async fn get(paths: &Paths, id: &str) -> Result<Instance> {
    paths::read_json(&paths.instance(id).join(FILE))
        .await?
        .ok_or_else(|| Error::msg(format!("Instance '{id}' not found")))
}

pub async fn save(paths: &Paths, inst: &Instance) -> Result<()> {
    paths::write_json(&paths.instance(&inst.id).join(FILE), inst).await
}

pub async fn create(paths: &Paths, name: &str, game_version: &str, loader: &str, loader_version: Option<String>) -> Result<Instance> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::msg("Instance name can't be empty"));
    }
    let base = slug(name);
    let mut id = base.clone();
    let mut n = 2;
    while paths.instance(&id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    let inst = Instance {
        id: id.clone(),
        name: name.to_string(),
        game_version: game_version.to_string(),
        loader: loader.to_string(),
        loader_version: if loader == "vanilla" { None } else { loader_version },
        created_at: Utc::now(),
        last_played: None,
        playtime_secs: 0,
        memory_mb: None,
        jvm_args: None,
        installed: false,
        version_id: None,
        notes: String::new(),
        metaspace_mb: None,
        java_path: None,
        java_runtime: None,
        account_id: None,
        quick_play: QuickPlay::default(),
        window_width: None,
        window_height: None,
        fullscreen: None,
        enable_commands: None,
        pre_launch_command: None,
        post_exit_command: None,
        wrapper_command: None,
        use_system_glfw: None,
        use_system_openal: None,
        pack: None,
        imported_from: None,
    };
    tokio::fs::create_dir_all(game_dir(paths, &id)).await?;
    save(paths, &inst).await?;
    Ok(inst)
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}

/// Copy an instance (worlds, mods, settings) under a new name. Game files are
/// shared, so only the instance folder is copied.
pub async fn duplicate(paths: &Paths, id: &str) -> Result<Instance> {
    let src = get(paths, id).await?;
    let name = format!("{} (copy)", src.name);
    let base = slug(&name);
    let mut new_id = base.clone();
    let mut n = 2;
    while paths.instance(&new_id).exists() {
        new_id = format!("{base}-{n}");
        n += 1;
    }
    let (from, to) = (paths.instance(id), paths.instance(&new_id));
    tokio::task::spawn_blocking(move || copy_dir(&from, &to))
        .await
        .map_err(|e| Error::msg(e.to_string()))??;
    let mut inst = src;
    inst.id = new_id;
    inst.name = name;
    inst.created_at = Utc::now();
    inst.last_played = None;
    inst.playtime_secs = 0;
    save(paths, &inst).await?;
    Ok(inst)
}

pub async fn delete(paths: &Paths, id: &str) -> Result<()> {
    let dir = paths.instance(id);
    // Guard against path tricks: only delete direct children of instances/.
    if dir.parent() != Some(paths.instances().as_path()) || id.contains("..") || id.is_empty() {
        return Err(Error::msg("Invalid instance id"));
    }
    if dir.exists() {
        tokio::fs::remove_dir_all(dir).await?;
    }
    Ok(())
}

/// Copy the user-editable fields from `edit` onto `current`. Changing the game
/// version or loader marks the instance for reinstall on next launch.
pub fn apply_edit(current: &mut Instance, edit: Instance) {
    let blank = |s: Option<String>| s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    if current.game_version != edit.game_version || current.loader != edit.loader || current.loader_version != edit.loader_version {
        current.installed = false;
        current.version_id = None;
    }
    current.name = edit.name.trim().to_string();
    current.game_version = edit.game_version;
    current.loader = edit.loader;
    current.loader_version = if current.loader == "vanilla" { None } else { edit.loader_version };
    current.notes = edit.notes;
    current.memory_mb = edit.memory_mb;
    current.metaspace_mb = edit.metaspace_mb;
    current.jvm_args = blank(edit.jvm_args);
    current.java_path = blank(edit.java_path);
    current.java_runtime = blank(edit.java_runtime);
    current.account_id = blank(edit.account_id);
    current.quick_play = edit.quick_play;
    current.window_width = edit.window_width;
    current.window_height = edit.window_height;
    current.fullscreen = edit.fullscreen;
    current.enable_commands = edit.enable_commands;
    current.pre_launch_command = blank(edit.pre_launch_command);
    current.post_exit_command = blank(edit.post_exit_command);
    current.wrapper_command = blank(edit.wrapper_command);
    current.use_system_glfw = edit.use_system_glfw;
    current.use_system_openal = edit.use_system_openal;
}
