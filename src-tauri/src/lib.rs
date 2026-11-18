mod error;
mod launcher;
mod window;

use crate::error::{Error, Result};
use crate::launcher::accounts::{self, AccountInfo, DeviceCode};
use crate::launcher::install::{self, InstallProgress, ProgressFn};
use crate::launcher::instances::{self, Instance};
use crate::launcher::java::{self, DetectedJava, RuntimeOption};
use crate::launcher::launch;
use crate::launcher::loaders::{self, Loader, LoaderVersion};
use crate::launcher::curseforge;
use crate::launcher::export;
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
    instances::apply_edit(&mut current, instance);
    instances::save(&state.paths, &current).await?;
    Ok(current)
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
    progress(InstallProgress { stage: "Done", done: 1, total: 1, bytes: 0 });
    let version_id = result?;
    inst.installed = true;
    inst.version_id = Some(version_id);
    instances::save(paths, inst).await
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
async fn launch_instance(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<()> {
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
    let plan = launch::plan(&paths, &inst, &version, &account, &settings).await?;

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

    let (kill_tx, kill_rx) = oneshot::channel();
    state.running.lock().unwrap().insert(id.clone(), kill_tx);
    let _ = app.emit("game-state", StateEvent { instance_id: id.clone(), state: "running", exit_code: None });

    window::after_launch(app.clone(), id.clone(), settings.after_launch.clone(), pid, window_seen);

    let started = std::time::Instant::now();
    inst.last_played = Some(chrono::Utc::now());
    instances::save(&paths, &inst).await?;

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
        if let Ok(mut fresh) = instances::get(&paths, &id).await {
            fresh.playtime_secs += started.elapsed().as_secs();
            let _ = instances::save(&paths, &fresh).await;
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
fn stop_instance(state: State<'_, AppState>, id: String) -> Result<()> {
    if let Some(tx) = state.running.lock().unwrap().remove(&id) {
        let _ = tx.send(());
    }
    Ok(())
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
    offset: u32,
) -> Result<SearchResult> {
    let kind = Kind::parse(&kind)?;
    if source == "curseforge" {
        let cf = cf_client(&state.paths).await?;
        let inst = instances::get(&state.paths, &instance_id).await?;
        return curseforge::search(&cf, Some(&inst), Some(kind), &query, &sort, offset).await;
    }
    modrinth::search(&state.paths, &instance_id, kind, &query, &sort, offset).await
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
async fn modpack_search(state: State<'_, AppState>, source: String, query: String, sort: String, offset: u32) -> Result<SearchResult> {
    if source == "curseforge" {
        return curseforge::search(&cf_client(&state.paths).await?, None, None, &query, &sort, offset).await;
    }
    if source == "ftb" {
        return ftb::search(&query, &sort, offset).await;
    }
    if source == "technic" {
        return technic::search(&query, &sort, offset).await;
    }
    modpacks::search(&query, &sort, offset).await
}

#[tauri::command]
async fn modpack_versions(state: State<'_, AppState>, project_id: String) -> Result<Vec<modrinth::Version>> {
    if ftb::is_ftb(&project_id) {
        return ftb::versions(&project_id).await;
    }
    if technic::is_technic(&project_id) {
        return technic::versions(&project_id).await;
    }
    if curseforge::is_cf(&project_id) {
        return curseforge::modpack_versions(&cf_client(&state.paths).await?, &project_id).await;
    }
    modpacks::versions(&project_id).await
}

enum PackKind {
    Modrinth(modpacks::Index),
    CurseForge(curseforge::Manifest),
    Ftb(ftb::Manifest),
    Technic(technic::Plan),
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
) -> Result<Instance> {
    let paths = state.paths.clone();
    let cf = cf_client(&paths).await.ok();
    if let Some(pid) = project_id.as_deref().filter(|p| ftb::is_ftb(p)) {
        let (manifest, info) = ftb::fetch(pid, version_id.as_deref()).await?;
        let inst = ftb::prepare(&paths, &manifest, info).await?;
        spawn_pack_install(app, paths, inst.clone(), std::path::PathBuf::new(), PackKind::Ftb(manifest), cf);
        return Ok(inst);
    }
    if let Some(pid) = project_id.as_deref().filter(|p| technic::is_technic(p)) {
        let (plan, info) = technic::fetch(&paths, pid, version_id.as_deref()).await?;
        let inst = technic::prepare(&paths, &plan, info).await?;
        spawn_pack_install(app, paths, inst.clone(), std::path::PathBuf::new(), PackKind::Technic(plan), cf);
        return Ok(inst);
    }
    let (archive, pack) = match (project_id, file_path) {
        (Some(pid), _) if curseforge::is_cf(&pid) => {
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

    spawn_pack_install(app, paths, inst.clone(), archive, kind, cf);
    Ok(inst)
}

/// Download a modpack's files and install the game in the background, with
/// progress on the new instance.
fn spawn_pack_install(app: AppHandle, paths: Paths, inst: Instance, archive: std::path::PathBuf, kind: PackKind, cf: Option<curseforge::Client>) {
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
        pack.icon_url = pack.icon_url.clone().or(project.icon_url.clone());
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

#[tauri::command]
async fn ms_login_start() -> Result<DeviceCode> {
    accounts::start_device_code().await
}

#[tauri::command]
async fn ms_login_finish(state: State<'_, AppState>, code: DeviceCode) -> Result<Vec<AccountInfo>> {
    let account = accounts::poll_device_code(&code).await?;
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
        "os": std::env::consts::OS,
        "totalMemoryMb": sys.total_memory() / 1024 / 1024,
        "dataDir": state.paths.root.to_string_lossy(),
        "defaultJvmArgs": settings::DEFAULT_JVM_ARGS,
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
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
            app.manage(AppState {
                paths,
                running: Mutex::new(HashMap::new()),
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
            game_logs,
            content_linked,
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
            java_runtimes,
            detect_java,
            test_java,
            list_accounts,
            ms_login_start,
            ms_login_finish,
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
