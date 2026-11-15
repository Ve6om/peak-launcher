//! Import instances from other launchers: Prism / PolyMC / MultiMC,
//! ATLauncher, the CurseForge app, the official Minecraft Launcher and older
//! Modrinth App profiles.
//!
//! Importing always copies files; the other launcher's folders are never
//! modified.

use super::install::{InstallProgress, ProgressFn};
use super::instances::{self, Instance};
use super::paths::Paths;
use crate::error::{Error, Result};
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// "prism" | "multimc" | "atlauncher" | "curseforge" | "modrinth" | "official"
    pub source: String,
    pub source_label: String,
    /// The instance folder in the other launcher.
    pub path: String,
    /// Where its game files are (often a subfolder).
    pub game_dir: String,
    pub name: String,
    pub game_version: String,
    pub loader: String,
    pub loader_version: Option<String>,
    pub playtime_secs: u64,
    pub last_played: Option<DateTime<Utc>>,
    pub memory_mb: Option<u32>,
    pub jvm_args: Option<String>,
    pub already_imported: bool,
}

fn label(source: &str) -> &'static str {
    match source {
        "prism" => "Prism Launcher",
        "multimc" => "MultiMC / PolyMC",
        "atlauncher" => "ATLauncher",
        "curseforge" => "CurseForge",
        "modrinth" => "Modrinth App",
        _ => "Minecraft Launcher",
    }
}

fn read_json(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

/// Forge versions are stored with the game version prefixed.
fn forge_version(v: &str, mc: &str) -> String {
    if v.starts_with(&format!("{mc}-")) { v.to_string() } else { format!("{mc}-{v}") }
}

fn data_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(appdata) = std::env::var("APPDATA") {
        out.push(PathBuf::from(appdata));
    }
    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        let home = PathBuf::from(home);
        out.push(home.join("Library/Application Support"));
        out.push(home.join(".local/share"));
        out.push(home.join(".var/app/org.prismlauncher.PrismLauncher/data"));
    }
    out
}

fn home() -> Option<PathBuf> {
    std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok().map(PathBuf::from)
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default()
}

// ---------------------------------------------------------------- Prism / MultiMC

/// Minimal INI reader for instance.cfg / prismlauncher.cfg.
fn ini_get(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == key).then(|| v.trim().trim_matches('"').to_string())
    })
}

fn inspect_multimc(dir: &Path, source: &str) -> Option<Candidate> {
    let cfg = std::fs::read_to_string(dir.join("instance.cfg")).ok()?;
    let pack = read_json(&dir.join("mmc-pack.json"))?;
    let comps = pack["components"].as_array()?;
    let ver = |uid: &str| comps.iter().find(|c| c["uid"] == uid).and_then(|c| c["version"].as_str()).map(String::from);
    let mc = ver("net.minecraft")?;
    let (loader, loader_version) = if let Some(v) = ver("net.fabricmc.fabric-loader") {
        ("fabric", Some(v))
    } else if let Some(v) = ver("org.quiltmc.quilt-loader") {
        ("quilt", Some(v))
    } else if let Some(v) = ver("net.neoforged") {
        ("neoforge", Some(v))
    } else if let Some(v) = ver("net.minecraftforge") {
        ("forge", Some(forge_version(&v, &mc)))
    } else {
        ("vanilla", None)
    };
    let game_dir = [".minecraft", "minecraft"].iter().map(|d| dir.join(d)).find(|p| p.is_dir()).unwrap_or_else(|| dir.join(".minecraft"));
    let on = |k: &str| ini_get(&cfg, k).map(|v| v == "true").unwrap_or(false);
    Some(Candidate {
        source: source.into(),
        source_label: label(source).into(),
        path: dir.to_string_lossy().into(),
        game_dir: game_dir.to_string_lossy().into(),
        name: ini_get(&cfg, "name").unwrap_or_else(|| dir.file_name().unwrap_or_default().to_string_lossy().into()),
        game_version: mc,
        loader: loader.into(),
        loader_version,
        playtime_secs: ini_get(&cfg, "totalTimePlayed").and_then(|v| v.parse().ok()).unwrap_or(0),
        last_played: ini_get(&cfg, "lastLaunchTime").and_then(|v| v.parse::<i64>().ok()).filter(|ms| *ms > 0).and_then(|ms| Utc.timestamp_millis_opt(ms).single()),
        memory_mb: if on("OverrideMemory") { ini_get(&cfg, "MaxMemAlloc").and_then(|v| v.parse().ok()) } else { None },
        jvm_args: if on("OverrideJavaArgs") { ini_get(&cfg, "JvmArgs").filter(|v| !v.is_empty()) } else { None },
        already_imported: false,
    })
}

fn prism_instance_dirs() -> Vec<(PathBuf, &'static str)> {
    let mut out = Vec::new();
    for base in data_dirs() {
        for (name, cfg_name, source) in [("PrismLauncher", "prismlauncher.cfg", "prism"), ("PolyMC", "polymc.cfg", "multimc")] {
            let root = base.join(name);
            if !root.is_dir() {
                continue;
            }
            // The instances folder can be moved in Prism's settings.
            let custom = std::fs::read_to_string(root.join(cfg_name)).ok().and_then(|t| ini_get(&t, "InstanceDir"));
            let dir = match custom {
                Some(d) if Path::new(&d).is_absolute() => PathBuf::from(d),
                Some(d) => root.join(d),
                None => root.join("instances"),
            };
            out.push((dir, source));
        }
    }
    out
}

// ---------------------------------------------------------------- ATLauncher

fn inspect_atlauncher(dir: &Path) -> Option<Candidate> {
    let v = read_json(&dir.join("instance.json"))?;
    let launcher = v.get("launcher")?;
    let mc = v["id"].as_str()?.to_string();
    let lv = &launcher["loaderVersion"];
    let (loader, loader_version) = match lv["type"].as_str().map(str::to_lowercase).as_deref() {
        Some("fabric") => ("fabric", lv["version"].as_str().map(String::from)),
        Some("quilt") => ("quilt", lv["version"].as_str().map(String::from)),
        Some("neoforge") => ("neoforge", lv["version"].as_str().map(String::from)),
        Some("forge") => ("forge", lv["version"].as_str().map(|x| forge_version(x, &mc))),
        _ => ("vanilla", None),
    };
    let last_played = match &launcher["lastPlayed"] {
        Value::String(s) => DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc)),
        Value::Object(o) => o.get("seconds").and_then(|s| s.as_i64()).and_then(|s| Utc.timestamp_opt(s, 0).single()),
        _ => None,
    }
    .filter(|d| d.timestamp() > 0);
    Some(Candidate {
        source: "atlauncher".into(),
        source_label: label("atlauncher").into(),
        path: dir.to_string_lossy().into(),
        game_dir: dir.to_string_lossy().into(),
        name: launcher["name"].as_str().map(String::from).unwrap_or_else(|| dir.file_name().unwrap_or_default().to_string_lossy().into()),
        game_version: mc,
        loader: loader.into(),
        loader_version,
        playtime_secs: 0,
        last_played,
        memory_mb: launcher["maximumMemory"].as_u64().map(|m| m as u32),
        jvm_args: launcher["javaArguments"].as_str().filter(|s| !s.trim().is_empty()).map(String::from),
        already_imported: false,
    })
}

// ---------------------------------------------------------------- CurseForge app

/// "forge-47.3.0", "fabric-0.15.11-1.20.1", "neoforge-21.1.77" -> loader + version.
pub fn parse_cf_loader(name: &str, mc: &str) -> (&'static str, Option<String>) {
    let Some((kind, rest)) = name.split_once('-') else { return ("vanilla", None) };
    let rest = rest.strip_suffix(&format!("-{mc}")).unwrap_or(rest).to_string();
    match kind.to_lowercase().as_str() {
        "forge" => ("forge", Some(forge_version(&rest, mc))),
        "neoforge" => ("neoforge", Some(rest)),
        "fabric" => ("fabric", Some(rest)),
        "quilt" => ("quilt", Some(rest)),
        _ => ("vanilla", None),
    }
}

fn inspect_curseforge(dir: &Path) -> Option<Candidate> {
    let v = read_json(&dir.join("minecraftinstance.json"))?;
    let mc = v["gameVersion"].as_str().or_else(|| v["baseModLoader"]["minecraftVersion"].as_str())?.to_string();
    let (loader, loader_version) = v["baseModLoader"]["name"].as_str().map(|n| parse_cf_loader(n, &mc)).unwrap_or(("vanilla", None));
    Some(Candidate {
        source: "curseforge".into(),
        source_label: label("curseforge").into(),
        path: dir.to_string_lossy().into(),
        game_dir: dir.to_string_lossy().into(),
        name: v["name"].as_str().map(String::from).unwrap_or_else(|| dir.file_name().unwrap_or_default().to_string_lossy().into()),
        game_version: mc,
        loader: loader.into(),
        loader_version,
        playtime_secs: 0,
        last_played: v["lastPlayed"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&Utc)).filter(|d| d.timestamp() > 0),
        memory_mb: if v["isMemoryOverride"].as_bool().unwrap_or(false) { v["allocatedMemory"].as_u64().map(|m| m as u32) } else { None },
        jvm_args: v["javaArgsOverride"].as_str().filter(|s| !s.trim().is_empty()).map(String::from),
        already_imported: false,
    })
}

// ---------------------------------------------------------------- Modrinth App (older profiles)

fn inspect_modrinth(dir: &Path) -> Option<Candidate> {
    let v = read_json(&dir.join("profile.json"))?;
    let meta = &v["metadata"];
    let mc = meta["game_version"].as_str()?.to_string();
    let loader = match meta["loader"].as_str().unwrap_or("vanilla") {
        "fabric" => "fabric",
        "quilt" => "quilt",
        "forge" => "forge",
        "neoforge" => "neoforge",
        _ => "vanilla",
    };
    let lv = meta["loader_version"]["id"].as_str().map(|v| if loader == "forge" { forge_version(v, &mc) } else { v.to_string() });
    Some(Candidate {
        source: "modrinth".into(),
        source_label: label("modrinth").into(),
        path: dir.to_string_lossy().into(),
        game_dir: dir.to_string_lossy().into(),
        name: meta["name"].as_str().map(String::from).unwrap_or_else(|| dir.file_name().unwrap_or_default().to_string_lossy().into()),
        game_version: mc,
        loader: loader.into(),
        loader_version: if loader == "vanilla" { None } else { lv },
        playtime_secs: 0,
        last_played: None,
        memory_mb: None,
        jvm_args: None,
        already_imported: false,
    })
}

// ---------------------------------------------------------------- official launcher

/// "fabric-loader-0.16.9-1.21.4", "1.20.1-forge-47.3.0", "quilt-loader-0.27-1.21.1", "neoforge-21.1.77", "1.21.4".
pub fn parse_version_id(id: &str) -> (String, &'static str, Option<String>) {
    if let Some(rest) = id.strip_prefix("fabric-loader-") {
        if let Some((lv, mc)) = rest.rsplit_once('-') {
            return (mc.into(), "fabric", Some(lv.into()));
        }
    }
    if let Some(rest) = id.strip_prefix("quilt-loader-") {
        if let Some((lv, mc)) = rest.rsplit_once('-') {
            return (mc.into(), "quilt", Some(lv.into()));
        }
    }
    if let Some((mc, fv)) = id.split_once("-forge-") {
        return (mc.into(), "forge", Some(forge_version(fv, mc)));
    }
    (id.into(), "vanilla", None)
}

/// Folders worth bringing over from the shared official .minecraft folder
/// (not versions/libraries/assets, which Peak manages itself).
const OFFICIAL_KEEP: &[&str] = &["saves", "resourcepacks", "shaderpacks", "screenshots", "options.txt", "servers.dat", "config", "mods"];

async fn official(latest_release: Option<String>) -> Vec<Candidate> {
    let mut out = Vec::new();
    let roots: Vec<PathBuf> = [
        std::env::var("APPDATA").ok().map(|a| PathBuf::from(a).join(".minecraft")),
        home().map(|h| h.join("Library/Application Support/minecraft")),
        home().map(|h| h.join(".minecraft")),
    ]
    .into_iter()
    .flatten()
    .filter(|p| p.join("launcher_profiles.json").exists())
    .collect();
    let mut seen = HashSet::new();
    for root in roots {
        let Some(v) = read_json(&root.join("launcher_profiles.json")) else { continue };
        let Some(profiles) = v["profiles"].as_object() else { continue };
        for p in profiles.values() {
            let id = p["lastVersionId"].as_str().unwrap_or("latest-release");
            let id = match id {
                "latest-release" | "latest-snapshot" => match &latest_release {
                    Some(r) => r.clone(),
                    None => continue,
                },
                other => other.to_string(),
            };
            let game_dir = p["gameDir"].as_str().map(PathBuf::from).unwrap_or_else(|| root.clone());
            // Profiles sharing the main folder are one import.
            let key = (game_dir.clone(), id.clone());
            if !seen.insert(key) || !game_dir.is_dir() {
                continue;
            }
            let (mc, loader, lv) = parse_version_id(&id);
            let name = p["name"].as_str().filter(|n| !n.is_empty()).map(String::from).unwrap_or_else(|| {
                if game_dir == root { format!("Minecraft {mc}") } else { game_dir.file_name().unwrap_or_default().to_string_lossy().into() }
            });
            out.push(Candidate {
                source: "official".into(),
                source_label: label("official").into(),
                path: game_dir.to_string_lossy().into(),
                game_dir: game_dir.to_string_lossy().into(),
                name,
                game_version: mc,
                loader: loader.into(),
                loader_version: lv,
                playtime_secs: 0,
                last_played: p["lastUsed"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&Utc)),
                memory_mb: None,
                jvm_args: p["javaArgs"].as_str().filter(|s| !s.trim().is_empty()).map(String::from),
                already_imported: false,
            });
        }
    }
    out
}

// ---------------------------------------------------------------- detection

/// Identify a single instance folder picked by the user.
pub fn inspect(dir: &Path) -> Option<Candidate> {
    if dir.join("mmc-pack.json").exists() {
        return inspect_multimc(dir, "multimc");
    }
    inspect_atlauncher(dir).or_else(|| inspect_curseforge(dir)).or_else(|| inspect_modrinth(dir))
}

/// Find instances from every launcher installed in its default location.
pub async fn detect(paths: &Paths, latest_release: Option<String>) -> Result<Vec<Candidate>> {
    let found = tokio::task::spawn_blocking(|| {
        let mut out = Vec::new();
        for (dir, source) in prism_instance_dirs() {
            out.extend(subdirs(&dir).iter().filter_map(|d| inspect_multimc(d, source)));
        }
        for base in data_dirs() {
            for name in ["ATLauncher", "atlauncher"] {
                out.extend(subdirs(&base.join(name).join("instances")).iter().filter_map(|d| inspect_atlauncher(d)));
            }
            for name in ["ModrinthApp", "com.modrinth.theseus"] {
                out.extend(subdirs(&base.join(name).join("profiles")).iter().filter_map(|d| inspect_modrinth(d)));
            }
        }
        if let Some(h) = home() {
            for p in [h.join("curseforge/minecraft/Instances"), h.join("Documents/curseforge/minecraft/Instances")] {
                out.extend(subdirs(&p).iter().filter_map(|d| inspect_curseforge(d)));
            }
        }
        out
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))?;

    let mut all = found;
    all.extend(official(latest_release).await);

    // Same folder found twice (e.g. ATLauncher and atlauncher on Windows).
    let mut seen = HashSet::new();
    all.retain(|c| seen.insert(c.path.to_lowercase()));

    let imported: HashSet<String> = instances::list(paths).await?.into_iter().filter_map(|i| i.imported_from).map(|p| p.to_lowercase()).collect();
    for c in &mut all {
        c.already_imported = imported.contains(&c.path.to_lowercase());
    }
    all.sort_by(|a, b| b.last_played.cmp(&a.last_played).then(a.name.cmp(&b.name)));
    Ok(all)
}

// ---------------------------------------------------------------- import

/// Never copy these: logs, caches and launcher-specific bookkeeping.
const SKIP: &[&str] = &["logs", "crash-reports", ".cache", "natives", "instance.json", "minecraftinstance.json", "profile.json", ".fabric", "downloads"];

fn collect(src: &Path, rel: &Path, only_top: Option<&[&str]>, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(src.join(rel)) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let r = rel.join(&name);
        if rel.as_os_str().is_empty() {
            if SKIP.contains(&name.as_str()) {
                continue;
            }
            if let Some(keep) = only_top {
                if !keep.contains(&name.as_str()) {
                    continue;
                }
            }
        }
        match e.file_type() {
            Ok(t) if t.is_dir() => collect(src, &r, None, out),
            Ok(t) if t.is_file() => out.push(r),
            _ => {}
        }
    }
}

/// Create the Peak instance for a candidate (files are copied by [`copy_files`]).
pub async fn create(paths: &Paths, c: &Candidate) -> Result<Instance> {
    let mut inst = instances::create(paths, &c.name, &c.game_version, &c.loader, c.loader_version.clone()).await?;
    inst.playtime_secs = c.playtime_secs;
    inst.last_played = c.last_played;
    inst.memory_mb = c.memory_mb;
    inst.jvm_args = c.jvm_args.clone();
    inst.imported_from = Some(c.path.clone());
    inst.notes = format!("Imported from {} ({})", c.source_label, c.path);
    instances::save(paths, &inst).await?;
    Ok(inst)
}

/// Copy the candidate's game files into the instance.
pub async fn copy_files(paths: &Paths, c: &Candidate, inst: &Instance, progress: &ProgressFn) -> Result<()> {
    let src = PathBuf::from(&c.game_dir);
    let dst = instances::game_dir(paths, &inst.id);
    let official = c.source == "official";
    let atl_disabled = (c.source == "atlauncher").then(|| PathBuf::from(&c.path).join("disabledmods"));
    let progress = progress.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let mut files = Vec::new();
        collect(&src, Path::new(""), official.then_some(OFFICIAL_KEEP), &mut files);
        let total = files.len();
        for (i, rel) in files.iter().enumerate() {
            let to = dst.join(rel);
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(src.join(rel), &to)?;
            if i % 25 == 0 || i + 1 == total {
                progress(InstallProgress { stage: "Copying files", done: i + 1, total, bytes: 0 });
            }
        }
        // ATLauncher keeps disabled mods in their own folder; Peak marks them
        // with a .disabled suffix inside mods/.
        if let Some(dis) = atl_disabled {
            if let Ok(rd) = std::fs::read_dir(&dis) {
                std::fs::create_dir_all(dst.join("mods"))?;
                for e in rd.flatten().filter(|e| e.path().is_file()) {
                    let name = format!("{}.disabled", e.file_name().to_string_lossy());
                    std::fs::copy(e.path(), dst.join("mods").join(name))?;
                }
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("peak-import-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Read-only: list what this computer's launchers have.
    /// cargo test -- --ignored --nocapture detect_this_pc
    #[tokio::test]
    #[ignore]
    async fn detect_this_pc() {
        let paths = Paths::new(std::env::temp_dir().join(format!("peak-detect-{}", std::process::id())));
        for c in detect(&paths, Some("1.21.4".into())).await.unwrap() {
            println!("{:<20} {:<30} {} {} {:?}  <- {}", c.source_label, c.name, c.game_version, c.loader, c.loader_version, c.path);
        }
    }

    #[test]
    fn reads_prism_instance() {
        let d = tmp("prism");
        std::fs::write(d.join("instance.cfg"), "[General]\nname=My Pack\nOverrideMemory=true\nMaxMemAlloc=6144\ntotalTimePlayed=3600\nlastLaunchTime=1700000000000\n").unwrap();
        std::fs::write(
            d.join("mmc-pack.json"),
            r#"{"components":[{"uid":"net.minecraft","version":"1.20.1"},{"uid":"net.minecraftforge","version":"47.3.0"}],"formatVersion":1}"#,
        )
        .unwrap();
        std::fs::create_dir_all(d.join(".minecraft/saves/World")).unwrap();
        let c = inspect(&d).unwrap();
        assert_eq!((c.name.as_str(), c.game_version.as_str(), c.loader.as_str()), ("My Pack", "1.20.1", "forge"));
        assert_eq!(c.loader_version.as_deref(), Some("1.20.1-47.3.0"));
        assert_eq!(c.memory_mb, Some(6144));
        assert_eq!(c.playtime_secs, 3600);
        assert!(c.game_dir.ends_with(".minecraft"));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn reads_atlauncher_and_curseforge() {
        let d = tmp("atl");
        std::fs::write(
            d.join("instance.json"),
            r#"{"id":"1.21.1","launcher":{"name":"ATL Pack","loaderVersion":{"version":"0.16.5","type":"Fabric"},"maximumMemory":8192}}"#,
        )
        .unwrap();
        let c = inspect(&d).unwrap();
        assert_eq!((c.source.as_str(), c.loader.as_str(), c.loader_version.as_deref()), ("atlauncher", "fabric", Some("0.16.5")));
        assert_eq!(c.memory_mb, Some(8192));
        let _ = std::fs::remove_dir_all(d);

        let d = tmp("cf");
        std::fs::write(
            d.join("minecraftinstance.json"),
            "\u{feff}{\"name\":\"CF Pack\",\"gameVersion\":\"1.20.1\",\"baseModLoader\":{\"name\":\"fabric-0.15.11-1.20.1\"},\"lastPlayed\":\"2024-05-01T10:00:00Z\"}",
        )
        .unwrap();
        let c = inspect(&d).unwrap();
        assert_eq!((c.source.as_str(), c.loader.as_str(), c.loader_version.as_deref()), ("curseforge", "fabric", Some("0.15.11")));
        assert!(c.last_played.is_some());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn parses_official_version_ids() {
        assert_eq!(parse_version_id("fabric-loader-0.16.9-1.21.4"), ("1.21.4".into(), "fabric", Some("0.16.9".into())));
        assert_eq!(parse_version_id("1.20.1-forge-47.3.0"), ("1.20.1".into(), "forge", Some("1.20.1-47.3.0".into())));
        assert_eq!(parse_version_id("1.21.4"), ("1.21.4".into(), "vanilla", None));
        assert_eq!(parse_cf_loader("forge-47.3.0", "1.20.1"), ("forge", Some("1.20.1-47.3.0".into())));
        assert_eq!(parse_cf_loader("neoforge-21.1.77", "1.21.1"), ("neoforge", Some("21.1.77".into())));
    }

    #[tokio::test]
    async fn imports_prism_instance_end_to_end() {
        let src = tmp("e2e-src");
        std::fs::write(src.join("instance.cfg"), "name=Imported\nOverrideMemory=true\nMaxMemAlloc=5120\ntotalTimePlayed=120\n").unwrap();
        std::fs::write(
            src.join("mmc-pack.json"),
            r#"{"components":[{"uid":"net.minecraft","version":"1.21.4"},{"uid":"net.fabricmc.fabric-loader","version":"0.16.9"}]}"#,
        )
        .unwrap();
        let mc = src.join(".minecraft");
        std::fs::create_dir_all(mc.join("saves/W")).unwrap();
        std::fs::write(mc.join("saves/W/level.dat"), b"lvl").unwrap();
        std::fs::create_dir_all(mc.join("mods")).unwrap();
        std::fs::write(mc.join("mods/sodium.jar"), b"jar").unwrap();
        std::fs::create_dir_all(mc.join("logs")).unwrap();
        std::fs::write(mc.join("logs/latest.log"), b"log").unwrap();

        let root = tmp("e2e-peak");
        let paths = Paths::new(root.clone());
        let c = inspect(&src).unwrap();
        let inst = create(&paths, &c).await.unwrap();
        let progress: ProgressFn = std::sync::Arc::new(|_| {});
        copy_files(&paths, &c, &inst, &progress).await.unwrap();

        let game = instances::game_dir(&paths, &inst.id);
        assert!(game.join("saves/W/level.dat").exists());
        assert!(game.join("mods/sodium.jar").exists());
        assert!(!game.join("logs").exists());
        let saved = instances::get(&paths, &inst.id).await.unwrap();
        assert_eq!((saved.loader.as_str(), saved.loader_version.as_deref()), ("fabric", Some("0.16.9")));
        assert_eq!((saved.memory_mb, saved.playtime_secs), (Some(5120), 120));
        assert_eq!(saved.imported_from.as_deref(), Some(src.to_string_lossy().as_ref()));
        // The source is left untouched.
        assert!(mc.join("logs/latest.log").exists() && mc.join("mods/sodium.jar").exists());
        let _ = std::fs::remove_dir_all(src);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn copy_skips_logs_and_keeps_worlds() {
        let src = tmp("copy-src");
        std::fs::create_dir_all(src.join("saves/World/region")).unwrap();
        std::fs::write(src.join("saves/World/level.dat"), b"x").unwrap();
        std::fs::create_dir_all(src.join("logs")).unwrap();
        std::fs::write(src.join("logs/latest.log"), b"x").unwrap();
        std::fs::write(src.join("options.txt"), b"x").unwrap();
        let mut files = Vec::new();
        collect(&src, Path::new(""), None, &mut files);
        let names: Vec<String> = files.iter().map(|p| p.to_string_lossy().replace('\\', "/")).collect();
        assert!(names.contains(&"saves/World/level.dat".to_string()));
        assert!(names.contains(&"options.txt".to_string()));
        assert!(!names.iter().any(|n| n.starts_with("logs")));
        let _ = std::fs::remove_dir_all(src);
    }
}
