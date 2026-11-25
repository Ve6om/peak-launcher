mod error;
mod launcher;
mod links;
mod presence;
mod window;

use crate::error::{Error, Result};
use crate::launcher::accounts::{self, AccountInfo, LoginStart};
use crate::launcher::install::{self, InstallProgress, ProgressFn};
use crate::launcher::instances::{self, Instance, PackInfo};
use crate::launcher::java::{self, DetectedJava, RuntimeOption};
use crate::launcher::launch;
use crate::launcher::loaders::{self, Loader, LoaderVersion};
use crate::launcher::curseforge;
use crate::launcher::export;
use crate::launcher::serverpack;
use crate::launcher::servers;
use crate::launcher::ftb;
use crate::launcher::technic;
use crate::launcher::importer;
use crate::launcher::manual;
use crate::launcher::moddeps::{self, LinkedMod};
use crate::launcher::modpacks;
use crate::launcher::modrinth::{self, InstalledItem, Kind, Project, SearchResult, UpdateInfo};
use crate::launcher::net;
use crate::launcher::paths::Paths;
use crate::launcher::settings::{self, Settings};
use crate::launcher::versions::{self, VersionManifest};
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::oneshot;

pub struct AppState {
    paths: Paths,
    running: Mutex<HashMap<String, oneshot::Sender<()>>>,
    /// Process id of each running game, so Stop can still end it directly if
    /// the normal stop signal has nowhere to go.
    pids: Mutex<HashMap<String, u32>>,
    installing: Mutex<HashSet<String>>,
    /// Recent console output per instance, so a console window opened later
    /// can show what was already printed.
    logs: Mutex<HashMap<String, VecDeque<LogLine>>>,
}

const LOG_LIMIT: usize = 5000;

#[derive(Clone, Serialize)]
struct LogLine {
    line: String,
    stream: &'static str,
}

/// Record a console line and send it to every open window.
fn push_log(app: &AppHandle, id: &str, line: String, stream: &'static str) {
    {
        let state = app.state::<AppState>();
        let mut logs = state.logs.lock().unwrap();
        let list = logs.entry(id.to_string()).or_default();
        list.push_back(LogLine { line: line.clone(), stream });
        while list.len() > LOG_LIMIT {
            list.pop_front();
        }
    }
    let _ = app.emit("game-log", LogEvent { instance_id: id.to_string(), line, stream });
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallEvent {
    instance_id: String,
    #[serde(flatten)]
    progress: InstallProgress,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogEvent {
    instance_id: String,
    line: String,
    stream: &'static str,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StateEvent {
    instance_id: String,
    state: &'static str,
    exit_code: Option<i32>,
}

// ---------------- versions & instances ----------------

#[tauri::command]
async fn get_versions(state: State<'_, AppState>) -> Result<VersionManifest> {
    versions::manifest(&state.paths).await
}

#[tauri::command]
async fn loader_versions(loader: String, game_version: String) -> Result<Vec<LoaderVersion>> {
    loaders::versions(Loader::parse(&loader)?, &game_version).await
}

#[tauri::command]
async fn list_instances(state: State<'_, AppState>) -> Result<Vec<Instance>> {
    instances::list(&state.paths).await
}

#[tauri::command]
async fn create_instance(
    state: State<'_, AppState>,
    name: String,
    game_version: String,
    loader: String,
    loader_version: Option<String>,
) -> Result<Instance> {
    let kind = Loader::parse(&loader)?;
    if kind != Loader::Vanilla && loader_version.as_deref().unwrap_or("").is_empty() {
        return Err(Error::msg("Pick a loader version."));
    }
    instances::create(&state.paths, &name, &game_version, &loader, loader_version).await
}

#[tauri::command]
async fn update_instance(state: State<'_, AppState>, instance: Instance) -> Result<Instance> {
    if instance.name.trim().is_empty() {
        return Err(Error::msg("Instance name can't be empty"));
    }
    Loader::parse(&instance.loader)?;
    let mut current = instances::get(&state.paths, &instance.id).await?;
    let was = current.shared;
    instances::apply_edit(&mut current, instance);
    instances::save(&state.paths, &current).await?;
    // Sharing turned on: sync now so the packs show straight away. A running
    // game syncs when it closes instead.
    if current.shared != was && current.shared.any() && !state.running.lock().unwrap().contains_key(&current.id) {
        let game = instances::game_dir(&state.paths, &current.id);
        launcher::shared::turned_on(&state.paths, &game, &current.shared, &current.game_version)?;
    }
    Ok(current)
}

/// Set the instance's own logo or banner picture ("icon" or "banner"); no
/// path puts the modpack's artwork back.
#[tauri::command]
async fn instance_set_picture(state: State<'_, AppState>, id: String, kind: String, path: Option<String>) -> Result<Instance> {
    instances::set_picture(&state.paths, &id, &kind, path.as_deref().map(std::path::Path::new)).await
}

#[tauri::command]
async fn duplicate_instance(state: State<'_, AppState>, id: String) -> Result<Instance> {
    instances::duplicate(&state.paths, &id).await
}

#[tauri::command]
async fn delete_instance(state: State<'_, AppState>, id: String) -> Result<()> {
    if state.running.lock().unwrap().contains_key(&id) {
        return Err(Error::msg("Stop the game before deleting this instance."));
    }
    instances::delete(&state.paths, &id).await
}

#[tauri::command]
async fn open_instance_folder(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;
    let dir = instances::game_dir(&state.paths, &id);
    tokio::fs::create_dir_all(&dir).await?;
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| Error::msg(e.to_string()))
}

/// World folder names, for the quick play picker.
#[tauri::command]
async fn list_worlds(state: State<'_, AppState>, id: String) -> Result<Vec<String>> {
    let saves = instances::game_dir(&state.paths, &id).join("saves");
    let mut out = Vec::new();
    if let Ok(mut rd) = tokio::fs::read_dir(&saves).await {
        while let Some(e) = rd.next_entry().await? {
            if e.path().join("level.dat").exists() {
                out.push(e.file_name().to_string_lossy().into_owned());
            }
        }
    }
    out.sort();
    Ok(out)
}

async fn install_inner(app: &AppHandle, paths: &Paths, inst: &mut Instance) -> Result<()> {
    {
        let state = app.state::<AppState>();
        let mut installing = state.installing.lock().unwrap();
        if !installing.insert(inst.id.clone()) {
            return Err(Error::msg("This instance is already being installed."));
        }
    }
    let emitter = app.clone();
    let id = inst.id.clone();
    let progress: ProgressFn = Arc::new(move |p: InstallProgress| {
        let _ = emitter.emit("install-progress", InstallEvent { instance_id: id.clone(), progress: p });
    });

    let result: Result<String> = async {
        install::install_version(paths, &inst.game_version, progress.clone()).await?;
        let loader = Loader::parse(&inst.loader)?;
        let loader_version = inst.loader_version.clone().unwrap_or_default();
        loaders::install(paths, loader, &inst.game_version, &loader_version, &progress).await
    }
    .await;

    app.state::<AppState>().installing.lock().unwrap().remove(&inst.id);
    // Save "installed" before saying Done: the window re-reads the instance
    // when it hears Done, and must see it installed (e.g. to join a server).
    let saved = match result {
        Ok(version_id) => {
            inst.installed = true;
            inst.version_id = Some(version_id);
            instances::save(paths, inst).await
        }
        Err(e) => Err(e),
    };
    progress(InstallProgress { stage: "Done", done: 1, total: 1, bytes: 0 });
    saved
}

#[tauri::command]
async fn install_instance(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<Instance> {
    let paths = state.paths.clone();
    let mut inst = instances::get(&paths, &id).await?;
    install_inner(&app, &paths, &mut inst).await?;
    Ok(inst)
}

// ---------------- launching ----------------

#[tauri::command]
async fn launch_instance(app: AppHandle, state: State<'_, AppState>, id: String, server: Option<String>) -> Result<()> {
    if state.running.lock().unwrap().contains_key(&id) {
        return Err(Error::msg("This instance is already running."));
    }
    let paths = state.paths.clone();
    let mut inst = instances::get(&paths, &id).await?;
    let settings = settings::load(&paths).await?;

    // The instance's chosen account, else the active one.
    let store = accounts::load(&paths).await?;
    let find = |aid: &String| store.accounts.iter().find(|x| &x.id == aid);
    let account = inst
        .account_id
        .as_ref()
        .and_then(find)
        .or_else(|| store.active.as_ref().and_then(find))
        .or_else(|| store.accounts.first())
        .cloned()
        .ok_or_else(|| Error::msg("Add an account before playing."))?;
    let account = accounts::ensure_fresh(&paths, account).await?;

    let version_id = inst.version_id.clone().unwrap_or_else(|| inst.game_version.clone());
    if !inst.installed || !paths.version_json(&version_id).exists() {
        install_inner(&app, &paths, &mut inst).await?;
    }
    let version_id = inst.version_id.clone().unwrap_or_else(|| inst.game_version.clone());
    let version = versions::resolve(&paths, &version_id).await?;
    // Joining from the Servers tab: go straight into that server this time
    // only (the instance's own quick play setting is left as it was).
    let mut launch_inst = inst.clone();
    if let Some(address) = server.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
        servers::parse_address(address)?;
        launch_inst.quick_play = instances::QuickPlay { mode: "server".into(), value: address.to_string() };
    }
    // A Java build the player picked (Temurin / GraalVM) is downloaded here,
    // with progress showing, rather than quietly inside the launch.
    if let Some((vendor, major)) = launch::vendor_choice(&launch_inst, &version, &settings) {
        if launcher::distros::installed(&paths, &vendor, major).is_none() {
            let emitter = app.clone();
            let iid = id.clone();
            let send = move |stage: &'static str, p: launcher::net::Progress| {
                let progress = InstallProgress { stage, done: p.done, total: p.total, bytes: p.bytes };
                let _ = emitter.emit("install-progress", InstallEvent { instance_id: iid.clone(), progress });
            };
            let send2 = send.clone();
            // If it fails, the launch below plays with Mojang's Java instead.
            let _ = launcher::distros::ensure(&paths, &vendor, major, move |p| send2("Downloading Java", p)).await;
            send("Done", launcher::net::Progress { done: 1, total: 1, bytes: 0 });
        }
    }
    let plan = launch::plan(&paths, &launch_inst, &version, &account, &settings).await?;

    state.logs.lock().unwrap().remove(&id);
    let _ = app.emit("game-log-reset", serde_json::json!({ "instanceId": id }));
    let log = |line: String, stream: &'static str| push_log(&app, &id, line, stream);
    log(format!("[Peak] Launching {} ({} {})", inst.name, inst.game_version, inst.loader), "launcher");

    if let Some(pre) = &plan.pre_launch {
        log(format!("[Peak] Running pre-launch command: {pre}"), "launcher");
        let out = launch::shell_command(pre, &inst, &plan, &paths).output().await?;
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            log(line.to_string(), "stdout");
        }
        if !out.status.success() {
            return Err(Error::msg(format!(
                "Pre-launch command failed (exit {}). {}",
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
    }

    if let Err(e) = launcher::shared::before_launch(&paths, &plan.game_dir, &inst.shared, &inst.game_version) {
        log(format!("[Peak] Couldn't bring in the shared settings and packs: {e}"), "launcher");
    }
    log(format!("[Peak] {}", launch::redacted(&plan, account.access_token.as_deref())), "launcher");
    let mut cmd = tokio::process::Command::new(&plan.program);
    cmd.args(&plan.args)
        .current_dir(&plan.game_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let mut child = cmd.spawn().map_err(|e| Error::msg(format!("Couldn't start the game: {e}")))?;
    let pid = child.id();
    // Set by the log readers when the game prints that its window is up.
    let window_seen = Arc::new(std::sync::atomic::AtomicBool::new(false));

    for (pipe, stream) in [
        (child.stdout.take().map(|p| Box::new(p) as Box<dyn tokio::io::AsyncRead + Send + Unpin>), "stdout"),
        (child.stderr.take().map(|p| Box::new(p) as Box<dyn tokio::io::AsyncRead + Send + Unpin>), "stderr"),
    ] {
        if let Some(pipe) = pipe {
            let app = app.clone();
            let id = id.clone();
            let window_seen = window_seen.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(pipe).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if window::looks_like_window_opened(&line) {
                        window_seen.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    push_log(&app, &id, line, stream);
                }
            });
        }
    }

    // From here on the game is running: register it and start watching it
    // straight away, and don't return early, or it would keep running with
    // nothing able to stop it.
    let (kill_tx, kill_rx) = oneshot::channel();
    state.running.lock().unwrap().insert(id.clone(), kill_tx);
    if let Some(pid) = pid {
        state.pids.lock().unwrap().insert(id.clone(), pid);
    }
    let _ = app.emit("game-state", StateEvent { instance_id: id.clone(), state: "running", exit_code: None });

    window::after_launch(app.clone(), id.clone(), settings.after_launch.clone(), pid, window_seen);

    let started = std::time::Instant::now();
    inst.last_played = Some(chrono::Utc::now());
    if let Err(e) = instances::save(&paths, &inst).await {
        // Only the "last played" time is lost; the game itself is fine.
        eprintln!("Couldn't save last played time for {id}: {e}");
    }

    let app2 = app.clone();
    let after_launch = settings.after_launch.clone();
    tokio::spawn(async move {
        let code = tokio::select! {
            status = child.wait() => status.ok().and_then(|s| s.code()),
            _ = kill_rx => {
                let _ = child.kill().await;
                None
            }
        };
        app2.state::<AppState>().running.lock().unwrap().remove(&id);
        app2.state::<AppState>().pids.lock().unwrap().remove(&id);
        if let Ok(mut fresh) = instances::get(&paths, &id).await {
            fresh.playtime_secs += started.elapsed().as_secs();
            let _ = instances::save(&paths, &fresh).await;
            if let Err(e) = launcher::shared::after_exit(&paths, &plan.game_dir, &fresh.shared) {
                eprintln!("Couldn't save the shared settings and packs for {id}: {e}");
            }
        }
        if let Some(post) = &plan.post_exit {
            let _ = launch::shell_command(post, &inst, &plan, &paths).spawn();
        }
        window::after_exit(&app2, &after_launch);
        let _ = app2.emit("game-state", StateEvent { instance_id: id, state: "stopped", exit_code: code });
    });
    Ok(())
}

#[tauri::command]
fn stop_instance(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<()> {
    let tx = state.running.lock().unwrap().remove(&id);
    // Normally the task watching the game ends it and reports "stopped".
    if let Some(tx) = tx {
        if tx.send(()).is_ok() {
            return Ok(());
        }
    }
    // Nothing is listening for the stop signal: end the process directly.
    let pid = state.pids.lock().unwrap().remove(&id);
    if let Some(pid) = pid {
        kill_java(pid);
    }
    let _ = app.emit("game-state", StateEvent { instance_id: id, state: "stopped", exit_code: None });
    Ok(())
}

/// End a game process by id, but only if it is still a Java process (a
/// process id can be reused by an unrelated program once the game is gone).
fn kill_java(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessesToUpdate, System};
    let pid = Pid::from_u32(pid);
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    match sys.process(pid) {
        Some(p) if p.name().to_string_lossy().to_lowercase().starts_with("java") => p.kill(),
        _ => false,
    }
}

#[tauri::command]
fn game_logs(state: State<'_, AppState>, id: String) -> Vec<LogLine> {
    state.logs.lock().unwrap().get(&id).map(|l| l.iter().cloned().collect()).unwrap_or_default()
}

fn console_label(id: &str) -> String {
    format!("console-{}", id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect::<String>())
}

/// Close an instance's console window, putting the console back in the launcher.
#[tauri::command]
fn close_console_window(app: AppHandle, id: String) {
    if let Some(w) = app.get_webview_window(&console_label(&id)) {
        let _ = w.close();
    }
}

/// Open (or focus) an instance's console in its own window.
#[tauri::command]
async fn open_console_window(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<()> {
    let label = console_label(&id);
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }
    let name = instances::get(&state.paths, &id).await.map(|i| i.name).unwrap_or_else(|_| id.clone());
    let script = format!("window.__PEAK_CONSOLE__ = {};", serde_json::to_string(&id).unwrap_or_default());
    let window = tauri::WebviewWindowBuilder::new(&app, label, tauri::WebviewUrl::App("index.html".into()))
        .title(format!("{name} - Console"))
        .inner_size(920.0, 600.0)
        .min_inner_size(520.0, 320.0)
        .background_color(tauri::window::Color(7, 8, 13, 255))
        .initialization_script(&script)
        .build()
        .map_err(|e| Error::msg(format!("Couldn't open the console window: {e}")))?;
    // Let the launcher hide its own copy of the console while this is open.
    let _ = app.emit("console-window", serde_json::json!({ "instanceId": id, "open": true }));
    let (app2, id2) = (app.clone(), id.clone());
    window.on_window_event(move |e| {
        if matches!(e, tauri::WindowEvent::Destroyed) {
            let _ = app2.emit("console-window", serde_json::json!({ "instanceId": id2, "open": false }));
        }
    });
    Ok(())
}

#[tauri::command]
fn running_instances(state: State<'_, AppState>) -> Vec<String> {
    state.running.lock().unwrap().keys().cloned().collect()
}

// ---------------- mods & content (Modrinth + CurseForge) ----------------
//
// CurseForge ids start with "cf:"; everything else is Modrinth.

async fn cf_client(paths: &Paths) -> Result<curseforge::Client> {
    curseforge::Client::new(&settings::load(paths).await?.curseforge_api_key)
}

#[tauri::command]
async fn content_search(
    state: State<'_, AppState>,
    source: String,
    instance_id: String,
    kind: String,
    query: String,
    sort: String,
    side: Option<String>,
    offset: u32,
) -> Result<SearchResult> {
    let kind = Kind::parse(&kind)?;
    if source == "curseforge" {
        let cf = cf_client(&state.paths).await?;
        let inst = instances::get(&state.paths, &instance_id).await?;
        return curseforge::search(&cf, Some(&inst), Some(kind), &query, &sort, offset).await;
    }
    // CurseForge doesn't say which side a mod runs on, so this only filters Modrinth.
    modrinth::search(&state.paths, &instance_id, kind, &query, &sort, side.as_deref().unwrap_or("any"), offset).await
}

#[tauri::command]
async fn content_project(state: State<'_, AppState>, id: String) -> Result<Project> {
    if ftb::is_ftb(&id) {
        return ftb::project(&id).await;
    }
    if technic::is_technic(&id) {
        return technic::project(&id).await;
    }
    if curseforge::is_cf(&id) {
        return curseforge::project(&cf_client(&state.paths).await?, &id).await;
    }
    modrinth::project(&id).await
}

#[tauri::command]
async fn content_versions(state: State<'_, AppState>, instance_id: String, kind: String, project_id: String) -> Result<Vec<modrinth::Version>> {
    let inst = instances::get(&state.paths, &instance_id).await?;
    let kind = Kind::parse(&kind)?;
    if curseforge::is_cf(&project_id) {
        return curseforge::compatible_versions(&cf_client(&state.paths).await?, &inst, &project_id, kind).await;
    }
    modrinth::compatible_versions(&inst, &project_id, kind).await
}

#[tauri::command]
async fn content_install(
    state: State<'_, AppState>,
    instance_id: String,
    kind: String,
    project_id: String,
    version_id: Option<String>,
) -> Result<Vec<String>> {
    let kind = Kind::parse(&kind)?;
    if curseforge::is_cf(&project_id) {
        let cf = cf_client(&state.paths).await?;
        return curseforge::install(&cf, &state.paths, &instance_id, kind, &project_id, version_id.as_deref()).await;
    }
    modrinth::install(&state.paths, &instance_id, kind, &project_id, version_id.as_deref()).await
}

/// What installing a project involves (itself + missing required dependencies).
#[tauri::command]
async fn content_plan(
    state: State<'_, AppState>,
    instance_id: String,
    kind: String,
    project_id: String,
    version_id: Option<String>,
) -> Result<Vec<modrinth::PlanItem>> {
    let kind = Kind::parse(&kind)?;
    if curseforge::is_cf(&project_id) {
        let cf = cf_client(&state.paths).await?;
        return curseforge::plan(&cf, &state.paths, &instance_id, kind, &project_id, version_id.as_deref()).await;
    }
    modrinth::plan(&state.paths, &instance_id, kind, &project_id, version_id.as_deref()).await
}

/// Install exactly the items the user confirmed.
#[tauri::command]
async fn content_install_items(state: State<'_, AppState>, instance_id: String, items: Vec<modrinth::PlanItem>) -> Result<Vec<String>> {
    modrinth::install_items(&state.paths, &instance_id, &items).await
}

#[tauri::command]
async fn content_list(state: State<'_, AppState>, instance_id: String, kind: String) -> Result<Vec<InstalledItem>> {
    modrinth::list(&state.paths, &instance_id, Kind::parse(&kind)?).await
}

#[tauri::command]
async fn content_set_enabled(state: State<'_, AppState>, instance_id: String, kind: String, file_name: String, enabled: bool) -> Result<()> {
    modrinth::set_enabled(&state.paths, &instance_id, Kind::parse(&kind)?, &file_name, enabled).await
}

/// Mods affected when one is disabled, removed or enabled, so the user can
/// change them together.
#[tauri::command]
async fn content_linked(state: State<'_, AppState>, instance_id: String, file_name: String, action: String) -> Result<Vec<LinkedMod>> {
    moddeps::linked(&state.paths, &instance_id, &file_name, &action).await
}

#[tauri::command]
async fn content_remove(state: State<'_, AppState>, instance_id: String, kind: String, file_name: String) -> Result<()> {
    modrinth::remove(&state.paths, &instance_id, Kind::parse(&kind)?, &file_name).await
}

/// Update check across both sources. CurseForge is only asked when the
/// instance has CurseForge files and a key is available.
#[tauri::command]
async fn content_updates(state: State<'_, AppState>, instance_id: String, kind: String) -> Result<Vec<UpdateInfo>> {
    let kind = Kind::parse(&kind)?;
    let mut out = modrinth::check_updates(&state.paths, &instance_id, kind).await?;
    let has_cf = modrinth::list(&state.paths, &instance_id, kind)
        .await?
        .iter()
        .any(|i| i.project_id.as_deref().map(curseforge::is_cf).unwrap_or(false));
    if has_cf {
        if let Ok(cf) = cf_client(&state.paths).await {
            out.extend(curseforge::check_updates(&cf, &state.paths, &instance_id, kind).await?);
        }
    }
    Ok(out)
}

#[tauri::command]
async fn open_content_folder(app: AppHandle, state: State<'_, AppState>, instance_id: String, kind: String) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;
    let dir = modrinth::content_dir(&state.paths, &instance_id, Kind::parse(&kind)?);
    tokio::fs::create_dir_all(&dir).await?;
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| Error::msg(e.to_string()))
}

// ---------------- modpacks ----------------

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallError {
    instance_id: String,
    name: String,
    message: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallBlocked {
    instance_id: String,
    name: String,
    files: Vec<curseforge::Blocked>,
}

#[tauri::command]
async fn modpack_search(
    state: State<'_, AppState>,
    source: String,
    query: String,
    sort: String,
    offset: u32,
    category: Option<String>,
) -> Result<SearchResult> {
    if source == "curseforge" {
        return curseforge::search(&cf_client(&state.paths).await?, None, None, &query, &sort, offset).await;
    }
    if source == "ftb" {
        return ftb::search(&query, &sort, offset).await;
    }
    if source == "technic" {
        return technic::search(&query, &sort, offset, category.as_deref()).await;
    }
    modpacks::search(&query, &sort, offset).await
}

#[tauri::command]
async fn modpack_versions(state: State<'_, AppState>, project_id: String) -> Result<Vec<modrinth::Version>> {
    pack_versions(&state.paths, &project_id).await
}

async fn pack_versions(paths: &Paths, project_id: &str) -> Result<Vec<modrinth::Version>> {
    if ftb::is_ftb(project_id) {
        return ftb::versions(project_id).await;
    }
    if technic::is_technic(project_id) {
        return technic::versions(project_id).await;
    }
    if curseforge::is_cf(project_id) {
        return curseforge::modpack_versions(&cf_client(paths).await?, project_id).await;
    }
    modpacks::versions(project_id).await
}

/// Modpack instances with a newer version (marked when the player skipped it
/// or muted the pack, so the window can still show them as outdated). Empty
/// when update checks are off in Settings.
#[tauri::command]
async fn modpack_updates(state: State<'_, AppState>) -> Result<Vec<launcher::packupdate::PackUpdate>> {
    use futures::StreamExt;
    if !settings::load(&state.paths).await.map(|s| s.modpack_update_check).unwrap_or(true) {
        return Ok(Vec::new());
    }
    let paths = state.paths.clone();
    let packs: Vec<(Instance, PackInfo, String)> = instances::list(&paths)
        .await?
        .into_iter()
        .filter_map(|i| {
            let p = i.pack.clone()?;
            let pid = p.project_id.clone()?;
            p.version_id.is_some().then_some((i, p, pid))
        })
        .collect();
    let found: Vec<Option<launcher::packupdate::PackUpdate>> = futures::stream::iter(packs)
        .map(|(inst, pack, pid)| {
            let paths = paths.clone();
            async move {
                // A pack that can't be checked (offline, removed) is just skipped.
                let versions = pack_versions(&paths, &pid).await.ok()?;
                // Newest version regardless of a skip; the skip is reported instead.
                let probe = PackInfo { skipped_version: None, ..pack.clone() };
                let next = launcher::packupdate::newer(&versions, &probe)?;
                Some(launcher::packupdate::PackUpdate {
                    instance_id: inst.id.clone(),
                    instance_name: inst.name.clone(),
                    pack_name: pack.name.clone(),
                    current: pack.version_number.clone(),
                    latest_id: next.id.clone(),
                    latest: next.version_number.clone(),
                    icon_url: pack.icon_url.clone(),
                    skipped: pack.skipped_version.as_deref() == Some(next.id.as_str()),
                    muted: pack.no_update_prompts,
                })
            }
        })
        .buffer_unordered(4)
        .collect()
        .await;
    Ok(found.into_iter().flatten().collect())
}

/// Don't offer this update again (a later one still shows).
#[tauri::command]
async fn modpack_skip_update(state: State<'_, AppState>, id: String, version_id: String) -> Result<Instance> {
    let mut inst = instances::get(&state.paths, &id).await?;
    let pack = inst.pack.as_mut().ok_or_else(|| Error::msg("This instance isn't a modpack."))?;
    pack.skipped_version = Some(version_id);
    instances::save(&state.paths, &inst).await?;
    Ok(inst)
}

/// Turn update prompts for one modpack off or back on.
#[tauri::command]
async fn modpack_mute_updates(state: State<'_, AppState>, id: String, muted: bool) -> Result<Instance> {
    let mut inst = instances::get(&state.paths, &id).await?;
    let pack = inst.pack.as_mut().ok_or_else(|| Error::msg("This instance isn't a modpack."))?;
    pack.no_update_prompts = muted;
    instances::save(&state.paths, &inst).await?;
    Ok(inst)
}

/// Switch a modpack instance to another version of its pack, newer or older,
/// without making a new instance. Worlds, settings, screenshots and playtime
/// stay; the old mods folder is moved to a backup first (so mods the player
/// added aren't lost), then the chosen version installs in the background.
#[tauri::command]
async fn modpack_change_version(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    version_id: String,
    file_path: Option<String>,
    remove_file: Option<bool>,
) -> Result<Instance> {
    if state.running.lock().unwrap().contains_key(&id) {
        return Err(Error::msg("Close the game before changing the modpack version."));
    }
    if state.installing.lock().unwrap().contains(&id) {
        return Err(Error::msg("Wait for the current install to finish first."));
    }
    let paths = state.paths.clone();
    let mut inst = instances::get(&paths, &id).await?;
    let old = inst.pack.clone().ok_or_else(|| Error::msg("This instance isn't a modpack."))?;
    let pid = old
        .project_id
        .clone()
        .ok_or_else(|| Error::msg("This modpack was installed from a file, so Peak can't fetch other versions of it."))?;
    if old.version_id.as_deref() == Some(version_id.as_str()) {
        return Err(Error::msg("That version is already installed."));
    }
    let cf = cf_client(&paths).await.ok();

    // Read the chosen version the same way a new install does (into a
    // throwaway instance), then copy what it needs onto this one.
    let (temp, kind, archive) = if ftb::is_ftb(&pid) {
        let (manifest, info) = ftb::fetch(&pid, Some(&version_id)).await?;
        let t = ftb::prepare(&paths, &manifest, info).await?;
        (t, PackKind::Ftb(manifest), std::path::PathBuf::new())
    } else if technic::is_technic(&pid) {
        let (plan, info) = technic::fetch(&paths, &pid, Some(&version_id)).await?;
        let t = technic::prepare(&paths, &plan, info).await?;
        (t, PackKind::Technic(plan), std::path::PathBuf::new())
    } else if curseforge::is_cf(&pid) {
        let client = cf.as_ref().ok_or_else(|| Error::msg(NEED_CF_KEY))?;
        let (path, info) = match &file_path {
            // Downloaded by hand (the pack's author turned off launcher downloads).
            Some(f) => {
                let path = std::path::PathBuf::from(f);
                let info = curseforge::manual_pack_info(client, &pid, &version_id, &path).await?;
                (path, info)
            }
            None => curseforge::fetch_modpack(client, &paths, &pid, Some(&version_id)).await?,
        };
        let (t, manifest) = curseforge::prepare_modpack(&paths, &path, Some(info)).await?;
        (t, PackKind::CurseForge(manifest), path)
    } else {
        let (path, info) = modpacks::fetch(&paths, &pid, Some(&version_id)).await?;
        let (t, index) = modpacks::prepare(&paths, &path, Some(info)).await?;
        (t, PackKind::Modrinth(index), path)
    };
    let _ = instances::delete(&paths, &temp.id).await;

    let mut pack = temp.pack.clone().unwrap_or_default();
    pack.icon_url = pack.icon_url.or(old.icon_url.clone());
    pack.banner_url = pack.banner_url.or(old.banner_url.clone());
    pack.description = pack.description.or(old.description.clone());
    pack.no_update_prompts = old.no_update_prompts;
    pack.skipped_version = None;
    pack.previous_version_id = old.version_id.clone();
    pack.previous_version_number = Some(old.version_number.clone()).filter(|v| !v.is_empty());
    inst.game_version = temp.game_version.clone();
    inst.loader = temp.loader.clone();
    inst.loader_version = temp.loader_version.clone();
    inst.pack = Some(pack);
    inst.installed = false;
    inst.version_id = None;

    launcher::packupdate::back_up_mods(&paths, &id, &old.version_number).await?;
    instances::save(&paths, &inst).await?;
    let cleanup = downloaded_zip(&app, &file_path, remove_file);
    spawn_pack_install(app, paths, inst.clone(), archive, kind, cf, cleanup);
    Ok(inst)
}

enum PackKind {
    Modrinth(modpacks::Index),
    CurseForge(curseforge::Manifest),
    Ftb(ftb::Manifest),
    Technic(technic::Plan),
}

/// A pack zip Peak found in Downloads for the player, to tidy away once
/// it's installed (only ever a .zip directly in the Downloads folder).
fn downloaded_zip(app: &AppHandle, file_path: &Option<String>, remove_file: Option<bool>) -> Option<std::path::PathBuf> {
    match (file_path, remove_file) {
        (Some(f), Some(true)) => {
            let f = std::path::PathBuf::from(f);
            let downloads = app.path().download_dir().ok().and_then(|d| d.canonicalize().ok());
            let parent = f.canonicalize().ok().and_then(|c| c.parent().map(|p| p.to_path_buf()));
            let is_zip = f.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip"));
            (is_zip && downloads.is_some() && downloads == parent).then_some(f)
        }
        _ => None,
    }
}

const NEED_CF_KEY: &str = "CurseForge needs an API key. Add yours in Settings > Integrations.";

/// Create an instance from a modpack: a Modrinth or CurseForge project, or a
/// local .mrpack / CurseForge .zip. Returns as soon as the instance exists;
/// files download in the background with progress on that instance.
#[tauri::command]
async fn modpack_install(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: Option<String>,
    version_id: Option<String>,
    file_path: Option<String>,
    remove_file: Option<bool>,
) -> Result<Instance> {
    let paths = state.paths.clone();
    let cleanup = downloaded_zip(&app, &file_path, remove_file);
    let cf = cf_client(&paths).await.ok();
    if let Some(pid) = project_id.as_deref().filter(|p| ftb::is_ftb(p)) {
        let (manifest, info) = ftb::fetch(pid, version_id.as_deref()).await?;
        let inst = ftb::prepare(&paths, &manifest, info).await?;
        spawn_pack_install(app, paths, inst.clone(), std::path::PathBuf::new(), PackKind::Ftb(manifest), cf, None);
        return Ok(inst);
    }
    if let Some(pid) = project_id.as_deref().filter(|p| technic::is_technic(p)) {
        let (plan, info) = technic::fetch(&paths, pid, version_id.as_deref()).await?;
        let inst = technic::prepare(&paths, &plan, info).await?;
        spawn_pack_install(app, paths, inst.clone(), std::path::PathBuf::new(), PackKind::Technic(plan), cf, None);
        return Ok(inst);
    }
    let (archive, pack) = match (project_id, file_path) {
        // A pack the player downloaded by hand (its author turned off
        // launcher downloads): their zip, with the pack's details.
        (Some(pid), Some(path)) if curseforge::is_cf(&pid) => {
            let cf = cf.as_ref().ok_or_else(|| Error::msg(NEED_CF_KEY))?;
            let path = std::path::PathBuf::from(path);
            let file_id = version_id.as_deref().ok_or_else(|| Error::msg("Which version of the pack is this?"))?;
            let info = curseforge::manual_pack_info(cf, &pid, file_id, &path).await?;
            (path, Some(info))
        }
        (Some(pid), None) if curseforge::is_cf(&pid) => {
            let cf = cf.as_ref().ok_or_else(|| Error::msg(NEED_CF_KEY))?;
            let (path, info) = curseforge::fetch_modpack(cf, &paths, &pid, version_id.as_deref()).await?;
            (path, Some(info))
        }
        (Some(pid), _) => {
            let (path, info) = modpacks::fetch(&paths, &pid, version_id.as_deref()).await?;
            (path, Some(info))
        }
        (None, Some(path)) => (std::path::PathBuf::from(path), None),
        _ => return Err(Error::msg("Choose a modpack to install")),
    };

    let probe = archive.clone();
    let is_cf_pack = tokio::task::spawn_blocking(move || curseforge::is_curseforge_pack(&probe)).await.unwrap_or(false);
    let (inst, kind) = if is_cf_pack {
        if cf.is_none() {
            return Err(Error::msg(NEED_CF_KEY));
        }
        let (inst, manifest) = curseforge::prepare_modpack(&paths, &archive, pack).await?;
        (inst, PackKind::CurseForge(manifest))
    } else {
        let (inst, index) = modpacks::prepare(&paths, &archive, pack).await?;
        (inst, PackKind::Modrinth(index))
    };

    spawn_pack_install(app, paths, inst.clone(), archive, kind, cf, cleanup);
    Ok(inst)
}

/// If a CurseForge pack has to be downloaded by hand (its author turned off
/// launcher downloads): its file name, size and download page.
#[tauri::command]
async fn modpack_manual_info(state: State<'_, AppState>, project_id: String, version_id: Option<String>) -> Result<Option<curseforge::ManualPack>> {
    if !curseforge::is_cf(&project_id) {
        return Ok(None);
    }
    curseforge::manual_pack(&cf_client(&state.paths).await?, &project_id, version_id.as_deref()).await
}

/// Look in the Downloads folder for a file the player is downloading by
/// hand. Returns its path once it's all there (the right size).
#[tauri::command]
async fn find_download(app: AppHandle, file_name: String, size: u64) -> Result<Option<String>> {
    if file_name.contains(['/', '\\']) || file_name.contains("..") {
        return Ok(None);
    }
    let dir = app.path().download_dir()?;
    let stem = file_name.trim_end_matches(".zip");
    // Browsers name a second copy "pack (1).zip".
    let mut entries = tokio::fs::read_dir(&dir).await?;
    while let Some(e) = entries.next_entry().await? {
        let name = e.file_name().to_string_lossy().into_owned();
        let same = name == file_name || (name.starts_with(stem) && name.ends_with(".zip") && name[stem.len()..].trim_start().starts_with('('));
        if !same {
            continue;
        }
        if let Ok(meta) = e.metadata().await {
            if meta.is_file() && (size == 0 || meta.len() == size) {
                return Ok(Some(e.path().to_string_lossy().into_owned()));
            }
        }
    }
    Ok(None)
}

/// Download a modpack's files and install the game in the background, with
/// progress on the new instance.
fn spawn_pack_install(
    app: AppHandle,
    paths: Paths,
    inst: Instance,
    archive: std::path::PathBuf,
    kind: PackKind,
    cf: Option<curseforge::Client>,
    cleanup: Option<std::path::PathBuf>,
) {
    let app2 = app.clone();
    tokio::spawn(async move {
        let mut inst = inst;
        let emitter = app2.clone();
        let id = inst.id.clone();
        let progress: ProgressFn = Arc::new(move |p: InstallProgress| {
            let _ = emitter.emit("install-progress", InstallEvent { instance_id: id.clone(), progress: p });
        });
        let result: Result<Vec<curseforge::Blocked>> = async {
            let blocked = match kind {
                PackKind::Ftb(manifest) => {
                    ftb::populate(&paths, &inst, &manifest, &progress).await?;
                    vec![]
                }
                PackKind::Technic(plan) => {
                    // Technic packs only reveal their loader once unpacked.
                    inst = technic::populate(&paths, &inst, plan, &progress).await?;
                    vec![]
                }
                PackKind::Modrinth(index) => {
                    modpacks::populate(&paths, &inst, &archive, &index, &progress).await?;
                    vec![]
                }
                PackKind::CurseForge(manifest) => {
                    let cf = cf.as_ref().ok_or_else(|| Error::msg(NEED_CF_KEY))?;
                    curseforge::populate_modpack(cf, &paths, &inst, &archive, &manifest, &progress).await?
                }
            };
            install_inner(&app2, &paths, &mut inst).await?;
            Ok(blocked)
        }
        .await;
        if result.is_ok() {
            if let Some(f) = &cleanup {
                let _ = tokio::fs::remove_file(f).await;
            }
        }
        match result {
            Ok(blocked) if !blocked.is_empty() => {
                let list: Vec<manual::ManualFile> = blocked.iter().cloned().map(Into::into).collect();
                let _ = manual::save(&paths, &inst.id, &list).await;
                let _ = app2.emit("install-blocked", InstallBlocked { instance_id: inst.id.clone(), name: inst.name.clone(), files: blocked });
            }
            Ok(_) => {}
            Err(e) => {
                progress(InstallProgress { stage: "Done", done: 1, total: 1, bytes: 0 });
                let _ = app2.emit(
                    "install-error",
                    InstallError { instance_id: inst.id.clone(), name: inst.name.clone(), message: e.to_string() },
                );
            }
        }
    });
}

// ---------------- import from other launchers ----------------

/// Instances found in other launchers' default folders.
#[tauri::command]
async fn import_detect(state: State<'_, AppState>) -> Result<Vec<importer::Candidate>> {
    let latest = versions::manifest(&state.paths).await.ok().map(|m| m.latest.release);
    importer::detect(&state.paths, latest).await
}

/// Identify a folder the user picked (Prism/MultiMC, ATLauncher, CurseForge, Modrinth).
#[tauri::command]
async fn import_inspect(path: String) -> Result<importer::Candidate> {
    let p = std::path::PathBuf::from(&path);
    tokio::task::spawn_blocking(move || importer::inspect(&p))
        .await
        .map_err(|e| Error::msg(e.to_string()))?
        .ok_or_else(|| Error::msg("That folder isn't an instance from Prism, MultiMC, ATLauncher, CurseForge or the Modrinth App."))
}

/// Import instances: create each one now, then copy files and install the
/// game in the background with progress on the new instance.
#[tauri::command]
async fn import_instances(app: AppHandle, state: State<'_, AppState>, candidates: Vec<importer::Candidate>) -> Result<Vec<Instance>> {
    let paths = state.paths.clone();
    let mut created = Vec::new();
    for c in candidates {
        let inst = importer::create(&paths, &c).await?;
        created.push(inst.clone());
        let (app2, paths2) = (app.clone(), paths.clone());
        tokio::spawn(async move {
            let mut inst = inst;
            let emitter = app2.clone();
            let id = inst.id.clone();
            let progress: ProgressFn = Arc::new(move |p: InstallProgress| {
                let _ = emitter.emit("install-progress", InstallEvent { instance_id: id.clone(), progress: p });
            });
            let result = async {
                importer::copy_files(&paths2, &c, &inst, &progress).await?;
                install_inner(&app2, &paths2, &mut inst).await
            }
            .await;
            if let Err(e) = result {
                progress(InstallProgress { stage: "Done", done: 1, total: 1, bytes: 0 });
                let _ = app2.emit(
                    "install-error",
                    InstallError { instance_id: inst.id.clone(), name: inst.name.clone(), message: e.to_string() },
                );
            }
        });
    }
    Ok(created)
}

// ---------------- export as a modpack ----------------

/// Files and folders in an instance that can be bundled into an exported pack.
#[tauri::command]
async fn export_entries(state: State<'_, AppState>, instance_id: String) -> Result<Vec<export::Entry>> {
    export::entries(&state.paths, &instance_id).await
}

/// Export an instance as a Modrinth .mrpack or CurseForge .zip.
#[tauri::command]
async fn export_pack(state: State<'_, AppState>, instance_id: String, options: export::ExportOptions) -> Result<export::ExportReport> {
    let cf = cf_client(&state.paths).await.ok();
    export::export(&state.paths, &instance_id, &options, cf.as_ref()).await
}

/// Sort an instance's mods into server and client-only for a server pack.
#[tauri::command]
async fn serverpack_analyze(state: State<'_, AppState>, instance_id: String) -> Result<Vec<serverpack::ModSide>> {
    serverpack::analyze(&state.paths, &instance_id).await
}

/// Write a server pack zip with start scripts for the instance's loader.
#[tauri::command]
async fn serverpack_export(state: State<'_, AppState>, instance_id: String, options: serverpack::ServerPackOptions) -> Result<serverpack::ServerPackReport> {
    serverpack::export(&state.paths, &instance_id, &options).await
}

// ---------------- servers ----------------

#[tauri::command]
async fn servers_list(state: State<'_, AppState>) -> Result<Vec<servers::Server>> {
    servers::list(&state.paths).await
}

#[tauri::command]
async fn server_save(state: State<'_, AppState>, server: servers::Server) -> Result<Vec<servers::Server>> {
    servers::save(&state.paths, server).await
}

#[tauri::command]
async fn server_delete(state: State<'_, AppState>, id: String) -> Result<Vec<servers::Server>> {
    servers::delete(&state.paths, &id).await
}

#[tauri::command]
async fn servers_reorder(state: State<'_, AppState>, ids: Vec<String>) -> Result<Vec<servers::Server>> {
    servers::reorder(&state.paths, &ids).await
}

/// Live status of one server, straight from the server.
#[tauri::command]
async fn server_ping(address: String) -> Result<servers::Status> {
    Ok(servers::ping(&address).await)
}

/// The servers in an instance's own multiplayer list, for importing.
#[tauri::command]
async fn servers_from_instance(state: State<'_, AppState>, instance_id: String) -> Result<Vec<(String, String)>> {
    servers::from_instance(&state.paths, &instance_id).await
}

/// The public server list from peaklauncher.com.
#[tauri::command]
async fn discover_list() -> Result<Vec<launcher::discover::Listing>> {
    launcher::discover::list().await
}

/// Launch an instance straight into a server from the list. The server is
/// also added to the game's own multiplayer list, so players can rejoin
/// from inside Minecraft after a disconnect.
#[tauri::command]
async fn discover_join(app: AppHandle, state: State<'_, AppState>, instance_id: String, address: String, name: Option<String>) -> Result<()> {
    servers::parse_address(&address)?;
    remember_in_game(&state.paths, &instance_id, name.as_deref().unwrap_or(&address), &address).await;
    launch_instance(app, state, instance_id, Some(address)).await
}

/// Add a server to the instance's in-game list (servers.dat). Failing to do
/// so never stops the game from starting.
async fn remember_in_game(paths: &Paths, instance_id: &str, name: &str, address: &str) {
    let file = instances::game_dir(paths, instance_id).join("servers.dat");
    if let Err(e) = launcher::serversdat::add(&file, name, address).await {
        eprintln!("Couldn't add {address} to the in-game server list: {e}");
    }
}

/// The servers in an instance's in-game multiplayer list.
#[tauri::command]
async fn instance_servers(state: State<'_, AppState>, id: String) -> Result<Vec<launcher::serversdat::Entry>> {
    launcher::serversdat::list(&instances::game_dir(&state.paths, &id).join("servers.dat")).await
}

#[tauri::command]
async fn instance_server_add(state: State<'_, AppState>, id: String, name: String, address: String) -> Result<Vec<launcher::serversdat::Entry>> {
    servers::parse_address(&address)?;
    if state.running.lock().unwrap().contains_key(&id) {
        return Err(Error::msg("Close the game first; it rewrites its server list when it exits."));
    }
    let file = instances::game_dir(&state.paths, &id).join("servers.dat");
    if !launcher::serversdat::add(&file, &name, &address).await? {
        return Err(Error::msg("That server is already in this instance's list."));
    }
    launcher::serversdat::list(&file).await
}

#[tauri::command]
async fn instance_server_remove(state: State<'_, AppState>, id: String, address: String) -> Result<Vec<launcher::serversdat::Entry>> {
    if state.running.lock().unwrap().contains_key(&id) {
        return Err(Error::msg("Close the game first; it rewrites its server list when it exits."));
    }
    launcher::serversdat::remove(&instances::game_dir(&state.paths, &id).join("servers.dat"), &address).await
}

#[tauri::command]
async fn instance_servers_reorder(state: State<'_, AppState>, id: String, addresses: Vec<String>) -> Result<Vec<launcher::serversdat::Entry>> {
    if state.running.lock().unwrap().contains_key(&id) {
        return Err(Error::msg("Close the game first; it rewrites its server list when it exits."));
    }
    launcher::serversdat::reorder(&instances::game_dir(&state.paths, &id).join("servers.dat"), &addresses).await
}

/// Launch an instance straight into one of its own servers.
#[tauri::command]
async fn instance_server_join(app: AppHandle, state: State<'_, AppState>, id: String, address: String) -> Result<()> {
    servers::parse_address(&address)?;
    launch_instance(app, state, id, Some(address)).await
}

/// Launch an instance straight into a server, and remember it for next time.
#[tauri::command]
async fn server_join(app: AppHandle, state: State<'_, AppState>, server_id: String, instance_id: String) -> Result<()> {
    let server = servers::list(&state.paths)
        .await?
        .into_iter()
        .find(|s| s.id == server_id)
        .ok_or_else(|| Error::msg("That server isn't in the list any more."))?;
    servers::remember_instance(&state.paths, &server_id, &instance_id).await?;
    remember_in_game(&state.paths, &instance_id, &server.name, &server.address).await;
    launch_instance(app, state, instance_id, Some(server.address)).await
}

// ---------------- modpack artwork ----------------

/// Fill in icon/banner artwork for a modpack instance installed before Peak
/// saved artwork. Returns the updated instance.
#[tauri::command]
async fn refresh_pack_art(state: State<'_, AppState>, id: String) -> Result<Instance> {
    let paths = &state.paths;
    let mut inst = instances::get(paths, &id).await?;
    let Some(pid) = inst.pack.as_ref().and_then(|p| p.project_id.clone()) else { return Ok(inst) };
    let project = if ftb::is_ftb(&pid) {
        ftb::project(&pid).await?
    } else if technic::is_technic(&pid) {
        technic::project(&pid).await?
    } else if curseforge::is_cf(&pid) {
        curseforge::project(&cf_client(paths).await?, &pid).await?
    } else {
        modrinth::project(&pid).await?
    };
    if let Some(pack) = inst.pack.as_mut() {
        // An empty saved icon (from before the CurseForge fix) counts as missing.
        pack.icon_url = pack.icon_url.clone().filter(|u| !u.trim().is_empty()).or(project.icon_url.clone());
        pack.banner_url = pack
            .banner_url
            .clone()
            .or_else(|| project.gallery.iter().find(|g| g.featured).or(project.gallery.first()).map(|g| g.url.clone()));
        if pack.description.is_none() && !project.description.is_empty() {
            pack.description = Some(project.description.clone());
        }
    }
    instances::save(paths, &inst).await?;
    Ok(inst)
}

// ---------------- manual downloads ----------------

/// Files this instance still needs downloaded by hand.
#[tauri::command]
async fn manual_list(state: State<'_, AppState>, instance_id: String) -> Result<Vec<manual::ManualFile>> {
    manual::load(&state.paths, &instance_id).await
}

/// Look in the Downloads folder for pending files and move any found into place.
#[tauri::command]
async fn manual_scan(app: AppHandle, state: State<'_, AppState>, instance_id: String) -> Result<Vec<manual::ManualFile>> {
    let downloads = app.path().download_dir()?;
    manual::scan(&state.paths, &instance_id, &downloads).await
}

#[tauri::command]
async fn manual_dismiss(state: State<'_, AppState>, instance_id: String) -> Result<()> {
    manual::save(&state.paths, &instance_id, &[]).await
}

/// The last mod / pack update that can be undone in an instance, if any.
#[tauri::command]
async fn content_undo_info(state: State<'_, AppState>, instance_id: String) -> Result<Option<launcher::undo::UndoInfo>> {
    Ok(launcher::undo::info(&state.paths, &instance_id).await)
}

#[tauri::command]
async fn content_undo(state: State<'_, AppState>, instance_id: String) -> Result<usize> {
    if state.running.lock().unwrap().contains_key(&instance_id) {
        return Err(Error::msg("Close the game first."));
    }
    launcher::undo::undo(&state.paths, &instance_id).await
}

/// Mods that are missing something they need, read from the mod files.
#[tauri::command]
async fn content_missing(state: State<'_, AppState>, instance_id: String) -> Result<Vec<moddeps::Missing>> {
    let dir = modrinth::content_dir(&state.paths, &instance_id, Kind::Mod);
    tokio::task::spawn_blocking(move || moddeps::missing_in(&moddeps::installed_mods(&dir)))
        .await
        .map_err(|e| Error::msg(e.to_string()))
}

/// Why a game that just closed with an error crashed, and what would fix it.
#[tauri::command]
async fn crash_diagnose(state: State<'_, AppState>, instance_id: String, since_ms: i64) -> Result<launcher::crash::Diagnosis> {
    let since = std::time::UNIX_EPOCH + std::time::Duration::from_millis(since_ms.max(0) as u64);
    let s = settings::load(&state.paths).await?;
    let console = state
        .logs
        .lock()
        .unwrap()
        .get(&instance_id)
        .map(|l| l.iter().map(|x| x.line.as_str()).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default();
    launcher::crash::for_instance(&state.paths, &instance_id, since, s.memory_mb, console).await
}

/// A mod named in a crash, as a project that can be installed (Modrinth id).
#[tauri::command]
async fn crash_find_mod(state: State<'_, AppState>, instance_id: String, mod_id: String, name: String) -> Result<Option<String>> {
    let inst = instances::get(&state.paths, &instance_id).await?;
    Ok(modrinth::project_for_mod_id(&inst, &mod_id, &name).await)
}

/// Open a crash report (only files in an instance's crash-reports folder).
#[tauri::command]
async fn open_crash_report(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;
    let p = std::path::PathBuf::from(&path).canonicalize()?;
    let root = state.paths.instances().canonicalize()?;
    let ok = p.starts_with(&root) && p.parent().and_then(|d| d.file_name()).is_some_and(|n| n == "crash-reports");
    if !ok {
        return Err(Error::msg("That isn't a crash report."));
    }
    app.opener().open_path(p.to_string_lossy(), None::<&str>).map_err(|e| Error::msg(e.to_string()))
}

#[tauri::command]
async fn perf_mods(state: State<'_, AppState>, instance_id: String) -> Result<launcher::perfmods::PerfMods> {
    launcher::perfmods::suggest(&state.paths, &instance_id).await
}

#[tauri::command]
async fn perf_mods_add(state: State<'_, AppState>, instance_id: String, project_ids: Vec<String>) -> Result<Vec<String>> {
    if state.running.lock().unwrap().contains_key(&instance_id) {
        return Err(Error::msg("Close the game first."));
    }
    launcher::perfmods::add(&state.paths, &instance_id, &project_ids).await
}

#[tauri::command]
async fn perf_mods_undo(state: State<'_, AppState>, instance_id: String) -> Result<usize> {
    if state.running.lock().unwrap().contains_key(&instance_id) {
        return Err(Error::msg("Close the game first."));
    }
    launcher::perfmods::undo(&state.paths, &instance_id).await
}

#[tauri::command]
fn game_defaults(state: State<'_, AppState>) -> Option<launcher::shared::DefaultsInfo> {
    launcher::shared::defaults_info(&state.paths)
}

#[tauri::command]
async fn game_defaults_save(state: State<'_, AppState>, instance_id: String) -> Result<launcher::shared::DefaultsInfo> {
    let inst = instances::get(&state.paths, &instance_id).await?;
    launcher::shared::save_defaults(&state.paths, &instances::game_dir(&state.paths, &inst.id), &inst.name)
}

#[tauri::command]
fn game_defaults_lines(state: State<'_, AppState>) -> Vec<(String, String)> {
    launcher::shared::defaults_lines(&state.paths)
}

#[tauri::command]
fn game_defaults_set(state: State<'_, AppState>, lines: Vec<(String, String)>) -> Result<launcher::shared::DefaultsInfo> {
    launcher::shared::set_defaults(&state.paths, lines)
}

#[tauri::command]
fn game_defaults_clear(state: State<'_, AppState>) -> Result<()> {
    launcher::shared::clear_defaults(&state.paths)
}

#[tauri::command]
async fn storage_usage(state: State<'_, AppState>) -> Result<launcher::storage::Usage> {
    launcher::storage::usage(&state.paths).await
}

#[tauri::command]
async fn storage_clean(state: State<'_, AppState>, what: String) -> Result<u64> {
    launcher::storage::clean(&state.paths, &what).await
}

#[tauri::command]
async fn open_data_folder(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_path(state.paths.root.to_string_lossy(), None::<&str>).map_err(|e| Error::msg(e.to_string()))
}

#[tauri::command]
async fn open_shared_folder(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;
    let dir = launcher::shared::dir(&state.paths);
    tokio::fs::create_dir_all(&dir).await?;
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| Error::msg(e.to_string()))
}

#[tauri::command]
async fn open_downloads_folder(app: AppHandle) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;
    let dir = app.path().download_dir()?;
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| Error::msg(e.to_string()))
}

// ---------------- java ----------------

#[tauri::command]
async fn java_runtimes(state: State<'_, AppState>) -> Result<Vec<RuntimeOption>> {
    java::available_runtimes(&state.paths).await
}

// ---------------- screenshots ----------------

#[tauri::command]
async fn screenshots_list(state: State<'_, AppState>) -> Result<Vec<launcher::screenshots::Screenshot>> {
    let all = instances::list(&state.paths).await?;
    Ok(launcher::screenshots::list(&state.paths, &all).await)
}

#[tauri::command]
async fn screenshot_preview(state: State<'_, AppState>, instance_id: String, file: String) -> Result<String> {
    let p = launcher::screenshots::thumbnail(&state.paths, &instance_id, &file).await?;
    Ok(p.to_string_lossy().into_owned())
}

#[tauri::command]
async fn screenshot_set_tags(state: State<'_, AppState>, key: String, tags: Vec<String>) -> Result<Vec<String>> {
    launcher::screenshots::set_tags(&state.paths, &key, tags).await
}

#[tauri::command]
async fn screenshot_copy(state: State<'_, AppState>, instance_id: String, file: String) -> Result<()> {
    let p = launcher::screenshots::checked(&state.paths, &instance_id, &file)?;
    launcher::screenshots::copy_image(p).await
}

/// Save one screenshot (a copy) or several (a zip) where the player chose.
#[tauri::command]
async fn screenshot_export(state: State<'_, AppState>, items: Vec<(String, String)>, to: String) -> Result<usize> {
    let files = items
        .iter()
        .map(|(id, f)| launcher::screenshots::checked(&state.paths, id, f))
        .collect::<Result<Vec<_>>>()?;
    let to = std::path::PathBuf::from(to);
    if files.len() == 1 && !to.to_string_lossy().to_ascii_lowercase().ends_with(".zip") {
        tokio::fs::copy(&files[0], &to).await?;
        return Ok(1);
    }
    launcher::screenshots::export_zip(files, to).await
}

#[tauri::command]
async fn screenshot_trash(state: State<'_, AppState>, items: Vec<(String, String)>) -> Result<usize> {
    launcher::screenshots::trash(&state.paths, items).await
}

/// The Optimize button: memory, flags and Java suggested for one instance.
#[tauri::command]
async fn suggest_tuning(state: State<'_, AppState>, id: String) -> Result<launcher::tuning::Suggestion> {
    let inst = instances::get(&state.paths, &id).await?;
    Ok(launcher::tuning::suggest_for(&state.paths, &inst).await)
}

/// Every Java Peak has downloaded (Mojang's, Temurin, GraalVM) and its size.
#[tauri::command]
async fn installed_java(state: State<'_, AppState>) -> Result<Vec<launcher::distros::InstalledRuntime>> {
    Ok(launcher::distros::list_installed(&state.paths).await)
}

/// Delete a downloaded Java; it comes back by itself when a game needs it.
#[tauri::command]
async fn remove_java(state: State<'_, AppState>, id: String) -> Result<()> {
    if !state.running.lock().unwrap().is_empty() {
        return Err(Error::msg("Close your games first: one of them may be using this Java."));
    }
    launcher::distros::remove(&state.paths, &id).await
}

#[tauri::command]
async fn detect_java(state: State<'_, AppState>) -> Result<Vec<DetectedJava>> {
    Ok(java::detect(&state.paths).await)
}

#[tauri::command]
async fn test_java(path: String) -> Result<DetectedJava> {
    java::probe(std::path::Path::new(path.trim()))
        .await
        .ok_or_else(|| Error::msg("That doesn't look like a working Java executable."))
}

// ---------------- accounts ----------------

#[tauri::command]
async fn list_accounts(state: State<'_, AppState>) -> Result<Vec<AccountInfo>> {
    Ok(accounts::info(&accounts::load(&state.paths).await?))
}

/// An account's skin (cached for an hour). None for offline accounts and
/// accounts without a custom skin.
#[tauri::command]
async fn account_skin(state: State<'_, AppState>, id: String, refresh: Option<bool>) -> Result<Option<launcher::skins::Skin>> {
    let store = accounts::load(&state.paths).await?;
    let Some(account) = store.accounts.iter().find(|a| a.id == id) else { return Ok(None) };
    if account.kind != accounts::AccountKind::Microsoft {
        return Ok(None);
    }
    launcher::skins::get(&state.paths, &account.id, refresh.unwrap_or(false)).await
}

/// The cape an account wears (None for offline accounts or no cape).
#[tauri::command]
async fn account_cape(state: State<'_, AppState>, id: String) -> Result<Option<String>> {
    launcher::skins::cape(&state.paths, &id).await
}

/// A signed-in Microsoft account with a usable Minecraft token, for skin changes.
async fn skin_account(paths: &Paths, id: &str) -> Result<accounts::Account> {
    let store = accounts::load(paths).await?;
    let account = store.accounts.into_iter().find(|a| a.id == id).ok_or_else(|| Error::msg("That account isn't signed in any more."))?;
    if account.kind != accounts::AccountKind::Microsoft {
        return Err(Error::msg("Only Microsoft accounts have skins."));
    }
    accounts::ensure_fresh(paths, account).await
}

fn token(account: &accounts::Account) -> Result<&str> {
    account.access_token.as_deref().ok_or_else(|| Error::msg("Sign in to this account again."))
}

/// The account's current skin and the capes it owns.
#[tauri::command]
async fn skin_wardrobe(state: State<'_, AppState>, id: String) -> Result<launcher::skins::Wardrobe> {
    let a = skin_account(&state.paths, &id).await?;
    launcher::skins::wardrobe(&state.paths, &a.id, token(&a)?).await
}

/// Check a skin file the player picked and return it for previewing.
#[tauri::command]
async fn skin_read_file(path: String) -> Result<String> {
    launcher::skins::read_skin_file(std::path::Path::new(&path)).await
}

/// Upload a skin file ("classic" or "slim" arms).
#[tauri::command]
async fn skin_upload(state: State<'_, AppState>, id: String, path: String, model: String) -> Result<launcher::skins::Wardrobe> {
    let png = tokio::fs::read(&path).await?;
    let a = skin_account(&state.paths, &id).await?;
    launcher::skins::upload(&state.paths, &a.id, token(&a)?, png, &model).await
}

#[tauri::command]
async fn skin_set_model(state: State<'_, AppState>, id: String, model: String) -> Result<launcher::skins::Wardrobe> {
    let a = skin_account(&state.paths, &id).await?;
    launcher::skins::set_model(&state.paths, &a.id, token(&a)?, &model).await
}

#[tauri::command]
async fn skin_reset(state: State<'_, AppState>, id: String) -> Result<launcher::skins::Wardrobe> {
    let a = skin_account(&state.paths, &id).await?;
    launcher::skins::reset(&state.paths, &a.id, token(&a)?).await
}

/// Skins this account has worn in Peak, newest first.
#[tauri::command]
async fn skin_history(state: State<'_, AppState>, id: String) -> Result<Vec<launcher::skins::PastSkin>> {
    launcher::skins::history(&state.paths, &id).await
}

/// Wear a skin from the account's history again.
#[tauri::command]
async fn skin_use_saved(state: State<'_, AppState>, id: String, skin_id: String, model: String) -> Result<launcher::skins::Wardrobe> {
    let png = launcher::skins::history_png(&state.paths, &id, &skin_id).await?;
    let a = skin_account(&state.paths, &id).await?;
    launcher::skins::upload(&state.paths, &a.id, token(&a)?, png, &model).await
}

#[tauri::command]
async fn skin_history_remove(state: State<'_, AppState>, id: String, skin_id: String) -> Result<()> {
    launcher::skins::history_remove(&state.paths, &id, &skin_id).await
}

/// Look up any player's id, skin and cape by name.
#[tauri::command]
async fn player_lookup(name: String) -> Result<launcher::skins::Player> {
    launcher::skins::lookup(&name).await
}

/// Show one of the account's capes (None hides the cape).
#[tauri::command]
async fn skin_set_cape(state: State<'_, AppState>, id: String, cape_id: Option<String>) -> Result<launcher::skins::Wardrobe> {
    let a = skin_account(&state.paths, &id).await?;
    launcher::skins::set_cape(&state.paths, &a.id, token(&a)?, cape_id.as_deref()).await
}

/// Whether Peak runs as an AppImage and has a menu entry.
#[tauri::command]
fn appimage_menu_status() -> launcher::appimage::MenuStatus {
    launcher::appimage::status()
}

/// Add (or update) the applications-menu entry for this AppImage.
#[tauri::command]
fn appimage_menu_install() -> Result<launcher::appimage::MenuStatus> {
    launcher::appimage::install()
}

#[tauri::command]
fn appimage_menu_remove() -> Result<launcher::appimage::MenuStatus> {
    launcher::appimage::remove()
}

#[tauri::command]
async fn ms_login_start() -> Result<LoginStart> {
    accounts::start_login().await
}

/// Stop a sign-in that is waiting for the user (the sign-in window was closed).
#[tauri::command]
fn ms_login_cancel() {
    accounts::cancel_login();
}

#[tauri::command]
async fn ms_login_finish(state: State<'_, AppState>, start: LoginStart) -> Result<Vec<AccountInfo>> {
    let account = accounts::finish_login(&start).await?;
    accounts::upsert(&state.paths, account).await?;
    Ok(accounts::info(&accounts::load(&state.paths).await?))
}

/// Offline accounts are for development only; release builds require Microsoft sign-in.
#[tauri::command]
async fn add_offline_account(state: State<'_, AppState>, username: String) -> Result<Vec<AccountInfo>> {
    if !cfg!(debug_assertions) {
        return Err(Error::msg("Offline accounts are only available in development builds."));
    }
    accounts::upsert(&state.paths, accounts::offline_account(username.trim())?).await?;
    Ok(accounts::info(&accounts::load(&state.paths).await?))
}

#[tauri::command]
async fn set_active_account(state: State<'_, AppState>, id: String) -> Result<Vec<AccountInfo>> {
    let mut store = accounts::load(&state.paths).await?;
    if store.accounts.iter().any(|a| a.id == id) {
        store.active = Some(id);
        accounts::save(&state.paths, &store).await?;
    }
    Ok(accounts::info(&store))
}

#[tauri::command]
async fn remove_account(state: State<'_, AppState>, id: String) -> Result<Vec<AccountInfo>> {
    let mut store = accounts::load(&state.paths).await?;
    store.accounts.retain(|a| a.id != id);
    accounts::forget_session(&id);
    if store.active.as_deref() == Some(id.as_str()) {
        store.active = store.accounts.first().map(|a| a.id.clone());
    }
    accounts::save(&state.paths, &store).await?;
    Ok(accounts::info(&store))
}

#[tauri::command]
fn app_info(state: State<'_, AppState>) -> serde_json::Value {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "devBuild": cfg!(debug_assertions),
        "msConfigured": accounts::MS_CLIENT_ID.map(|s| !s.is_empty()).unwrap_or(false),
        "tokensEncrypted": accounts::tokens_encrypted(),
        "os": std::env::consts::OS,
        "totalMemoryMb": sys.total_memory() / 1024 / 1024,
        "dataDir": state.paths.root.to_string_lossy(),
        "jvmPresets": launcher::jvmflags::presets(),
        "curseforgeBuiltIn": curseforge::works_without_user_key(),
    })
}

// ---------------- settings ----------------

#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> Result<Settings> {
    settings::load(&state.paths).await
}

#[tauri::command]
async fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<Settings> {
    let old_key = settings::load(&state.paths).await.map(|s| s.curseforge_api_key).unwrap_or_default();
    settings::save(&state.paths, &settings).await?;
    // The key decides whether CurseForge is asked directly or via the proxy.
    if old_key.trim() != settings.curseforge_api_key.trim() {
        curseforge::clear_cache();
    }
    Ok(settings)
}

/// Put every setting back to its default (keeps your own CurseForge key).
#[tauri::command]
async fn reset_settings(state: State<'_, AppState>) -> Result<Settings> {
    let current = settings::load(&state.paths).await.unwrap_or_default();
    let fresh = settings::defaults_keeping(&current);
    settings::save(&state.paths, &fresh).await?;
    Ok(fresh)
}

// ---------------------------------------------------------------- start with the computer

/// The argument the OS passes when it starts Peak at sign-in.
const AUTOSTART_ARG: &str = "--autostart";

/// Whether Peak starts when the user signs in. The OS keeps this (a Run key on
/// Windows, a login item on macOS, an autostart entry on Linux), so it's off
/// until the user turns it on.
#[tauri::command]
fn autostart_enabled(app: AppHandle) -> Result<bool> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| Error::msg(format!("Couldn't check whether Peak starts with your computer: {e}")))
}

#[tauri::command]
fn set_autostart(app: AppHandle, on: bool) -> Result<()> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    let result = if on { manager.enable() } else { manager.disable() };
    result.map_err(|e| Error::msg(format!("Couldn't change whether Peak starts with your computer: {e}")))
}

/// Linux: WebKitGTK's GPU path (DMA-BUF) is badly broken on NVIDIA's driver,
/// especially under Wayland: the whole window crawls or stays blank. Turn it
/// off there, unless the player set it themselves.
#[cfg(target_os = "linux")]
fn linux_webview_workarounds() {
    let nvidia = std::path::Path::new("/proc/driver/nvidia/version").exists();
    if nvidia && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: runs first thing in run(), before Tauri or any other thread starts.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    linux_webview_workarounds();
    tauri::Builder::default()
        // First: a second copy of Peak (e.g. opened by a peak:// link) hands
        // its link to this one and exits.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            window::show_main(app);
            links::handle(app, argv);
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec![AUTOSTART_ARG])))
        .setup(|app| {
            // Started with the computer: open minimized rather than in the way.
            if std::env::args().any(|a| a == AUTOSTART_ARG) {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.minimize();
                }
            }
            let root = app.path().data_dir()?.join("PeakLauncher");
            std::fs::create_dir_all(&root)?;
            let paths = Paths::new(root);
            // Apply network settings (proxy, timeouts) before anything downloads.
            let s = tauri::async_runtime::block_on(settings::load(&paths)).unwrap_or_default();
            if let Err(e) = net::configure(&s.net_config()) {
                eprintln!("Ignoring invalid network settings: {e}");
            }
            if let Err(e) = window::setup_tray(app) {
                eprintln!("Tray icon unavailable: {e}");
            }
            app.manage(links::PendingLink::default());
            app.manage(presence::Presence::default());
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                // Installers register peak:// already; this also covers a
                // portable copy or a build run straight from its folder.
                #[cfg(any(windows, target_os = "linux"))]
                if let Err(e) = app.deep_link().register_all() {
                    eprintln!("Couldn't register peak:// links: {e}");
                }
                let handle = app.handle().clone();
                app.deep_link().on_open_url(move |event| {
                    links::handle(&handle, event.urls().iter().map(|u| u.to_string()));
                });
                if let Ok(Some(urls)) = app.deep_link().get_current() {
                    links::handle(app.handle(), urls.iter().map(|u| u.to_string()));
                }
            }
            app.manage(AppState {
                paths,
                running: Mutex::new(HashMap::new()),
                pids: Mutex::new(HashMap::new()),
                installing: Mutex::new(HashSet::new()),
                logs: Mutex::new(HashMap::new()),
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            // Console windows belong to the launcher; close them with it.
            if window.label() == "main" && matches!(event, tauri::WindowEvent::Destroyed) {
                for (label, w) in window.app_handle().webview_windows() {
                    if label.starts_with("console-") {
                        let _ = w.destroy();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            autostart_enabled,
            set_autostart,
            game_logs,
            content_linked,
            serverpack_analyze,
            servers_list,
            server_save,
            server_delete,
            servers_reorder,
            server_ping,
            servers_from_instance,
            server_join,
            serverpack_export,
            open_console_window,
            close_console_window,
            get_versions,
            loader_versions,
            list_instances,
            create_instance,
            update_instance,
            duplicate_instance,
            delete_instance,
            open_instance_folder,
            list_worlds,
            install_instance,
            launch_instance,
            stop_instance,
            running_instances,
            export_entries,
            export_pack,
            import_detect,
            import_inspect,
            import_instances,
            modpack_search,
            modpack_versions,
            modpack_install,
            modpack_manual_info,
            find_download,
            modpack_updates,
            modpack_skip_update,
            modpack_mute_updates,
            modpack_change_version,
            reset_settings,
            discover_list,
            instance_set_picture,
            instance_servers,
            instance_server_add,
            instance_server_remove,
            instance_servers_reorder,
            instance_server_join,
            links::take_pending_link,
            presence::presence_show,
            presence::presence_clear,
            discover_join,
            content_search,
            content_project,
            content_versions,
            content_install,
            content_plan,
            content_install_items,
            content_list,
            content_set_enabled,
            content_remove,
            content_updates,
            open_content_folder,
            refresh_pack_art,
            manual_list,
            manual_scan,
            manual_dismiss,
            open_downloads_folder,
            open_shared_folder,
            open_data_folder,
            storage_usage,
            storage_clean,
            game_defaults,
            game_defaults_save,
            game_defaults_clear,
            game_defaults_lines,
            game_defaults_set,
            perf_mods,
            crash_diagnose,
            content_missing,
            content_undo_info,
            content_undo,
            crash_find_mod,
            open_crash_report,
            perf_mods_add,
            perf_mods_undo,
            java_runtimes,
            installed_java,
            screenshots_list,
            screenshot_preview,
            screenshot_set_tags,
            screenshot_copy,
            screenshot_export,
            screenshot_trash,
            suggest_tuning,
            remove_java,
            detect_java,
            test_java,
            list_accounts,
            ms_login_start,
            ms_login_finish,
            ms_login_cancel,
            account_skin,
            account_cape,
            appimage_menu_status,
            appimage_menu_install,
            appimage_menu_remove,
            skin_wardrobe,
            skin_read_file,
            skin_upload,
            skin_set_model,
            skin_reset,
            skin_set_cape,
            skin_history,
            skin_use_saved,
            skin_history_remove,
            player_lookup,
            add_offline_account,
            set_active_account,
            remove_account,
            app_info,
            get_settings,
            save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Peak Launcher");
}

#[cfg(test)]
mod stop_tests {
    /// The safety net never ends a process that isn't Java (here: this test itself).
    #[test]
    fn only_java_processes_are_ended() {
        assert!(!super::kill_java(std::process::id()), "must refuse a non-Java process");
        assert!(!super::kill_java(u32::MAX - 7), "unknown ids do nothing");
    }

    /// Ends a real Java process that is stuck while shutting down, by id:
    /// PEAK_HANG_JAVA=<java.exe> PEAK_HANG_SRC=<Hang.java> cargo test --lib -- --ignored --nocapture kill_hung_java_by_pid
    #[test]
    #[ignore]
    fn kill_hung_java_by_pid() {
        let java = std::env::var("PEAK_HANG_JAVA").unwrap();
        let src = std::env::var("PEAK_HANG_SRC").unwrap();
        let mut child = std::process::Command::new(java).arg(src).spawn().unwrap();
        std::thread::sleep(std::time::Duration::from_secs(5));
        assert!(child.try_wait().unwrap().is_none(), "should be stuck");
        assert!(super::kill_java(child.id()), "should end it");
        let t = std::time::Instant::now();
        while child.try_wait().unwrap().is_none() {
            assert!(t.elapsed() < std::time::Duration::from_secs(5), "still running after kill");
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        println!("ended in {:?}", t.elapsed());
    }
}
