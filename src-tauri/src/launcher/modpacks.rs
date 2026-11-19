//! Modrinth modpacks (.mrpack): https://support.modrinth.com/en/articles/8802351
//!
//! A .mrpack is a zip with `modrinth.index.json` (game/loader versions plus a
//! list of files to download) and `overrides/` folders copied into the game
//! directory.

use super::install::{InstallProgress, ProgressFn, stage};
use super::instances::{self, Instance, PackInfo};
use super::modrinth::{self, SearchResult, Version};
use super::net::{self, Download};
use super::paths::Paths;
use crate::error::{Error, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

const API: &str = "https://api.modrinth.com/v2";

/// Hosts a pack may download from (per the .mrpack spec).
const ALLOWED_HOSTS: &[&str] = &["cdn.modrinth.com", "github.com", "raw.githubusercontent.com", "gitlab.com"];

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    pub name: String,
    #[serde(default)]
    pub version_id: String,
    #[serde(default)]
    pub files: Vec<PackFile>,
    pub dependencies: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackFile {
    pub path: String,
    #[serde(default)]
    pub hashes: HashMap<String, String>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
    #[serde(default)]
    pub downloads: Vec<String>,
    #[serde(default)]
    pub file_size: Option<u64>,
}

pub async fn search(query: &str, sort: &str, offset: u32) -> Result<SearchResult> {
    let index = match sort {
        "downloads" | "follows" | "newest" | "updated" => sort,
        _ => "relevance",
    };
    let resp = net::client()
        .get(format!("{API}/search"))
        .query(&[
            ("query", query.to_string()),
            ("facets", r#"[["project_type:modpack"]]"#.to_string()),
            ("index", index.to_string()),
            ("offset", offset.to_string()),
            ("limit", "24".to_string()),
        ])
        .send()
        .await?
        .error_for_status()?;
    Ok(resp.json().await?)
}

/// Every version of a modpack, newest first.
pub async fn versions(project_id: &str) -> Result<Vec<Version>> {
    net::get_json(&format!("{API}/project/{project_id}/version")).await
}

/// Download a pack's .mrpack from Modrinth (best release unless a version is given).
pub async fn fetch(paths: &Paths, project_id: &str, version_id: Option<&str>) -> Result<(PathBuf, PackInfo)> {
    let version: Version = match version_id {
        Some(v) => net::get_json(&format!("{API}/version/{v}")).await?,
        None => {
            let list = versions(project_id).await?;
            list.iter()
                .find(|v| v.version_type == "release")
                .or_else(|| list.first())
                .cloned()
                .ok_or_else(|| Error::msg("This modpack has no versions"))?
        }
    };
    let file = version.primary_file().ok_or_else(|| Error::msg("This modpack version has no files"))?.clone();
    let dest = paths.cache().join("modpacks").join(&file.filename);
    net::download_all(
        vec![Download::new(file.url.clone(), dest.clone(), file.hashes.get("sha1").cloned(), Some(file.size))],
        1,
        |_| {},
    )
    .await?;
    let project = modrinth::project(project_id).await?;
    Ok((
        dest,
        PackInfo {
            name: project.title,
            version_number: version.version_number,
            project_id: Some(project_id.to_string()),
            version_id: Some(version.id),
            banner_url: project.gallery.iter().find(|g| g.featured).or(project.gallery.first()).map(|g| g.url.clone()),
            description: Some(project.description.clone()).filter(|d| !d.is_empty()),
            icon_url: project.icon_url,
            ..Default::default()
        },
    ))
}

fn read_index(mrpack: &Path) -> Result<Index> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(mrpack)?)?;
    let mut entry = zip
        .by_name("modrinth.index.json")
        .map_err(|_| Error::msg("This isn't a Modrinth modpack (no modrinth.index.json)"))?;
    let mut buf = String::new();
    entry.read_to_string(&mut buf)?;
    Ok(serde_json::from_str(&buf)?)
}

/// Read a pack and create its (not yet downloaded) instance.
pub async fn prepare(paths: &Paths, mrpack: &Path, pack: Option<PackInfo>) -> Result<(Instance, Index)> {
    let p = mrpack.to_owned();
    let index = tokio::task::spawn_blocking(move || read_index(&p)).await.map_err(|e| Error::msg(e.to_string()))??;
    let deps = &index.dependencies;
    let mc = deps.get("minecraft").ok_or_else(|| Error::msg("The modpack doesn't say which Minecraft version it needs"))?;
    let (loader, loader_version) = if let Some(v) = deps.get("fabric-loader") {
        ("fabric", Some(v.clone()))
    } else if let Some(v) = deps.get("quilt-loader") {
        ("quilt", Some(v.clone()))
    } else if let Some(v) = deps.get("neoforge") {
        ("neoforge", Some(v.clone()))
    } else if let Some(v) = deps.get("forge") {
        // Our Forge ids include the game version, e.g. "1.20.1-47.3.0".
        ("forge", Some(if v.starts_with(&format!("{mc}-")) { v.clone() } else { format!("{mc}-{v}") }))
    } else {
        ("vanilla", None)
    };

    let mut inst = instances::create(paths, &index.name, mc, loader, loader_version).await?;
    inst.pack = Some(pack.unwrap_or_else(|| PackInfo {
        name: index.name.clone(),
        version_number: index.version_id.clone(),
        project_id: None,
        version_id: None,
        icon_url: None,
        banner_url: None,
        description: None,
        ..Default::default()
    }));
    if let Some(p) = inst.pack.as_mut() {
        if p.version_number.is_empty() {
            p.version_number = index.version_id.clone();
        }
    }
    instances::save(paths, &inst).await?;
    Ok((inst, index))
}

/// A pack-relative path that stays inside the game folder.
fn safe_path(rel: &str) -> Option<PathBuf> {
    let p = Path::new(rel);
    if p.is_absolute() || p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(p.to_path_buf())
}

fn allowed_url(url: &str) -> bool {
    reqwest::Url::parse(url)
        .ok()
        .filter(|u| u.scheme() == "https")
        .and_then(|u| u.host_str().map(|h| ALLOWED_HOSTS.contains(&h)))
        .unwrap_or(false)
}

/// Download the pack's files and copy its overrides into the instance.
pub async fn populate(paths: &Paths, inst: &Instance, mrpack: &Path, index: &Index, progress: &ProgressFn) -> Result<()> {
    let game = instances::game_dir(paths, &inst.id);
    let mut files = Vec::new();
    for f in &index.files {
        if f.env.as_ref().and_then(|e| e.get("client")).map(|c| c == "unsupported").unwrap_or(false) {
            continue; // server-only file
        }
        let rel = safe_path(&f.path).ok_or_else(|| Error::msg(format!("Unsafe path in modpack: {}", f.path)))?;
        let url = f
            .downloads
            .iter()
            .find(|u| allowed_url(u))
            .ok_or_else(|| Error::msg(format!("No allowed download for {}", f.path)))?;
        files.push(Download::new(url.clone(), game.join(rel), f.hashes.get("sha1").cloned(), f.file_size));
    }
    net::download_all(files, net::concurrency(), stage(progress, "Downloading modpack")).await?;

    progress(InstallProgress { stage: "Copying modpack files", done: 0, total: 0, bytes: 0 });
    let (src, dst) = (mrpack.to_owned(), game.clone());
    tokio::task::spawn_blocking(move || -> Result<()> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&src)?)?;
        // client-overrides are applied after overrides so they win.
        for prefix in ["overrides/", "client-overrides/"] {
            for i in 0..zip.len() {
                let mut entry = zip.by_index(i)?;
                let Some(name) = entry.enclosed_name() else { continue };
                let Ok(rel) = name.strip_prefix(prefix.trim_end_matches('/')) else { continue };
                if entry.is_dir() || rel.as_os_str().is_empty() {
                    continue;
                }
                let out = dst.join(rel);
                if let Some(parent) = out.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::io::copy(&mut entry, &mut std::fs::File::create(&out)?)?;
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// cargo test -- --ignored --nocapture modpack_flow
    #[tokio::test]
    #[ignore]
    async fn modpack_flow() {
        let dir = std::env::temp_dir().join(format!("peak-pack-test-{}", std::process::id()));
        let paths = Paths::new(dir.clone());
        let found = search("fabulously optimized", "relevance", 0).await.unwrap();
        let hit = &found.hits[0];
        println!("pack: {} ({})", hit.title, hit.project_id);
        let (mrpack, info) = fetch(&paths, &hit.project_id, None).await.unwrap();
        println!("version {} -> {}", info.version_number, mrpack.display());
        let (inst, index) = prepare(&paths, &mrpack, Some(info)).await.unwrap();
        println!("instance {} : mc {} {} {:?}, {} files", inst.id, inst.game_version, inst.loader, inst.loader_version, index.files.len());
        let progress: ProgressFn = std::sync::Arc::new(|_| {});
        populate(&paths, &inst, &mrpack, &index, &progress).await.unwrap();
        let game = instances::game_dir(&paths, &inst.id);
        let mods = std::fs::read_dir(game.join("mods")).map(|r| r.count()).unwrap_or(0);
        let config = game.join("config").exists();
        println!("mods on disk: {mods}, config folder from overrides: {config}");
        assert!(mods > 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_unsafe_paths_and_hosts() {
        assert!(safe_path("mods/a.jar").is_some());
        assert!(safe_path("../evil.jar").is_none());
        assert!(safe_path("/etc/passwd").is_none());
        assert!(safe_path("mods/../../x").is_none());
        assert!(allowed_url("https://cdn.modrinth.com/data/x/a.jar"));
        assert!(!allowed_url("http://cdn.modrinth.com/data/x/a.jar"));
        assert!(!allowed_url("https://example.com/a.jar"));
    }
}
