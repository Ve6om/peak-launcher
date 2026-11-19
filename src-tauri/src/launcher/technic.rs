//! Technic Platform modpacks (public API, no key needed).
//!
//! Ids are "technic:<slug>". A pack is either a single zip, or a "Solder"
//! build listing many zips. All zips are extracted into the game folder. The
//! loader ships as `bin/modpack.jar` (a Forge installer/universal jar) or
//! `bin/version.json`; we read which loader and version it is and install
//! that loader the normal way instead of using Technic's files.

use super::install::{InstallProgress, ProgressFn, stage};
use super::instances::{self, Instance, PackInfo};
use super::modrinth::{GalleryImage, Project, SearchHit, SearchResult, Version};
use super::net::{self, Download};
use super::paths::Paths;
use crate::error::{Error, Result};
use futures::{StreamExt, stream};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const API: &str = "https://api.technicpack.net";
pub const PREFIX: &str = "technic:";
/// Used if the current launcher build can't be fetched; the API requires one.
const FALLBACK_BUILD: u64 = 1180;

pub fn is_technic(id: &str) -> bool {
    id.starts_with(PREFIX)
}

fn slug(id: &str) -> Result<String> {
    let s = id.trim_start_matches(PREFIX);
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.') {
        return Err(Error::msg(format!("Bad Technic id '{id}'")));
    }
    Ok(s.to_string())
}

static BUILD: Mutex<Option<u64>> = Mutex::new(None);

/// The Technic launcher build number the API expects in every request.
async fn build() -> u64 {
    if let Some(b) = *BUILD.lock().unwrap() {
        return b;
    }
    let b = net::get_json::<Value>(&format!("{API}/launcher/version/stable4"))
        .await
        .ok()
        .and_then(|v| v["build"].as_u64())
        .unwrap_or(FALLBACK_BUILD);
    *BUILD.lock().unwrap() = Some(b);
    b
}

static CACHE: Mutex<Option<HashMap<String, (Instant, Value)>>> = Mutex::new(None);
const CACHE_TTL: Duration = Duration::from_secs(15 * 60);

async fn details(slug: &str) -> Result<Value> {
    if let Some((at, v)) = CACHE.lock().unwrap().get_or_insert_with(HashMap::new).get(slug) {
        if at.elapsed() < CACHE_TTL {
            return Ok(v.clone());
        }
    }
    let v: Value = net::get_json(&format!("{API}/modpack/{slug}?build={}", build().await)).await?;
    if v.get("error").is_some() {
        return Err(Error::msg("Technic couldn't find that modpack"));
    }
    CACHE.lock().unwrap().get_or_insert_with(HashMap::new).insert(slug.to_string(), (Instant::now(), v.clone()));
    Ok(v)
}

fn to_hit(v: &Value) -> SearchHit {
    SearchHit {
        project_id: format!("{PREFIX}{}", v["name"].as_str().unwrap_or_default()),
        slug: v["name"].as_str().unwrap_or_default().to_string(),
        title: v["displayName"].as_str().unwrap_or_default().to_string(),
        description: v["description"].as_str().unwrap_or_default().lines().next().unwrap_or_default().chars().take(240).collect(),
        icon_url: v["icon"]["url"].as_str().map(String::from),
        downloads: v["installs"].as_u64().unwrap_or(0),
        follows: v["runs"].as_u64().unwrap_or(0),
        author: v["user"].as_str().unwrap_or_default().to_string(),
        categories: [v["minecraft"].as_str().map(|m| format!("Minecraft {m}"))].into_iter().flatten().collect(),
        date_modified: String::new(),
    }
}

/// Search Technic, or show trending packs when there's no search term.
/// Technic returns everything at once, so paging is done here.
pub async fn search(query: &str, sort: &str, offset: u32) -> Result<SearchResult> {
    let b = build().await;
    let list: Value = if query.trim().is_empty() {
        net::get_json(&format!("{API}/trending?build={b}")).await?
    } else {
        net::client()
            .get(format!("{API}/search"))
            .query(&[("q", query.trim()), ("build", &b.to_string())])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?
    };
    let slugs: Vec<String> = list["modpacks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m["slug"].as_str().map(String::from))
        .collect();
    let total = slugs.len() as u64;
    let page: Vec<String> = slugs.into_iter().skip(offset as usize).take(24).collect();
    let mut hits: Vec<SearchHit> = stream::iter(page)
        .map(|s| async move { details(&s).await.ok() })
        .buffered(8)
        .filter_map(|v| async move { v.map(|v| to_hit(&v)) })
        .collect()
        .await;
    match sort {
        "downloads" => hits.sort_by_key(|h| std::cmp::Reverse(h.downloads)),
        "follows" => hits.sort_by_key(|h| std::cmp::Reverse(h.follows)),
        _ => {}
    }
    Ok(SearchResult { hits, total_hits: total, offset: offset as u64 })
}

pub async fn project(id: &str) -> Result<Project> {
    let v = details(&slug(id)?).await?;
    let gallery = ["background", "logo"]
        .iter()
        .filter_map(|k| v[*k]["url"].as_str())
        .map(|url| GalleryImage { url: url.to_string(), featured: false, title: None })
        .collect();
    Ok(Project {
        id: format!("{PREFIX}{}", v["name"].as_str().unwrap_or_default()),
        slug: v["name"].as_str().unwrap_or_default().to_string(),
        title: v["displayName"].as_str().unwrap_or_default().to_string(),
        description: format!("by {} · Minecraft {}", v["user"].as_str().unwrap_or("unknown"), v["minecraft"].as_str().unwrap_or("?")),
        body: v["description"].as_str().unwrap_or_default().to_string(),
        icon_url: v["icon"]["url"].as_str().map(String::from),
        downloads: v["installs"].as_u64().unwrap_or(0),
        followers: v["runs"].as_u64().unwrap_or(0),
        categories: vec![],
        project_type: "modpack".into(),
        source_url: None,
        issues_url: None,
        wiki_url: None,
        discord_url: None,
        gallery,
        client_side: None,
        updated: None,
        website_url: v["platformUrl"].as_str().map(String::from),
    })
}

fn solder_base(v: &Value) -> Option<String> {
    v["solder"].as_str().filter(|s| s.starts_with("http")).map(|s| s.trim_end_matches('/').to_string())
}

fn version(id: String, project: &str, name: &str, kind: &str, mc: Option<&str>) -> Version {
    Version {
        id,
        project_id: project.to_string(),
        name: name.to_string(),
        version_number: name.to_string(),
        version_type: kind.to_string(),
        date_published: String::new(),
        downloads: 0,
        game_versions: mc.into_iter().map(String::from).collect(),
        loaders: vec![],
        files: vec![],
        dependencies: vec![],
    }
}

/// Builds of a pack, newest first; the recommended one is marked as release.
pub async fn versions(id: &str) -> Result<Vec<Version>> {
    let s = slug(id)?;
    let v = details(&s).await?;
    let pid = format!("{PREFIX}{s}");
    match solder_base(&v) {
        Some(base) => {
            let sv: Value = net::get_json(&format!("{base}/modpack/{s}")).await?;
            let rec = sv["recommended"].as_str().unwrap_or_default().to_string();
            let mut builds: Vec<Version> = sv["builds"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|b| b.as_str())
                .map(|b| version(b.to_string(), &pid, b, if b == rec { "release" } else { "beta" }, None))
                .collect();
            builds.reverse();
            // Recommended first, then the rest newest-first.
            builds.sort_by_key(|b| b.version_type != "release");
            Ok(builds)
        }
        None => {
            let name = v["version"].as_str().unwrap_or("latest");
            Ok(vec![version(name.to_string(), &pid, name, "release", v["minecraft"].as_str())])
        }
    }
}

/// What to download for a pack version.
pub struct Plan {
    pub minecraft: String,
    pub zips: Vec<Download>,
}

pub async fn fetch(paths: &Paths, id: &str, version_id: Option<&str>) -> Result<(Plan, PackInfo)> {
    let s = slug(id)?;
    let v = details(&s).await?;
    let cache = paths.cache().join("technic").join(&s);
    let info = |ver: &str| PackInfo {
        name: v["displayName"].as_str().unwrap_or(&s).to_string(),
        version_number: ver.to_string(),
        project_id: Some(format!("{PREFIX}{s}")),
        version_id: Some(ver.to_string()),
        icon_url: v["icon"]["url"].as_str().map(String::from),
        banner_url: v["background"]["url"].as_str().map(String::from),
        description: v["description"].as_str().and_then(|d| d.lines().map(str::trim).find(|l| !l.is_empty())).map(String::from),
        ..Default::default()
    };
    match solder_base(&v) {
        Some(base) => {
            let build = match version_id {
                Some(b) => b.to_string(),
                None => {
                    let sv: Value = net::get_json(&format!("{base}/modpack/{s}")).await?;
                    sv["recommended"].as_str().or(sv["latest"].as_str()).ok_or_else(|| Error::msg("This modpack has no builds"))?.to_string()
                }
            };
            let bv: Value = net::get_json(&format!("{base}/modpack/{s}/{build}")).await?;
            let minecraft = bv["minecraft"].as_str().ok_or_else(|| Error::msg("The build doesn't say which Minecraft version it needs"))?.to_string();
            let zips = bv["mods"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|m| {
                    let url = m["url"].as_str()?;
                    let name = format!("{}-{}.zip", m["name"].as_str()?, m["version"].as_str().unwrap_or("0")).replace(['/', '\\'], "_");
                    Some(Download::new(url.to_string(), cache.join(name), None, m["filesize"].as_u64().filter(|s| *s > 0)))
                })
                .collect();
            Ok((Plan { minecraft, zips }, info(&build)))
        }
        None => {
            let url = v["url"].as_str().filter(|u| !u.is_empty()).ok_or_else(|| Error::msg("This modpack has no download"))?;
            let minecraft = v["minecraft"].as_str().ok_or_else(|| Error::msg("The modpack doesn't say which Minecraft version it needs"))?.to_string();
            let ver = v["version"].as_str().unwrap_or("latest").to_string();
            Ok((Plan { minecraft, zips: vec![Download::new(url.to_string(), cache.join("pack.zip"), None, None)] }, info(&ver)))
        }
    }
}

/// Create the instance (the loader is filled in once the files are read).
pub async fn prepare(paths: &Paths, plan: &Plan, pack: PackInfo) -> Result<Instance> {
    if !supported_minecraft(&plan.minecraft) {
        return Err(Error::msg(format!(
            "This pack uses Minecraft {}, which installs mods in a way Peak doesn't support yet (versions before 1.6).",
            plan.minecraft
        )));
    }
    let mut inst = instances::create(paths, &pack.name, &plan.minecraft, "vanilla", None).await?;
    inst.pack = Some(pack);
    instances::save(paths, &inst).await?;
    Ok(inst)
}

fn supported_minecraft(v: &str) -> bool {
    let mut parts = v.split('.').skip(1).map(|p| p.parse::<u32>().unwrap_or(0));
    !matches!(parts.next(), Some(minor) if minor < 6) || !v.starts_with("1.")
}

/// Loader declared in a version.json: forge / neoforge / fabric / quilt.
pub fn loader_from_version_json(v: &Value, mc: &str) -> Option<(&'static str, String)> {
    for lib in v["libraries"].as_array()? {
        let name = lib["name"].as_str().unwrap_or_default();
        let ver = |prefix: &str| name.strip_prefix(prefix).map(|r| r.split(':').next().unwrap_or(r).to_string());
        if let Some(x) = ver("net.minecraftforge:forge:") {
            return Some(("forge", if x.starts_with(&format!("{mc}-")) { x } else { format!("{mc}-{x}") }));
        }
        if let Some(x) = ver("net.neoforged:neoforge:") {
            return Some(("neoforge", x));
        }
        if let Some(x) = ver("net.fabricmc:fabric-loader:") {
            return Some(("fabric", x));
        }
        if let Some(x) = ver("org.quiltmc:quilt-loader:") {
            return Some(("quilt", x));
        }
    }
    None
}

/// Find the loader in an extracted pack: bin/version.json, or the
/// version.json inside bin/modpack.jar.
fn detect_loader(game: &Path, mc: &str) -> Result<Option<(&'static str, String)>> {
    let plain = game.join("bin").join("version.json");
    if let Ok(text) = std::fs::read_to_string(&plain) {
        if let Some(l) = serde_json::from_str::<Value>(&text).ok().and_then(|v| loader_from_version_json(&v, mc)) {
            return Ok(Some(l));
        }
    }
    let jar = game.join("bin").join("modpack.jar");
    if jar.exists() {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&jar)?)?;
        if let Ok(mut entry) = zip.by_name("version.json") {
            let mut text = String::new();
            entry.read_to_string(&mut text)?;
            if let Some(l) = serde_json::from_str::<Value>(&text).ok().and_then(|v| loader_from_version_json(&v, mc)) {
                return Ok(Some(l));
            }
        }
        return Err(Error::msg("This pack patches Minecraft's game jar directly, which Peak doesn't support."));
    }
    Ok(None)
}

fn extract(zip_path: &Path, dest: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(zip_path)?)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let Some(rel) = entry.enclosed_name() else { continue }; // skips "../" tricks
        let out = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::io::copy(&mut entry, &mut std::fs::File::create(&out)?)?;
    }
    Ok(())
}

/// Download and extract the pack, then set the instance's loader from what
/// the pack contains. Returns the updated instance.
pub async fn populate(paths: &Paths, inst: &Instance, plan: Plan, progress: &ProgressFn) -> Result<Instance> {
    let zips: Vec<PathBuf> = plan.zips.iter().map(|d| d.path.clone()).collect();
    for d in &plan.zips {
        if !d.url.starts_with("https://") && !d.url.starts_with("http://") {
            return Err(Error::msg(format!("Unsupported download: {}", d.url)));
        }
    }
    net::download_all(plan.zips, net::concurrency(), stage(progress, "Downloading modpack")).await?;

    let game = instances::game_dir(paths, &inst.id);
    let total = zips.len();
    let (g, mc, p) = (game.clone(), inst.game_version.clone(), progress.clone());
    let loader = tokio::task::spawn_blocking(move || -> Result<Option<(&'static str, String)>> {
        for (i, z) in zips.iter().enumerate() {
            extract(z, &g).map_err(|e| Error::msg(format!("Couldn't extract {}: {e}", z.file_name().unwrap_or_default().to_string_lossy())))?;
            p(InstallProgress { stage: "Unpacking modpack", done: i + 1, total, bytes: 0 });
        }
        let found = detect_loader(&g, &mc)?;
        // Technic's own launcher files aren't needed; Peak installs the loader itself.
        let _ = std::fs::remove_dir_all(g.join("bin"));
        Ok(found)
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))??;

    let mut updated = instances::get(paths, &inst.id).await?;
    if let Some((l, v)) = loader {
        updated.loader = l.to_string();
        updated.loader_version = Some(v);
        updated.installed = false;
        instances::save(paths, &updated).await?;
    }
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_loader_and_versions() {
        let v: Value = serde_json::from_str(
            r#"{"libraries":[{"name":"net.minecraft:launchwrapper:1.12"},{"name":"net.minecraftforge:forge:1.12.2-14.23.5.2860"}]}"#,
        )
        .unwrap();
        assert_eq!(loader_from_version_json(&v, "1.12.2"), Some(("forge", "1.12.2-14.23.5.2860".into())));
        let f: Value = serde_json::from_str(r#"{"libraries":[{"name":"net.fabricmc:fabric-loader:0.16.9"}]}"#).unwrap();
        assert_eq!(loader_from_version_json(&f, "1.21.1"), Some(("fabric", "0.16.9".into())));
        assert!(supported_minecraft("1.7.10") && supported_minecraft("1.12.2") && supported_minecraft("1.6.4"));
        assert!(!supported_minecraft("1.5.2") && !supported_minecraft("1.4.7"));
        assert!(slug("technic:tekkit-2").is_ok() && slug("technic:../x").is_err());
    }

    /// cargo test -- --ignored --nocapture technic_live
    #[tokio::test]
    #[ignore]
    async fn technic_live() {
        let t = search("", "relevance", 0).await.unwrap();
        println!("trending: {} first: {:?}", t.total_hits, t.hits.iter().take(3).map(|h| &h.title).collect::<Vec<_>>());
        let s = search("tekkit 2", "relevance", 0).await.unwrap();
        println!("search: {:?}", s.hits.iter().take(3).map(|h| (&h.title, &h.project_id)).collect::<Vec<_>>());
        let p = project("technic:tekkit-2").await.unwrap();
        println!("project {} installs {} site {:?}", p.title, p.downloads, p.website_url);
        let vs = versions("technic:tekkit-2").await.unwrap();
        println!("builds: {} first {} ({})", vs.len(), vs[0].name, vs[0].version_type);

        // Real install of just the loader zip to check detection end to end.
        let dir = std::env::temp_dir().join(format!("peak-technic-{}", std::process::id()));
        let paths = Paths::new(dir.clone());
        let (mut plan, info) = fetch(&paths, "technic:tekkit-2", None).await.unwrap();
        println!("plan: mc {} with {} zips", plan.minecraft, plan.zips.len());
        plan.zips.retain(|z| z.url.contains("/forge/") || z.url.contains("forgelin"));
        let inst = prepare(&paths, &plan, info).await.unwrap();
        let progress: ProgressFn = std::sync::Arc::new(|_| {});
        let done = populate(&paths, &inst, plan, &progress).await.unwrap();
        println!("detected loader: {} {:?}", done.loader, done.loader_version);
        assert_eq!(done.loader, "forge");
        let game = instances::game_dir(&paths, &done.id);
        assert!(!game.join("bin").exists());
        assert!(game.join("mods").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
