//! Installs a vanilla game version: version JSON, client jar, libraries,
//! natives, assets, logging config and the matching Java runtime.

use super::java;
use super::net::{self, Download};
use super::paths::{self, Paths};
use super::rules;
use super::versions::{self, AssetIndex, VersionJson, maven_path};
use crate::error::{Error, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const ASSET_BASE: &str = "https://resources.download.minecraft.net";
const LIBRARY_BASE: &str = "https://libraries.minecraft.net";

#[derive(Debug, Clone, Serialize)]
pub struct InstallProgress {
    pub stage: &'static str,
    pub done: usize,
    pub total: usize,
    pub bytes: u64,
}

pub type ProgressFn = Arc<dyn Fn(InstallProgress) + Send + Sync>;

pub fn stage(cb: &ProgressFn, stage: &'static str) -> impl Fn(net::Progress) + Send + Sync + 'static {
    let cb = cb.clone();
    move |p| cb(InstallProgress { stage, done: p.done, total: p.total, bytes: p.bytes })
}

/// Fetch (or reuse) the version JSON for `id`.
async fn fetch_version_json(paths: &Paths, id: &str) -> Result<VersionJson> {
    let manifest = versions::manifest(paths).await?;
    match manifest.versions.iter().find(|v| v.id == id) {
        Some(entry) => {
            let dl = Download::new(entry.url.clone(), paths.version_json(id), entry.sha1.clone(), None);
            net::download_all(vec![dl], 1, |_| {}).await?;
        }
        None if paths.version_json(id).exists() => {}
        None => return Err(Error::msg(format!("Unknown Minecraft version {id}"))),
    }
    versions::load_version_json(paths, id).await
}

/// Old Forge profiles point at Maven hosts that moved or are plain HTTP.
pub fn fix_url(url: &str) -> String {
    url.replace("http://files.minecraftforge.net/maven", "https://maven.minecraftforge.net")
        .replace("https://files.minecraftforge.net/maven", "https://maven.minecraftforge.net")
        .replacen("http://", "https://", 1)
}

/// Path of a library's main artifact, if it has one for this platform. The
/// download has an empty URL when the file ships inside a loader installer.
pub fn library_artifact(paths: &Paths, lib: &versions::Library) -> Option<(PathBuf, Download)> {
    if !rules::allowed(lib.rules.as_deref()) {
        return None;
    }
    if let Some(a) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
        let rel = a.path.clone().or_else(|| maven_path(&lib.name))?;
        let path = paths.libraries().join(&rel);
        return Some((path.clone(), Download::new(fix_url(&a.url), path, a.sha1.clone(), a.size)));
    }
    // Maven-style entry without a downloads block (Fabric, Quilt, old Forge).
    if lib.downloads.is_none() && lib.natives.is_none() {
        let rel = maven_path(&lib.name)?;
        let base = lib.url.clone().filter(|u| !u.is_empty()).unwrap_or_else(|| LIBRARY_BASE.to_string());
        let url = format!("{}/{}", fix_url(base.trim_end_matches('/')), rel);
        let path = paths.libraries().join(&rel);
        return Some((path.clone(), Download::new(url, path, lib.sha1.clone(), lib.size)));
    }
    None
}

/// Download every library of a (merged) version that isn't on disk yet.
pub async fn download_libraries(paths: &Paths, version: &VersionJson, progress: &ProgressFn, label: &'static str) -> Result<()> {
    let mut files: Vec<Download> = version
        .libraries
        .iter()
        .filter_map(|l| library_artifact(paths, l).map(|(_, d)| d))
        .filter(|d| !d.url.is_empty() && !d.url.ends_with('/'))
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files.dedup_by(|a, b| a.path == b.path);
    net::download_all(files, net::concurrency(), stage(progress, label)).await
}

/// Old-style natives jar (pre-1.19) for this platform, if any.
pub fn library_natives(paths: &Paths, lib: &versions::Library) -> Option<(PathBuf, Download)> {
    if !rules::allowed(lib.rules.as_deref()) {
        return None;
    }
    let classifier = lib.native_classifier()?;
    let a = lib.downloads.as_ref()?.classifiers.as_ref()?.get(&classifier)?;
    let rel = a.path.clone().or_else(|| maven_path(&format!("{}:{}", lib.name, classifier)))?;
    let path = paths.libraries().join(&rel);
    Some((path.clone(), Download::new(a.url.clone(), path, a.sha1.clone(), a.size)))
}

pub fn natives_dir(paths: &Paths, version: &str) -> PathBuf {
    paths.version_dir(version).join("natives")
}

pub fn log_config_path(paths: &Paths, file_id: &str) -> PathBuf {
    paths.assets().join("log_configs").join(file_id)
}

/// Install everything needed to launch `version_id`. Safe to call repeatedly:
/// files that are already present and valid are skipped.
pub async fn install_version(paths: &Paths, version_id: &str, progress: ProgressFn) -> Result<VersionJson> {
    progress(InstallProgress { stage: "Preparing", done: 0, total: 0, bytes: 0 });
    let version = fetch_version_json(paths, version_id).await?;

    let mut files = Vec::new();
    if let Some(client) = version.downloads.as_ref().and_then(|d| d.client.as_ref()) {
        files.push(Download::new(client.url.clone(), paths.version_jar(version_id), client.sha1.clone(), client.size));
    } else {
        return Err(Error::msg("This version has no client download"));
    }

    let mut native_jars = Vec::new();
    for lib in &version.libraries {
        if let Some((_, dl)) = library_artifact(paths, lib) {
            files.push(dl);
        }
        if let Some((path, dl)) = library_natives(paths, lib) {
            native_jars.push(path);
            files.push(dl);
        }
    }

    if let Some(log) = version.logging.as_ref().and_then(|l| l.client.as_ref()) {
        files.push(Download::new(
            log.file.url.clone(),
            log_config_path(paths, &log.file.id),
            log.file.sha1.clone(),
            log.file.size,
        ));
    }

    // Asset index, then every asset object it lists.
    let mut asset_index = None;
    if let Some(ai) = &version.asset_index {
        let index_path = paths.assets().join("indexes").join(format!("{}.json", ai.id));
        net::download_all(vec![Download::new(ai.url.clone(), index_path.clone(), ai.sha1.clone(), ai.size)], 1, |_| {})
            .await?;
        let index: AssetIndex = paths::read_json(&index_path).await?.ok_or_else(|| Error::msg("Missing asset index"))?;
        for obj in index.objects.values() {
            let rel = obj.rel_path();
            files.push(Download::new(
                format!("{ASSET_BASE}/{rel}"),
                paths.assets().join("objects").join(&rel),
                Some(obj.hash.clone()),
                Some(obj.size),
            ));
        }
        asset_index = Some((ai.id.clone(), index));
    }

    // De-duplicate (some versions list the same library twice).
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files.dedup_by(|a, b| a.path == b.path);

    net::download_all(files, net::concurrency(), stage(&progress, "Downloading game files")).await?;

    if let Some((id, index)) = &asset_index {
        if index.is_virtual {
            copy_virtual_assets(paths, index, &paths.assets().join("virtual").join(id)).await?;
        }
    }

    extract_natives(&native_jars, &natives_dir(paths, version_id)).await?;

    let component = version.java_version.as_ref().map(|j| j.component.as_str()).unwrap_or(java::LEGACY_COMPONENT);
    java::ensure_runtime(paths, component, true, stage(&progress, "Downloading Java")).await?;

    Ok(version)
}

/// Very old versions read assets by their original names instead of hashes.
pub async fn copy_virtual_assets(paths: &Paths, index: &AssetIndex, target: &Path) -> Result<()> {
    for (name, obj) in &index.objects {
        let dest = target.join(name);
        if dest.exists() {
            continue;
        }
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::copy(paths.assets().join("objects").join(obj.rel_path()), &dest).await?;
    }
    Ok(())
}

async fn extract_natives(jars: &[PathBuf], target: &Path) -> Result<()> {
    if jars.is_empty() {
        return Ok(());
    }
    let jars = jars.to_vec();
    let target = target.to_owned();
    tokio::task::spawn_blocking(move || -> Result<()> {
        std::fs::create_dir_all(&target)?;
        for jar in jars {
            let mut zip = zip::ZipArchive::new(std::fs::File::open(&jar)?)?;
            for i in 0..zip.len() {
                let mut entry = zip.by_index(i)?;
                let Some(name) = entry.enclosed_name() else { continue };
                if entry.is_dir() || name.starts_with("META-INF") {
                    continue;
                }
                let out = target.join(name);
                if let Some(parent) = out.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut f = std::fs::File::create(&out)?;
                std::io::copy(&mut entry, &mut f)?;
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))?
}
