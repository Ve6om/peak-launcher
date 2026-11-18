//! Mod loaders: Fabric, Quilt, Forge and NeoForge.
//!
//! Fabric and Quilt publish ready-made launcher profiles. Forge and NeoForge
//! ship an installer jar whose `install_profile.json` lists libraries plus
//! "processors" (small Java programs that patch the game), which we run the
//! same way the official installer does.

use super::install::{self, ProgressFn, stage};
use super::java;
use super::net::{self, Download};
use super::paths::Paths;
use super::versions::{self, Library, maven_path};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const FABRIC_META: &str = "https://meta.fabricmc.net/v2";
pub const QUILT_META: &str = "https://meta.quiltmc.org/v3";
const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge";
const FORGE_PROMOS: &str = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const NEOFORGE_MAVEN: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loader {
    Vanilla,
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

impl Loader {
    pub fn parse(s: &str) -> Result<Loader> {
        Ok(match s {
            "vanilla" => Loader::Vanilla,
            "fabric" => Loader::Fabric,
            "quilt" => Loader::Quilt,
            "forge" => Loader::Forge,
            "neoforge" => Loader::NeoForge,
            other => return Err(Error::msg(format!("Unknown mod loader '{other}'"))),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersion {
    pub id: String,
    pub stable: bool,
    pub recommended: bool,
}

/// Loader versions available for a Minecraft version, newest first.
pub async fn versions(loader: Loader, mc: &str) -> Result<Vec<LoaderVersion>> {
    match loader {
        Loader::Vanilla => Ok(vec![]),
        Loader::Fabric | Loader::Quilt => fabric_like_versions(loader, mc).await,
        Loader::Forge => forge_versions(mc).await,
        Loader::NeoForge => neoforge_versions(mc).await,
    }
}

/// Install the loader for `mc` and return the id of its version profile.
/// Vanilla `mc` must already be installed.
pub async fn install(paths: &Paths, loader: Loader, mc: &str, loader_version: &str, progress: &ProgressFn) -> Result<String> {
    let id = match loader {
        Loader::Vanilla => return Ok(mc.to_string()),
        Loader::Fabric | Loader::Quilt => install_fabric_like(paths, loader, mc, loader_version).await?,
        Loader::Forge | Loader::NeoForge => install_forge_like(paths, loader, mc, loader_version, progress).await?,
    };

    let merged = versions::resolve(paths, &id).await?;
    install::download_libraries(paths, &merged, progress, "Downloading mod loader").await?;

    // The game jar must sit next to the loader profile (Forge's module system
    // matches it by the profile's name).
    let src = paths.version_jar(mc);
    let dst = paths.version_jar(&id);
    let same = match (tokio::fs::metadata(&src).await, tokio::fs::metadata(&dst).await) {
        (Ok(a), Ok(b)) => a.len() == b.len(),
        _ => false,
    };
    if !same {
        tokio::fs::copy(&src, &dst).await?;
    }
    Ok(id)
}

// ---------------------------------------------------------------- Fabric / Quilt

fn meta_base(loader: Loader) -> &'static str {
    if loader == Loader::Quilt { QUILT_META } else { FABRIC_META }
}

async fn fabric_like_versions(loader: Loader, mc: &str) -> Result<Vec<LoaderVersion>> {
    #[derive(Deserialize)]
    struct Entry {
        loader: L,
    }
    #[derive(Deserialize)]
    struct L {
        version: String,
        #[serde(default)]
        stable: Option<bool>,
    }
    let url = format!("{}/versions/loader/{}", meta_base(loader), mc);
    let entries: Vec<Entry> = net::get_json(&url).await?;
    let mut out: Vec<LoaderVersion> = entries
        .into_iter()
        .map(|e| {
            let stable = e.loader.stable.unwrap_or_else(|| !e.loader.version.contains('-'));
            LoaderVersion { id: e.loader.version, stable, recommended: false }
        })
        .collect();
    if let Some(first) = out.iter_mut().find(|v| v.stable) {
        first.recommended = true;
    }
    Ok(out)
}

async fn install_fabric_like(paths: &Paths, loader: Loader, mc: &str, version: &str) -> Result<String> {
    let url = format!("{}/versions/loader/{}/{}/profile/json", meta_base(loader), mc, version);
    let bytes = net::client().get(&url).send().await?.error_for_status()?.bytes().await?;
    let profile: Value = serde_json::from_slice(&bytes)?;
    let id = profile["id"].as_str().ok_or_else(|| Error::msg("Loader profile has no id"))?.to_string();
    write_profile(paths, &id, &bytes).await?;
    Ok(id)
}

async fn write_profile(paths: &Paths, id: &str, bytes: &[u8]) -> Result<()> {
    let path = paths.version_json(id);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(path, bytes).await?;
    Ok(())
}

// ---------------------------------------------------------------- Forge / NeoForge

async fn maven_versions(url: &str) -> Result<Vec<String>> {
    let xml = net::client().get(url).send().await?.error_for_status()?.text().await?;
    Ok(xml
        .split("<version>")
        .skip(1)
        .filter_map(|s| s.split("</version>").next())
        .map(|s| s.trim().to_string())
        .collect())
}

async fn forge_versions(mc: &str) -> Result<Vec<LoaderVersion>> {
    let all = maven_versions(&format!("{FORGE_MAVEN}/maven-metadata.xml")).await?;
    let promos: Value = net::get_json(FORGE_PROMOS).await.unwrap_or(Value::Null);
    let promo = |kind: &str| promos["promos"][format!("{mc}-{kind}")].as_str().map(|v| format!("{mc}-{v}"));
    let recommended = promo("recommended").or_else(|| promo("latest"));
    let prefix = format!("{mc}-");
    // Old versions carry a suffix, e.g. "1.7.10-10.13.4.1614-1.7.10".
    let is_rec = |v: &str| recommended.as_deref().map(|r| v == r || v.starts_with(&format!("{r}-"))).unwrap_or(false);
    let mut out: Vec<LoaderVersion> = all
        .into_iter()
        .filter(|v| v.starts_with(&prefix))
        .map(|v| LoaderVersion { recommended: is_rec(&v), stable: true, id: v })
        .collect();
    out.reverse();
    if !out.iter().any(|v| v.recommended) {
        if let Some(first) = out.first_mut() {
            first.recommended = true;
        }
    }
    Ok(out)
}

/// NeoForge versions encode the game version: 21.1.x -> 1.21.1, 20.4.x -> 1.20.4,
/// 21.0.x -> 1.21. Year-based game versions (26.1 and later) keep the year.
fn neoforge_mc(v: &str) -> Option<String> {
    let mut parts = v.split(|c: char| c == '.' || c == '-');
    let a: u32 = parts.next()?.parse().ok()?;
    let b: u32 = parts.next()?.parse().ok()?;
    if a < 25 {
        Some(if b == 0 { format!("1.{a}") } else { format!("1.{a}.{b}") })
    } else {
        let c: u32 = parts.next()?.parse().ok()?;
        Some(if c == 0 { format!("{a}.{b}") } else { format!("{a}.{b}.{c}") })
    }
}

async fn neoforge_versions(mc: &str) -> Result<Vec<LoaderVersion>> {
    let all = maven_versions(&format!("{NEOFORGE_MAVEN}/maven-metadata.xml")).await?;
    let mut out: Vec<LoaderVersion> = all
        .into_iter()
        .filter(|v| neoforge_mc(v).as_deref() == Some(mc))
        .map(|v| LoaderVersion { stable: !v.contains("beta") && !v.contains("alpha"), recommended: false, id: v })
        .collect();
    out.reverse();
    if let Some(first) = out.iter_mut().find(|v| v.stable) {
        first.recommended = true;
    }
    Ok(out)
}

fn read_entry(zip_path: &Path, name: &str) -> Result<Option<Vec<u8>>> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(zip_path)?)?;
    let name = name.trim_start_matches('/');
    let Ok(mut entry) = zip.by_name(name) else { return Ok(None) };
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buf)?;
    Ok(Some(buf))
}

async fn read_entry_async(zip_path: &Path, name: &str) -> Result<Option<Vec<u8>>> {
    let (p, n) = (zip_path.to_owned(), name.to_owned());
    tokio::task::spawn_blocking(move || read_entry(&p, &n)).await.map_err(|e| Error::msg(e.to_string()))?
}

async fn extract_to(zip_path: &Path, name: &str, dest: &Path) -> Result<bool> {
    match read_entry_async(zip_path, name).await? {
        Some(bytes) => {
            if let Some(parent) = dest.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(dest, bytes).await?;
            Ok(true)
        }
        None => Ok(false),
    }
}

/// The Forge/NeoForge installer jar for `version`, downloaded once and kept
/// in the cache (also used to build server packs).
pub async fn forge_installer(paths: &Paths, loader: Loader, version: &str) -> Result<PathBuf> {
    let (base, name) = if loader == Loader::Forge { (FORGE_MAVEN, "forge") } else { (NEOFORGE_MAVEN, "neoforge") };
    let file = format!("{name}-{version}-installer.jar");
    let installer = paths.cache().join("installers").join(&file);
    net::download_all(vec![Download::new(format!("{base}/{version}/{file}"), installer.clone(), None, None)], 1, |_| {})
        .await
        .map_err(|e| Error::msg(format!("Couldn't download the {name} installer: {e}")))?;
    Ok(installer)
}

async fn install_forge_like(paths: &Paths, loader: Loader, mc: &str, version: &str, progress: &ProgressFn) -> Result<String> {
    let installer = forge_installer(paths, loader, version).await?;

    let profile_bytes = read_entry_async(&installer, "install_profile.json")
        .await?
        .ok_or_else(|| Error::msg("This installer has no install profile (versions before 1.6 aren't supported)"))?;
    let profile: Value = serde_json::from_slice(&profile_bytes)?;

    // Legacy installers (1.12.2 and older): the profile embeds the version JSON
    // and a "universal" jar that just needs to be copied into libraries.
    if let Some(info) = profile.get("versionInfo") {
        let id = info["id"].as_str().ok_or_else(|| Error::msg("Forge profile has no id"))?.to_string();
        write_profile(paths, &id, &serde_json::to_vec_pretty(info)?).await?;
        let coord = profile["install"]["path"].as_str().ok_or_else(|| Error::msg("Forge profile has no install path"))?;
        let jar = profile["install"]["filePath"].as_str().ok_or_else(|| Error::msg("Forge profile has no file path"))?;
        let dest = paths.libraries().join(maven_path(coord).ok_or_else(|| Error::msg("Bad Forge coordinate"))?);
        if !extract_to(&installer, jar, &dest).await? {
            return Err(Error::msg("The Forge installer is missing its universal jar"));
        }
        return Ok(id);
    }

    // Modern installers.
    let json_name = profile["json"].as_str().unwrap_or("/version.json");
    let version_bytes = read_entry_async(&installer, json_name)
        .await?
        .ok_or_else(|| Error::msg("The installer is missing its version profile"))?;
    let version_json: Value = serde_json::from_slice(&version_bytes)?;
    let id = version_json["id"].as_str().ok_or_else(|| Error::msg("Loader profile has no id"))?.to_string();
    write_profile(paths, &id, &version_bytes).await?;

    // Libraries for the profile and for the processors. Ones without a URL
    // are bundled inside the installer under maven/.
    let mut libs: Vec<Library> = serde_json::from_value(profile["libraries"].clone()).unwrap_or_default();
    libs.extend(serde_json::from_value::<Vec<Library>>(version_json["libraries"].clone()).unwrap_or_default());
    let mut downloads = Vec::new();
    for lib in &libs {
        let Some(a) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) else { continue };
        let Some(rel) = a.path.clone().or_else(|| maven_path(&lib.name)) else { continue };
        let dest = paths.libraries().join(&rel);
        if a.url.is_empty() {
            if !dest.exists() {
                extract_to(&installer, &format!("maven/{rel}"), &dest).await?;
            }
        } else {
            downloads.push(Download::new(install::fix_url(&a.url), dest, a.sha1.clone(), a.size));
        }
    }
    downloads.sort_by(|a, b| a.path.cmp(&b.path));
    downloads.dedup_by(|a, b| a.path == b.path);
    net::download_all(downloads, net::concurrency(), stage(progress, "Downloading mod loader")).await?;

    run_processors(paths, &profile, &installer, mc, &id, progress).await?;
    Ok(id)
}

fn lib_path(paths: &Paths, coord: &str) -> Result<PathBuf> {
    Ok(paths.libraries().join(maven_path(coord).ok_or_else(|| Error::msg(format!("Bad library coordinate {coord}")))?))
}

/// Output that means the JVM never got going (as opposed to the tool failing).
fn java_failed_to_start(out: &std::process::Output) -> bool {
    let text = String::from_utf8_lossy(&out.stderr).to_string() + &String::from_utf8_lossy(&out.stdout);
    text.contains("Error occurred during initialization of VM")
        || text.contains("Error: could not open")
        || text.contains("Could not create the Java Virtual Machine")
}

async fn run_processors(paths: &Paths, profile: &Value, installer: &Path, mc: &str, id: &str, progress: &ProgressFn) -> Result<()> {
    let processors: Vec<Value> = profile["processors"].as_array().cloned().unwrap_or_default();
    let processors: Vec<Value> = processors
        .into_iter()
        .filter(|p| match p["sides"].as_array() {
            Some(sides) => sides.iter().any(|s| s == "client"),
            None => true,
        })
        .collect();
    if processors.is_empty() {
        return Ok(());
    }

    let tmp = paths.cache().join("loader-data").join(id);
    tokio::fs::create_dir_all(&tmp).await?;

    let mut data: HashMap<String, String> = HashMap::new();
    if let Some(obj) = profile["data"].as_object() {
        for (key, v) in obj {
            let Some(val) = v["client"].as_str() else { continue };
            let resolved = if val.starts_with('[') && val.ends_with(']') {
                lib_path(paths, &val[1..val.len() - 1])?.to_string_lossy().into_owned()
            } else if val.starts_with('\'') && val.ends_with('\'') {
                val[1..val.len() - 1].to_string()
            } else if let Some(entry) = val.strip_prefix('/') {
                let dest = tmp.join(entry);
                extract_to(installer, entry, &dest).await?;
                dest.to_string_lossy().into_owned()
            } else {
                val.to_string()
            };
            data.insert(key.clone(), resolved);
        }
    }
    let s = |p: PathBuf| p.to_string_lossy().into_owned();
    data.insert("SIDE".into(), "client".into());
    data.insert("MINECRAFT_JAR".into(), s(paths.version_jar(mc)));
    data.insert("MINECRAFT_VERSION".into(), mc.into());
    data.insert("ROOT".into(), s(paths.root.clone()));
    data.insert("INSTALLER".into(), s(installer.to_path_buf()));
    data.insert("LIBRARY_DIR".into(), s(paths.libraries()));

    let subst = |arg: &str| -> Result<String> {
        if arg.starts_with('[') && arg.ends_with(']') {
            return Ok(s(lib_path(paths, &arg[1..arg.len() - 1])?));
        }
        let mut out = String::new();
        let mut rest = arg;
        while let Some(start) = rest.find('{') {
            out.push_str(&rest[..start]);
            match rest[start..].find('}') {
                Some(end) => {
                    let key = &rest[start + 1..start + end];
                    match data.get(key) {
                        Some(v) => out.push_str(v),
                        None => out.push_str(&rest[start..=start + end]),
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
        Ok(out)
    };

    // Processors run on the game's own Java runtime.
    let vanilla = versions::load_version_json(paths, mc).await?;
    let component = vanilla.java_version.as_ref().map(|j| j.component.clone()).unwrap_or_else(|| java::LEGACY_COMPONENT.into());
    let java_exe = java::console_java(&java::ensure_runtime(paths, &component, false, |_| {}).await?);
    let sep = if cfg!(windows) { ";" } else { ":" };

    let total = processors.len();
    for (i, p) in processors.iter().enumerate() {
        progress(install::InstallProgress { stage: "Setting up mod loader", done: i, total, bytes: 0 });

        // Skip work whose outputs already exist with the expected hashes.
        if let Some(outputs) = p["outputs"].as_object().filter(|o| !o.is_empty()) {
            let mut all_ok = true;
            for (k, v) in outputs {
                let path = PathBuf::from(subst(k)?);
                let expected = subst(v.as_str().unwrap_or(""))?.trim_matches('\'').to_string();
                let ok = path.exists() && net::sha1_file(&path).await.map(|h| h.eq_ignore_ascii_case(&expected)).unwrap_or(false);
                if !ok {
                    all_ok = false;
                    break;
                }
            }
            if all_ok {
                continue;
            }
        }

        let jar_coord = p["jar"].as_str().ok_or_else(|| Error::msg("Processor without a jar"))?;
        let jar = lib_path(paths, jar_coord)?;
        let manifest = read_entry_async(&jar, "META-INF/MANIFEST.MF").await?.unwrap_or_default();
        let manifest = String::from_utf8_lossy(&manifest);
        let main_class = manifest
            .lines()
            .find_map(|l| l.strip_prefix("Main-Class:"))
            .map(|s| s.trim().to_string())
            .ok_or_else(|| Error::msg(format!("Processor {jar_coord} has no main class")))?;

        let mut cp = vec![s(jar)];
        for c in p["classpath"].as_array().into_iter().flatten() {
            if let Some(c) = c.as_str() {
                cp.push(s(lib_path(paths, c)?));
            }
        }
        let mut args = vec!["-cp".to_string(), cp.join(sep), main_class];
        for a in p["args"].as_array().into_iter().flatten() {
            if let Some(a) = a.as_str() {
                args.push(subst(a)?);
            }
        }

        let run = || async {
            let mut cmd = tokio::process::Command::new(&java_exe);
            cmd.args(&args).current_dir(&tmp);
            #[cfg(windows)]
            cmd.creation_flags(0x0800_0000);
            cmd.output().await.map_err(|e| Error::msg(format!("Couldn't run Java: {e}")))
        };
        let mut out = run().await?;
        if !out.status.success() && java_failed_to_start(&out) {
            // Java itself couldn't start, usually a damaged runtime file:
            // check every file, replace bad ones, and try once more.
            progress(install::InstallProgress { stage: "Repairing Java", done: 0, total: 0, bytes: 0 });
            java::ensure_runtime(paths, &component, true, |_| {}).await?;
            out = run().await?;
        }
        if !out.status.success() {
            let text = String::from_utf8_lossy(&out.stderr).to_string() + &String::from_utf8_lossy(&out.stdout);
            let tail: Vec<&str> = text.lines().rev().take(12).collect::<Vec<_>>().into_iter().rev().collect();
            return Err(Error::msg(format!("Mod loader setup failed ({jar_coord}):\n{}", tail.join("\n"))));
        }
    }
    progress(install::InstallProgress { stage: "Setting up mod loader", done: total, total, bytes: 0 });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::neoforge_mc;

    #[test]
    fn neoforge_versions_map_to_minecraft() {
        assert_eq!(neoforge_mc("21.1.77").as_deref(), Some("1.21.1"));
        assert_eq!(neoforge_mc("21.0.167").as_deref(), Some("1.21"));
        assert_eq!(neoforge_mc("20.4.237-beta").as_deref(), Some("1.20.4"));
        assert_eq!(neoforge_mc("26.1.0.5-beta").as_deref(), Some("26.1"));
    }
}
