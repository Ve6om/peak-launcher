//! Builds the Java command line for an installed version, applying global
//! settings and per-instance overrides.

use super::accounts::{Account, AccountKind};
use super::install::{self, library_artifact, log_config_path, natives_dir};
use super::instances::{self, Instance};
use super::java;
use super::paths::{self, Paths};
use super::rules;
use super::settings::Settings;
use super::versions::{Arg, ArgValue, AssetIndex, VersionJson};
use crate::error::{Error, Result};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub struct LaunchPlan {
    /// What actually gets executed (java, or a wrapper / gamemoderun).
    pub program: String,
    pub args: Vec<String>,
    pub java: PathBuf,
    pub game_dir: PathBuf,
    pub pre_launch: Option<String>,
    pub post_exit: Option<String>,
}

/// Options after combining global settings with the instance's overrides.
pub struct Effective {
    pub memory_mb: u32,
    pub metaspace_mb: u32,
    pub jvm_args: String,
    pub jvm_preset: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub commands: bool,
    pub pre_launch: String,
    pub post_exit: String,
    pub wrapper: String,
    pub system_glfw: bool,
    pub system_openal: bool,
}

pub fn effective(inst: &Instance, s: &Settings) -> Effective {
    let pick = |o: &Option<String>, g: &String| o.clone().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| g.clone());
    Effective {
        memory_mb: inst.memory_mb.unwrap_or(s.memory_mb).max(512),
        metaspace_mb: inst.metaspace_mb.unwrap_or(s.metaspace_mb),
        jvm_args: pick(&inst.jvm_args, &s.jvm_args),
        jvm_preset: inst.jvm_preset.clone().filter(|p| super::jvmflags::is_known(p)).unwrap_or_else(|| s.jvm_preset.clone()),
        width: inst.window_width.unwrap_or(s.window_width),
        height: inst.window_height.unwrap_or(s.window_height),
        fullscreen: inst.fullscreen.unwrap_or(s.fullscreen),
        commands: inst.enable_commands.unwrap_or(s.enable_commands),
        pre_launch: pick(&inst.pre_launch_command, &s.pre_launch_command),
        post_exit: pick(&inst.post_exit_command, &s.post_exit_command),
        wrapper: pick(&inst.wrapper_command, &s.wrapper_command),
        system_glfw: inst.use_system_glfw.unwrap_or(s.use_system_glfw),
        system_openal: inst.use_system_openal.unwrap_or(s.use_system_openal),
    }
}

/// Pick the Java binary: instance custom path > instance runtime override >
/// global custom path (when bundled Java is off) > the version's runtime.
pub async fn resolve_java(paths: &Paths, inst: &Instance, version: &VersionJson, s: &Settings) -> Result<PathBuf> {
    if let Some(p) = inst.java_path.as_ref().filter(|p| !p.trim().is_empty()) {
        let p = PathBuf::from(p.trim());
        return if p.exists() { Ok(p) } else { Err(Error::msg(format!("Java not found at {}", p.display()))) };
    }
    if let Some(component) = inst.java_runtime.as_ref().filter(|c| !c.trim().is_empty()) {
        return java::ensure_runtime(paths, component, false, |_| {}).await;
    }
    if !s.use_bundled_java && !s.java_path.trim().is_empty() {
        let p = PathBuf::from(s.java_path.trim());
        return if p.exists() { Ok(p) } else { Err(Error::msg(format!("Java not found at {}", p.display()))) };
    }
    let component = version.java_version.as_ref().map(|j| j.component.as_str()).unwrap_or(java::LEGACY_COMPONENT);
    java::ensure_runtime(paths, component, false, |_| {}).await
}

fn substitute(arg: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(arg.len());
    let mut rest = arg;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        match rest[start..].find('}') {
            Some(end) => {
                let key = &rest[start + 2..start + end];
                match vars.get(key) {
                    Some(v) => out.push_str(v),
                    None => out.push_str(&rest[start..start + end + 1]),
                }
                rest = &rest[start + end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

fn expand(args: &[Arg], features: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for a in args {
        match a {
            Arg::Plain(s) => out.push(s.clone()),
            Arg::Ruled { rules: r, value } => {
                if rules::allowed_with(Some(r), features) {
                    match value {
                        ArgValue::One(s) => out.push(s.clone()),
                        ArgValue::Many(v) => out.extend(v.iter().cloned()),
                    }
                }
            }
        }
    }
    out
}

fn mentions(args: &[Arg], needle: &str) -> bool {
    args.iter().any(|a| match a {
        Arg::Plain(s) => s.contains(needle),
        Arg::Ruled { value: ArgValue::One(s), .. } => s.contains(needle),
        Arg::Ruled { value: ArgValue::Many(v), .. } => v.iter().any(|s| s.contains(needle)),
    })
}

/// Where the game should read assets from, preparing legacy layouts if needed.
async fn assets_dir(paths: &Paths, version: &VersionJson, game_dir: &std::path::Path) -> Result<PathBuf> {
    let Some(ai) = &version.asset_index else { return Ok(paths.assets()) };
    let index_path = paths.assets().join("indexes").join(format!("{}.json", ai.id));
    let Some(index) = paths::read_json::<AssetIndex>(&index_path).await? else { return Ok(paths.assets()) };
    if index.map_to_resources {
        let dir = game_dir.join("resources");
        install::copy_virtual_assets(paths, &index, &dir).await?;
        Ok(dir)
    } else if index.is_virtual {
        Ok(paths.assets().join("virtual").join(&ai.id))
    } else {
        Ok(paths.assets())
    }
}

pub async fn plan(
    paths: &Paths,
    inst: &Instance,
    version: &VersionJson,
    account: &Account,
    settings: &Settings,
) -> Result<LaunchPlan> {
    let game_dir = instances::game_dir(paths, &inst.id);
    tokio::fs::create_dir_all(&game_dir).await?;
    let fx = effective(inst, settings);
    let java = resolve_java(paths, inst, version, settings).await?;

    // Classpath: every allowed library plus the client jar, without duplicates.
    // With system GLFW/OpenAL, Minecraft's bundled natives for those are skipped.
    let sep = if cfg!(windows) { ";" } else { ":" };
    let mut seen = HashSet::new();
    let mut cp = Vec::new();
    for lib in &version.libraries {
        let natives = lib.name.contains(":natives-");
        if natives && ((fx.system_glfw && lib.name.contains("glfw")) || (fx.system_openal && lib.name.contains("openal"))) {
            continue;
        }
        if let Some((path, _)) = library_artifact(paths, lib) {
            if seen.insert(path.clone()) {
                cp.push(path.to_string_lossy().into_owned());
            }
        }
    }
    cp.push(paths.version_jar(&version.id).to_string_lossy().into_owned());
    let classpath = cp.join(sep);

    let assets_root = assets_dir(paths, version, &game_dir).await?;
    let natives = natives_dir(paths, &inst.game_version);
    tokio::fs::create_dir_all(&natives).await?;

    let token = account.access_token.clone().unwrap_or_else(|| "0".into());
    let user_type = if account.kind == AccountKind::Microsoft { "msa" } else { "legacy" };
    let s = |p: &PathBuf| p.to_string_lossy().into_owned();

    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("auth_player_name", account.username.clone());
    vars.insert("version_name", version.id.clone());
    vars.insert("game_directory", s(&game_dir));
    vars.insert("assets_root", s(&assets_root));
    vars.insert("game_assets", s(&assets_root));
    vars.insert("assets_index_name", version.assets.clone().unwrap_or_else(|| inst.game_version.clone()));
    vars.insert("auth_uuid", account.id.clone());
    vars.insert("auth_access_token", token.clone());
    vars.insert("auth_session", format!("token:{token}:{}", account.id));
    vars.insert("auth_xuid", String::new());
    vars.insert("clientid", String::new());
    vars.insert("user_type", user_type.into());
    vars.insert("user_properties", "{}".into());
    vars.insert("version_type", version.kind.clone().unwrap_or_else(|| "release".into()));
    vars.insert("natives_directory", s(&natives));
    vars.insert("launcher_name", "PeakLauncher".into());
    vars.insert("launcher_version", env!("CARGO_PKG_VERSION").into());
    vars.insert("classpath", classpath);
    vars.insert("classpath_separator", sep.into());
    vars.insert("library_directory", s(&paths.libraries()));
    vars.insert("resolution_width", fx.width.to_string());
    vars.insert("resolution_height", fx.height.to_string());
    vars.insert("quickPlayPath", s(&game_dir.join("quickPlay").join("log.json")));

    // Launch features (custom resolution, quick play).
    let qp = &inst.quick_play;
    let qp_value = qp.value.trim().to_string();
    let mut features: Vec<&str> = Vec::new();
    if !fx.fullscreen {
        features.push("has_custom_resolution");
    }
    let game_args = version.arguments.as_ref().map(|a| a.game.as_slice()).unwrap_or(&[]);
    let modern_qp = mentions(game_args, "quickPlayMultiplayer");
    if !qp_value.is_empty() && modern_qp {
        match qp.mode.as_str() {
            "world" => {
                features.push("is_quick_play_singleplayer");
                vars.insert("quickPlaySingleplayer", qp_value.clone());
            }
            "server" => {
                features.push("is_quick_play_multiplayer");
                vars.insert("quickPlayMultiplayer", qp_value.clone());
            }
            "realm" => {
                features.push("is_quick_play_realms");
                vars.insert("quickPlayRealms", qp_value.clone());
            }
            _ => {}
        }
    }

    let mut args = vec![format!("-Xmx{}M", fx.memory_mb), "-Xms512M".to_string()];
    if fx.metaspace_mb > 0 {
        args.push(format!("-XX:MetaspaceSize={}M", fx.metaspace_mb));
    }
    // The preset's flags for the Java that will actually run, then the player's own.
    args.extend(super::jvmflags::flags_for(&fx.jvm_preset, super::jvmflags::java_major(&java)));
    args.extend(fx.jvm_args.split_whitespace().map(String::from));

    if let Some(log) = version.logging.as_ref().and_then(|l| l.client.as_ref()) {
        let path = log_config_path(paths, &log.file.id);
        if path.exists() {
            args.push(log.argument.replace("${path}", &path.to_string_lossy()));
        }
    }

    let legacy = version.arguments.is_none();
    let (jvm, mut game) = match &version.arguments {
        Some(a) => (expand(&a.jvm, &features), expand(&a.game, &features)),
        None => (
            vec!["-Djava.library.path=${natives_directory}".into(), "-cp".into(), "${classpath}".into()],
            version.minecraft_arguments.clone().unwrap_or_default().split_whitespace().map(String::from).collect(),
        ),
    };
    if legacy && !fx.fullscreen {
        game.extend(["--width".into(), "${resolution_width}".into(), "--height".into(), "${resolution_height}".into()]);
    }
    if fx.fullscreen {
        game.push("--fullscreen".into());
    }
    // Older versions can still join a server directly.
    if qp.mode == "server" && !qp_value.is_empty() && !modern_qp {
        let (host, port) = qp_value.split_once(':').unwrap_or((&qp_value, "25565"));
        game.extend(["--server".into(), host.to_string(), "--port".into(), port.to_string()]);
    }

    args.extend(jvm.iter().map(|a| substitute(a, &vars)));
    args.push(version.main_class.clone());
    args.extend(game.iter().map(|a| substitute(a, &vars)));

    // Prefix with a wrapper command and/or Feral GameMode (Linux).
    let mut prefix: Vec<String> = Vec::new();
    if fx.commands {
        prefix.extend(fx.wrapper.split_whitespace().map(String::from));
    }
    if cfg!(target_os = "linux") && settings.gamemode {
        prefix.insert(0, "gamemoderun".into());
    }
    let (program, args) = if prefix.is_empty() {
        (s(&java), args)
    } else {
        let program = prefix.remove(0);
        prefix.push(s(&java));
        prefix.extend(args);
        (program, prefix)
    };

    let cmd = |c: &str| (fx.commands && !c.trim().is_empty()).then(|| c.trim().to_string());
    Ok(LaunchPlan {
        program,
        args,
        java,
        game_dir,
        pre_launch: cmd(&fx.pre_launch),
        post_exit: cmd(&fx.post_exit),
    })
}

/// The command line with the access token hidden, for logs.
pub fn redacted(plan: &LaunchPlan, token: Option<&str>) -> String {
    let mut line = format!("{} {}", plan.program, plan.args.join(" "));
    if let Some(t) = token.filter(|t| t.len() > 4) {
        line = line.replace(t, "<hidden>");
    }
    line
}

/// Run a user launch command through the system shell with instance details
/// available as $INST_NAME, $INST_ID, $INST_DIR, $INST_MC_DIR and $INST_JAVA.
pub fn shell_command(command: &str, inst: &Instance, plan: &LaunchPlan, paths: &Paths) -> tokio::process::Command {
    let env = [
        ("INST_NAME", inst.name.clone()),
        ("INST_ID", inst.id.clone()),
        ("INST_DIR", paths.instance(&inst.id).to_string_lossy().into_owned()),
        ("INST_MC_DIR", plan.game_dir.to_string_lossy().into_owned()),
        ("INST_JAVA", plan.java.to_string_lossy().into_owned()),
    ];
    let mut expanded = command.to_string();
    for (k, v) in &env {
        expanded = expanded.replace(&format!("${k}"), v);
    }
    let mut cmd = if cfg!(windows) {
        let mut c = tokio::process::Command::new("cmd");
        c.arg("/C").arg(&expanded);
        c
    } else {
        let mut c = tokio::process::Command::new("sh");
        c.arg("-c").arg(&expanded);
        c
    };
    cmd.envs(env).current_dir(&plan.game_dir);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    cmd
}
