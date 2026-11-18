//! Server packs: a zip a server owner can unpack and start. Client-only mods
//! are found (Modrinth's data, the mod's own metadata, a list of well-known
//! client mods) and moved out of the way; the user reviews the result first.
//! The pack includes start scripts that set up the right server for the
//! instance's mod loader.

use super::instances::{self, Instance};
use super::loaders::{self, Loader};
use super::moddeps::{self, ModInfo};
use super::modrinth::{Kind, content_dir};
use super::net;
use super::paths::Paths;
use super::settings;
use super::versions;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Mods that only change what you see or hear, or the client's UI. Matched
/// on the ids a mod file provides.
const KNOWN_CLIENT: &[&str] = &[
    "sodium", "iris", "oculus", "embeddium", "rubidium", "magnesium", "indium", "sodium-extra", "sodiumextra",
    "reeses-sodium-options", "reeses_sodium_options", "sodiumoptionsapi", "optifine", "optifabric", "modmenu", "zoomify",
    "ok_zoomer", "okzoomer", "xaerominimap", "xaerominimapfair", "xaeroworldmap", "entityculling", "betterf3",
    "mousetweaks", "controlling", "notenoughanimations", "ambientsounds", "sound_physics_remastered", "lambdynlights",
    "dynamiclightsreforged", "continuity", "enhancedblockentities", "fpsreducer", "fancymenu", "drippyloadingscreen",
    "legendarytooltips", "betterthirdperson", "shouldersurfing", "skinlayers3d", "presencefootsteps", "fallingleaves",
    "waveycapes", "chat_heads", "dynamic_fps", "dynamicfps", "immediatelyfast", "moreculling", "citresewn", "animatica",
    "entity_texture_features", "entity_model_features", "cull_leaves", "cullleaves", "betteradvancements",
    "euphoria_patcher", "rrls", "nochatreports_client", "screenshot_viewer", "blur", "bettertitlescreen",
    "packmenu", "loadmyresources", "resourcepackoverrides", "itemborders", "visuality", "effective", "particlerain",
    "physicsmod", "firstperson", "betterclouds", "distanthorizons_client", "farsight", "sounds", "extrasounds",
    "auudio", "konkrete", "smoothscrollingeverywhere", "smoothswapping", "inventoryhud", "toastcontrol", "betterpingdisplay",
    "durabilitytooltip", "enchantmentdescriptions_client", "chatanimation", "wthit_client", "jade_client",
];

/// Folders a server needs from the instance; everything else (saves, resource
/// packs, shaders, screenshots, logs, options) stays behind.
const SERVER_FOLDERS: &[&str] = &["config", "defaultconfigs", "kubejs", "scripts", "global_packs", "datapacks", "openloader", "paxi"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModSide {
    pub file_name: String,
    pub name: String,
    /// True when the mod belongs on players' computers only.
    pub client_only: bool,
    /// False when the decision is a best guess the user should check.
    pub sure: bool,
    pub reason: String,
}

// ---------------------------------------------------------------- detection

#[derive(Deserialize)]
struct MrVersion {
    project_id: String,
}

#[derive(Deserialize)]
struct MrProject {
    id: String,
    #[serde(default)]
    client_side: String,
    #[serde(default)]
    server_side: String,
}

/// What Modrinth says about each file, by SHA-1: (client_side, server_side).
async fn modrinth_sides(hashes: &[String]) -> HashMap<String, (String, String)> {
    let mut out = HashMap::new();
    for chunk in hashes.chunks(500) {
        let Ok(resp) = net::client()
            .post("https://api.modrinth.com/v2/version_files")
            .json(&json!({ "hashes": chunk, "algorithm": "sha1" }))
            .send()
            .await
        else {
            continue;
        };
        let Ok(versions) = resp.json::<HashMap<String, MrVersion>>().await else { continue };
        let ids: Vec<String> = versions.values().map(|v| v.project_id.clone()).collect::<HashSet<_>>().into_iter().collect();
        let mut projects: HashMap<String, MrProject> = HashMap::new();
        for ids in ids.chunks(100) {
            let Ok(list) = serde_json::to_string(ids) else { continue };
            let resp = net::client().get("https://api.modrinth.com/v2/projects").query(&[("ids", list)]).send().await;
            if let Ok(found) = match resp {
                Ok(r) => r.json::<Vec<MrProject>>().await,
                Err(e) => Err(e),
            } {
                projects.extend(found.into_iter().map(|p| (p.id.clone(), p)));
            }
        }
        for (hash, v) in versions {
            if let Some(p) = projects.get(&v.project_id) {
                out.insert(hash, (p.client_side.clone(), p.server_side.clone()));
            }
        }
    }
    out
}

struct Scanned {
    file_name: String,
    sha1: String,
    info: ModInfo,
}

fn scan_mods(dir: &Path) -> Vec<Scanned> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let file_name = e.file_name().to_string_lossy().into_owned();
        // Disabled mods aren't part of the pack.
        if !file_name.ends_with(".jar") || !e.path().is_file() {
            continue;
        }
        let Ok(bytes) = std::fs::read(e.path()) else { continue };
        let sha1 = hex::encode(Sha1::digest(&bytes));
        out.push(Scanned { info: moddeps::read_mod(&e.path()), file_name, sha1 });
    }
    out
}

/// Decide one mod's side from what's known about it.
fn classify(info: &ModInfo, modrinth: Option<&(String, String)>) -> (bool, bool, String) {
    // The loader itself skips mods marked for the other side, so this is certain.
    if let (Some(env), Some(src @ ("fabric.mod.json" | "quilt.mod.json"))) = (info.environment.as_deref(), info.environment_source) {
        return match env {
            "client" => (true, true, format!("Marked client-only in {src}")),
            _ => (false, true, format!("Marked server-only in {src}")),
        };
    }
    if let Some((client, server)) = modrinth {
        if server == "unsupported" {
            return (true, true, "Modrinth: client only".into());
        }
        if client == "unsupported" {
            return (false, true, "Modrinth: server only".into());
        }
        return (false, true, "Modrinth: runs on servers".into());
    }
    if info.environment.as_deref() == Some("client") {
        return (true, false, "Only needs Minecraft on the client (mods.toml)".into());
    }
    if info.provides.iter().any(|id| KNOWN_CLIENT.contains(&id.as_str())) {
        return (true, false, "Known client-only mod".into());
    }
    (false, false, "No client-only marker found, kept to be safe".into())
}

/// Sort the instance's enabled mods into server and client-only.
pub async fn analyze(paths: &Paths, inst_id: &str) -> Result<Vec<ModSide>> {
    let dir = content_dir(paths, inst_id, Kind::Mod);
    let scanned = tokio::task::spawn_blocking(move || scan_mods(&dir)).await.map_err(|e| Error::msg(e.to_string()))?;
    let hashes: Vec<String> = scanned.iter().map(|s| s.sha1.clone()).collect();
    let sides = modrinth_sides(&hashes).await;
    Ok(decide(&scanned, &sides))
}

fn decide(scanned: &[Scanned], sides: &HashMap<String, (String, String)>) -> Vec<ModSide> {
    let mut out: Vec<ModSide> = scanned
        .iter()
        .map(|s| {
            let (client_only, sure, reason) = classify(&s.info, sides.get(&s.sha1));
            let name = s.info.name.clone().unwrap_or_else(|| s.file_name.trim_end_matches(".jar").to_string());
            ModSide { file_name: s.file_name.clone(), name, client_only, sure, reason }
        })
        .collect();

    // A server mod that needs a "client-only" one keeps it on the server.
    loop {
        let mut changed = false;
        let needed: HashMap<String, String> = scanned
            .iter()
            .zip(&out)
            .filter(|(_, m)| !m.client_only)
            .flat_map(|(s, m)| s.info.requires.iter().map(move |r| (r.clone(), m.name.clone())))
            .collect();
        for (s, m) in scanned.iter().zip(out.iter_mut()) {
            if !m.client_only {
                continue;
            }
            if let Some(by) = s.info.provides.iter().find_map(|p| needed.get(p)) {
                m.client_only = false;
                m.sure = true;
                m.reason = format!("Needed by {by}");
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    out.sort_by(|a, b| b.client_only.cmp(&a.client_only).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out
}

// ---------------------------------------------------------------- export

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerPackOptions {
    /// Where to write the zip.
    pub path: String,
    /// Mod files that belong on clients only.
    pub client_files: Vec<String>,
    /// Put client-only mods in mods/CLIENTSIDE (true) or leave them out.
    pub keep_client: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerPackReport {
    pub path: String,
    pub size_bytes: u64,
    pub server_mods: usize,
    pub client_mods: usize,
    pub java_major: u32,
}

/// A file going into the zip: from disk or generated.
enum Item {
    File(String, PathBuf),
    Text(String, String, bool),
}

/// The server files for the instance's loader, plus the commands the start
/// scripts run (Windows, then Unix).
struct LoaderSetup {
    files: Vec<Item>,
    bat: String,
    sh: String,
    note: String,
}

async fn loader_setup(paths: &Paths, inst: &Instance) -> Result<LoaderSetup> {
    let mc = &inst.game_version;
    let loader = Loader::parse(&inst.loader)?;
    let cache = paths.cache().join("server-packs");
    let fetch = |url: String, name: &str| {
        let dest = cache.join(name);
        async move {
            net::download_all(vec![net::Download::new(url.clone(), dest.clone(), None, None)], 1, |_| {})
                .await
                .map_err(|e| Error::msg(format!("Couldn't download {url}: {e}")))?;
            Ok::<PathBuf, Error>(dest)
        }
    };
    let java = |jar: &str| (format!("java -Xms%MEMORY% -Xmx%MEMORY% -jar {jar} nogui"), format!("java -Xms\"$MEMORY\" -Xmx\"$MEMORY\" -jar {jar} nogui"));

    Ok(match loader {
        Loader::Vanilla => {
            let v = versions::load_version_json(paths, mc).await?;
            let url = v.downloads.and_then(|d| d.server).map(|a| a.url).ok_or_else(|| Error::msg(format!("Minecraft {mc} has no official server")))?;
            let jar = fetch(url, &format!("minecraft-server-{mc}.jar")).await?;
            let (bat, sh) = java("server.jar");
            LoaderSetup { files: vec![Item::File("server.jar".into(), jar)], bat, sh, note: format!("Minecraft {mc} (vanilla)") }
        }
        Loader::Fabric => {
            let lv = inst.loader_version.clone().ok_or_else(|| Error::msg("No Fabric version set"))?;
            let installers: Vec<Value> = net::get_json(&format!("{}/versions/installer", loaders::FABRIC_META)).await?;
            let installer = installers
                .iter()
                .find(|i| i["stable"].as_bool().unwrap_or(false))
                .or(installers.first())
                .and_then(|i| i["version"].as_str())
                .ok_or_else(|| Error::msg("Couldn't find the Fabric installer"))?
                .to_string();
            let url = format!("{}/versions/loader/{mc}/{lv}/{installer}/server/jar", loaders::FABRIC_META);
            let jar = fetch(url, &format!("fabric-server-{mc}-{lv}-{installer}.jar")).await?;
            let (bat, sh) = java("fabric-server-launch.jar");
            LoaderSetup { files: vec![Item::File("fabric-server-launch.jar".into(), jar)], bat, sh, note: format!("Fabric {lv}") }
        }
        Loader::Quilt => {
            let lv = inst.loader_version.clone().ok_or_else(|| Error::msg("No Quilt version set"))?;
            let installers: Vec<Value> = net::get_json(&format!("{}/versions/installer", loaders::QUILT_META)).await?;
            let url = installers.first().and_then(|i| i["url"].as_str()).ok_or_else(|| Error::msg("Couldn't find the Quilt installer"))?.to_string();
            let jar = fetch(url, "quilt-installer.jar").await?;
            let install = format!("java -jar quilt-installer.jar install server {mc} {lv} --install-dir=. --download-server");
            let (run_bat, run_sh) = java("quilt-server-launch.jar");
            LoaderSetup {
                files: vec![Item::File("quilt-installer.jar".into(), jar)],
                bat: format!("if not exist quilt-server-launch.jar (\r\n  {install} || goto :failed\r\n)\r\n{run_bat}"),
                sh: format!("if [ ! -f quilt-server-launch.jar ]; then\n  {install}\nfi\n{run_sh}"),
                note: format!("Quilt {lv}"),
            }
        }
        Loader::Forge | Loader::NeoForge => {
            let lv = inst.loader_version.clone().ok_or_else(|| Error::msg("No loader version set"))?;
            let installer = loaders::forge_installer(paths, loader, &lv).await?;
            let name = if loader == Loader::Forge { "Forge" } else { "NeoForge" };
            // Newer versions make run.bat/run.sh (memory in user_jvm_args.txt);
            // older ones make a forge jar to start directly.
            let bat = "if not exist .server-installed (\r\n  \
                       java -jar installer.jar --installServer || goto :failed\r\n  \
                       echo -Xms%MEMORY% -Xmx%MEMORY%> user_jvm_args.txt\r\n  \
                       type nul > .server-installed\r\n)\r\n\
                       if exist run.bat (\r\n  call run.bat nogui\r\n  goto :end\r\n)\r\n\
                       set SERVER_JAR=\r\n\
                       for %%f in (forge-*.jar) do if /i not \"%%f\"==\"installer.jar\" set SERVER_JAR=%%f\r\n\
                       if \"%SERVER_JAR%\"==\"\" (\r\n  echo Couldn't find the server jar.\r\n  goto :failed\r\n)\r\n\
                       java -Xms%MEMORY% -Xmx%MEMORY% -jar \"%SERVER_JAR%\" nogui"
                .to_string();
            let sh = "if [ ! -f .server-installed ]; then\n  \
                      java -jar installer.jar --installServer\n  \
                      echo \"-Xms$MEMORY -Xmx$MEMORY\" > user_jvm_args.txt\n  \
                      touch .server-installed\nfi\n\
                      if [ -f run.sh ]; then\n  exec sh ./run.sh nogui\nfi\n\
                      SERVER_JAR=$(ls forge-*.jar 2>/dev/null | grep -v installer | head -n 1)\n\
                      if [ -z \"$SERVER_JAR\" ]; then echo \"Couldn't find the server jar.\"; exit 1; fi\n\
                      exec java -Xms\"$MEMORY\" -Xmx\"$MEMORY\" -jar \"$SERVER_JAR\" nogui"
                .to_string();
            LoaderSetup { files: vec![Item::File("installer.jar".into(), installer)], bat, sh, note: format!("{name} {lv}") }
        }
    })
}

fn start_bat(memory: &str, body: &str) -> String {
    format!(
        "@echo off\r\nrem Server pack made with Peak Launcher (https://peaklauncher.com)\r\n\
         cd /d \"%~dp0\"\r\nset MEMORY={memory}\r\n\r\n{body}\r\n\r\n:end\r\npause\r\nexit /b\r\n\r\n\
         :failed\r\necho.\r\necho The server didn't start. See the messages above.\r\npause\r\nexit /b 1\r\n"
    )
}

fn start_sh(memory: &str, body: &str) -> String {
    format!("#!/bin/sh\n# Server pack made with Peak Launcher (https://peaklauncher.com)\nset -e\ncd \"$(dirname \"$0\")\"\nMEMORY={memory}\n\n{body}\n")
}

fn readme(inst: &Instance, note: &str, java_major: u32, memory: &str, client: &[String], keep_client: bool) -> String {
    let mut s = format!(
        "{name} server\r\n{line}\r\n\r\nMinecraft {mc}, {note}\r\nNeeds Java {java_major} or newer.\r\n\r\n\
         Starting the server\r\n  Windows: double-click start.bat\r\n  Linux / macOS: run  sh start.sh\r\n\r\n\
         The first start sets up the server files. Minecraft then stops and asks you to accept its EULA:\r\n\
         open eula.txt and change eula=false to eula=true (this means you agree to https://aka.ms/MinecraftEULA),\r\n\
         then start it again.\r\n\r\nMemory: the server uses {memory}. Change MEMORY at the top of start.bat / start.sh.\r\n",
        name = inst.name,
        line = "=".repeat(inst.name.len() + 7),
        mc = inst.game_version,
    );
    if !client.is_empty() {
        s.push_str(&format!(
            "\r\nClient-only mods ({}) {}:\r\n",
            client.len(),
            if keep_client { "were moved to mods/CLIENTSIDE, where the server ignores them" } else { "were left out" }
        ));
        for c in client {
            s.push_str(&format!("  - {c}\r\n"));
        }
        s.push_str("If the server reports a missing mod, move that one back into mods.\r\n");
    }
    s
}

/// Every file under `dir`, as paths relative to `base` with forward slashes.
fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(base, &p, out);
        } else if let Ok(rel) = p.strip_prefix(base) {
            out.push((rel.to_string_lossy().replace('\\', "/"), p));
        }
    }
}

pub async fn export(paths: &Paths, inst_id: &str, opts: &ServerPackOptions) -> Result<ServerPackReport> {
    let inst = instances::get(paths, inst_id).await?;
    let game = instances::game_dir(paths, inst_id);
    let setup = loader_setup(paths, &inst).await?;
    let memory_mb = match inst.memory_mb {
        Some(m) => m,
        None => settings::load(paths).await.map(|s| s.memory_mb).unwrap_or(4096),
    };
    let memory = format!("{}G", (memory_mb.max(2048) as f64 / 1024.0).ceil() as u32);
    let java_major = versions::load_version_json(paths, &inst.game_version)
        .await
        .ok()
        .and_then(|v| v.java_version.map(|j| j.major_version))
        .unwrap_or(8);

    let client: HashSet<&str> = opts.client_files.iter().map(String::as_str).collect();
    let mods_dir = content_dir(paths, inst_id, Kind::Mod);
    let mut items: Vec<Item> = Vec::new();
    let (mut server_mods, mut client_names) = (0usize, Vec::new());
    if let Ok(rd) = std::fs::read_dir(&mods_dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".jar") || !e.path().is_file() {
                continue;
            }
            if client.contains(name.as_str()) {
                client_names.push(name.clone());
                if opts.keep_client {
                    items.push(Item::File(format!("mods/CLIENTSIDE/{name}"), e.path()));
                }
            } else {
                server_mods += 1;
                items.push(Item::File(format!("mods/{name}"), e.path()));
            }
        }
    }
    client_names.sort_by_key(|n| n.to_lowercase());
    for folder in SERVER_FOLDERS {
        let mut files = Vec::new();
        walk(&game, &game.join(folder), &mut files);
        items.extend(files.into_iter().map(|(rel, abs)| Item::File(rel, abs)));
    }
    items.extend(setup.files);
    items.push(Item::Text("start.bat".into(), start_bat(&memory, &setup.bat), false));
    items.push(Item::Text("start.sh".into(), start_sh(&memory, &setup.sh), true));
    items.push(Item::Text(
        "README.txt".into(),
        readme(&inst, &setup.note, java_major, &memory, &client_names, opts.keep_client),
        false,
    ));

    let path = PathBuf::from(&opts.path);
    let size = tokio::task::spawn_blocking(move || write_zip(&path, items)).await.map_err(|e| Error::msg(e.to_string()))??;
    Ok(ServerPackReport { path: opts.path.clone(), size_bytes: size, server_mods, client_mods: client_names.len(), java_major })
}

fn write_zip(path: &Path, items: Vec<Item>) -> Result<u64> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("part");
    {
        let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(std::fs::File::create(&tmp)?));
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut seen = HashSet::new();
        for item in items {
            match item {
                Item::File(rel, abs) => {
                    if !seen.insert(rel.clone()) {
                        continue;
                    }
                    zip.start_file(rel, opts)?;
                    std::io::copy(&mut std::fs::File::open(abs)?, &mut zip)?;
                }
                Item::Text(rel, text, executable) => {
                    let o = if executable { opts.unix_permissions(0o755) } else { opts.unix_permissions(0o644) };
                    zip.start_file(rel, o)?;
                    zip.write_all(text.as_bytes())?;
                }
            }
        }
        zip.finish()?.flush()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(std::fs::metadata(path)?.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(provides: &[&str], requires: &[&str], env: Option<(&str, &'static str)>) -> ModInfo {
        ModInfo {
            name: Some(provides[0].to_string()),
            provides: provides.iter().map(|s| s.to_string()).collect(),
            requires: requires.iter().map(|s| s.to_string()).collect(),
            environment: env.map(|(e, _)| e.to_string()),
            environment_source: env.map(|(_, s)| s),
        }
    }

    fn scanned(name: &str, i: ModInfo) -> Scanned {
        Scanned { file_name: format!("{name}.jar"), sha1: name.to_string(), info: i }
    }

    #[test]
    fn sorts_mods_by_the_best_evidence() {
        let mods = vec![
            scanned("sodium", info(&["sodium"], &[], Some(("client", "fabric.mod.json")))),
            scanned("lithium", info(&["lithium"], &[], None)),
            scanned("zoom", info(&["zoomify"], &[], None)),
            scanned("torches", info(&["torches"], &[], Some(("client", "mods.toml")))),
            scanned("mapmod", info(&["mapmod"], &[], None)),
            scanned("unknown", info(&["unknownmod"], &[], None)),
        ];
        let mut sides = HashMap::new();
        sides.insert("lithium".to_string(), ("optional".to_string(), "optional".to_string()));
        sides.insert("mapmod".to_string(), ("required".to_string(), "unsupported".to_string()));
        let out = decide(&mods, &sides);
        let get = |f: &str| out.iter().find(|m| m.file_name == format!("{f}.jar")).unwrap();
        assert!(get("sodium").client_only && get("sodium").sure);
        assert!(!get("lithium").client_only && get("lithium").sure);
        assert!(get("mapmod").client_only && get("mapmod").sure);
        assert!(get("zoom").client_only && !get("zoom").sure);
        assert!(get("torches").client_only && !get("torches").sure);
        assert!(!get("unknown").client_only && !get("unknown").sure);
        assert!(out[0].client_only, "client-only mods are listed first");
    }

    #[test]
    fn keeps_client_mods_that_server_mods_need() {
        let mods = vec![
            scanned("lib", info(&["sharedlib"], &[], None)),
            scanned("server-mod", info(&["servermod"], &["sharedlib"], None)),
        ];
        let mut sides = HashMap::new();
        sides.insert("lib".to_string(), ("required".to_string(), "unsupported".to_string()));
        let out = decide(&mods, &sides);
        let lib = out.iter().find(|m| m.file_name == "lib.jar").unwrap();
        assert!(!lib.client_only);
        assert_eq!(lib.reason, "Needed by servermod");
    }

    #[test]
    fn fabric_marker_beats_modrinth() {
        let mods = vec![scanned("a", info(&["a"], &[], Some(("client", "fabric.mod.json"))))];
        let mut sides = HashMap::new();
        sides.insert("a".to_string(), ("required".to_string(), "optional".to_string()));
        assert!(decide(&mods, &sides)[0].client_only);
    }

    #[test]
    fn scripts_and_readme_mention_what_matters() {
        let bat = start_bat("6G", "java -jar x.jar");
        assert!(bat.contains("set MEMORY=6G") && bat.contains("cd /d \"%~dp0\""));
        let sh = start_sh("6G", "java -jar x.jar");
        assert!(sh.starts_with("#!/bin/sh") && sh.contains("MEMORY=6G"));
        let inst: Instance = serde_json::from_value(json!({
            "id": "p", "name": "Pack", "gameVersion": "1.21.1", "loader": "fabric", "loaderVersion": "0.16.9", "createdAt": "2026-01-01T00:00:00Z"
        }))
        .unwrap();
        let r = readme(&inst, "Fabric 0.16.9", 21, "6G", &["sodium.jar".into()], true);
        assert!(r.contains("Java 21") && r.contains("eula=true") && r.contains("mods/CLIENTSIDE") && r.contains("sodium.jar"));
    }
}

#[cfg(test)]
mod live {
    /// Analyze a real instance and build its server pack in the temp folder:
    /// PEAK_TEST_INSTANCE=<id> cargo test --lib -- --ignored --nocapture serverpack_live
    #[tokio::test]
    #[ignore]
    async fn serverpack_live() {
        let id = std::env::var("PEAK_TEST_INSTANCE").expect("PEAK_TEST_INSTANCE");
        let root = std::path::PathBuf::from(std::env::var("APPDATA").unwrap()).join("PeakLauncher");
        let paths = super::Paths::new(root);
        let sides = super::analyze(&paths, &id).await.unwrap();
        let client: Vec<_> = sides.iter().filter(|m| m.client_only).collect();
        let unsure = sides.iter().filter(|m| !m.sure).count();
        println!("{} mods: {} client-only, {} not sure", sides.len(), client.len(), unsure);
        for m in &client {
            println!("  CLIENT {:<40} {}{}", m.name, m.reason, if m.sure { "" } else { "  (check)" });
        }
        let out = std::env::temp_dir().join(format!("peak-serverpack-{id}.zip"));
        let opts = super::ServerPackOptions {
            path: out.to_string_lossy().into(),
            client_files: client.iter().map(|m| m.file_name.clone()).collect(),
            keep_client: true,
        };
        let r = super::export(&paths, &id, &opts).await.unwrap();
        println!("wrote {} ({} KB): {} server mods, {} client, Java {}", r.path, r.size_bytes / 1024, r.server_mods, r.client_mods, r.java_major);
    }
}
