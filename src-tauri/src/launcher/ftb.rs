//! Feed The Beast modpacks (public API, no key needed).
//!
//! Ids are prefixed with "ftb:" ("ftb:130"). A pack version lists every file
//! with its folder, download URL and SHA-1, plus the game and loader versions.

use super::install::{InstallProgress, ProgressFn, stage};
use super::instances::{self, Instance, PackInfo};
use super::modrinth::{GalleryImage, Project, SearchHit, SearchResult, Version};
use super::net::{self, Download};
use super::paths::Paths;
use crate::error::{Error, Result};
use chrono::{TimeZone, Utc};
use futures::{StreamExt, stream};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const API: &str = "https://api.feed-the-beast.com/v1/modpacks/public/modpack";
pub const PREFIX: &str = "ftb:";
const FILE_HOSTS: &[&str] = &["files.feed-the-beast.com", "cdn.feed-the-beast.com", "edge.forgecdn.net", "mediafilez.forgecdn.net", "cdn.modrinth.com"];

pub fn is_ftb(id: &str) -> bool {
    id.starts_with(PREFIX)
}

fn raw_id(id: &str) -> Result<u64> {
    id.trim_start_matches(PREFIX).parse().map_err(|_| Error::msg(format!("Bad FTB id '{id}'")))
}

fn iso(epoch: i64) -> String {
    Utc.timestamp_opt(epoch, 0).single().map(|d| d.to_rfc3339()).unwrap_or_default()
}

/// Pack details are cached for a while; listing needs every pack's details.
static CACHE: Mutex<Option<HashMap<u64, (Instant, Value)>>> = Mutex::new(None);
const CACHE_TTL: Duration = Duration::from_secs(15 * 60);

async fn pack(id: u64) -> Result<Value> {
    if let Some((at, v)) = CACHE.lock().unwrap().get_or_insert_with(HashMap::new).get(&id) {
        if at.elapsed() < CACHE_TTL {
            return Ok(v.clone());
        }
    }
    let v: Value = net::get_json(&format!("{API}/{id}")).await?;
    if v["status"] != "success" {
        return Err(Error::msg("FTB couldn't find that modpack"));
    }
    CACHE.lock().unwrap().get_or_insert_with(HashMap::new).insert(id, (Instant::now(), v.clone()));
    Ok(v)
}

async fn packs(ids: Vec<u64>) -> Vec<Value> {
    stream::iter(ids)
        .map(|id| async move { pack(id).await.ok() })
        .buffered(12)
        .filter_map(|v| async move { v })
        .filter(|v| {
            let public = !v["private"].as_bool().unwrap_or(false);
            async move { public }
        })
        .collect()
        .await
}

fn art(v: &Value, kind: &str) -> Vec<String> {
    v["art"].as_array().into_iter().flatten().filter(|a| a["type"] == kind).filter_map(|a| a["url"].as_str().map(String::from)).collect()
}

fn to_hit(v: &Value) -> SearchHit {
    SearchHit {
        project_id: format!("{PREFIX}{}", v["id"]),
        slug: v["slug"].as_str().unwrap_or_default().to_string(),
        title: v["name"].as_str().unwrap_or_default().to_string(),
        description: v["synopsis"].as_str().unwrap_or_default().to_string(),
        icon_url: art(v, "square").into_iter().next(),
        downloads: v["installs"].as_u64().unwrap_or(0),
        follows: v["plays_14d"].as_u64().unwrap_or(0),
        author: v["authors"][0]["name"].as_str().unwrap_or("FTB Team").to_string(),
        categories: v["tags"].as_array().into_iter().flatten().filter_map(|t| t["name"].as_str().map(String::from)).collect(),
        date_modified: iso(v["updated"].as_i64().unwrap_or(0)),
        client_side: None,
        server_side: None,
    }
}

/// List or search FTB packs.
pub async fn search(query: &str, sort: &str, offset: u32) -> Result<SearchResult> {
    let ids: Vec<u64> = if query.trim().is_empty() {
        let all: Value = net::get_json(&format!("{API}/all")).await?;
        all["packs"].as_array().into_iter().flatten().filter_map(|v| v.as_u64()).collect()
    } else {
        let found: Value = net::client()
            .get(format!("{API}/search/50"))
            .query(&[("term", query.trim())])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        found["packs"].as_array().into_iter().flatten().filter_map(|v| v.as_u64()).collect()
    };
    let mut list = packs(ids).await;
    let num = |v: &Value, k: &str| v[k].as_i64().unwrap_or(0);
    match sort {
        "downloads" => list.sort_by_key(|v| -num(v, "installs")),
        "follows" => list.sort_by_key(|v| -num(v, "plays")),
        "updated" => list.sort_by_key(|v| -num(v, "updated")),
        "newest" => list.sort_by_key(|v| -num(v, "released")),
        // "Relevance": keep search order, or featured + popular first when listing.
        _ if query.trim().is_empty() => list.sort_by_key(|v| (!v["featured"].as_bool().unwrap_or(false), -num(v, "plays_14d"))),
        _ => {}
    }
    let total = list.len() as u64;
    let hits = list.iter().skip(offset as usize).take(24).map(to_hit).collect();
    Ok(SearchResult { hits, total_hits: total, offset: offset as u64 })
}

pub async fn project(id: &str) -> Result<Project> {
    let v = pack(raw_id(id)?).await?;
    let slug = v["slug"].as_str().unwrap_or_default().to_string();
    Ok(Project {
        id: format!("{PREFIX}{}", v["id"]),
        slug: slug.clone(),
        title: v["name"].as_str().unwrap_or_default().to_string(),
        description: v["synopsis"].as_str().unwrap_or_default().to_string(),
        body: v["description"].as_str().unwrap_or_default().to_string(),
        icon_url: art(&v, "square").into_iter().next(),
        downloads: v["installs"].as_u64().unwrap_or(0),
        followers: v["plays_14d"].as_u64().unwrap_or(0),
        categories: v["tags"].as_array().into_iter().flatten().filter_map(|t| t["name"].as_str().map(String::from)).collect(),
        project_type: "modpack".into(),
        source_url: None,
        issues_url: None,
        wiki_url: None,
        discord_url: None,
        gallery: art(&v, "screenshot").into_iter().map(|url| GalleryImage { url, featured: false, title: None }).collect(),
        client_side: None,
        updated: Some(iso(v["updated"].as_i64().unwrap_or(0))),
        website_url: Some(format!("https://www.feed-the-beast.com/modpacks/{}-{}", v["id"], slug)),
    })
}

fn target(targets: &Value, name: &str) -> Option<String> {
    targets.as_array()?.iter().find(|t| t["name"] == name).and_then(|t| t["version"].as_str().map(String::from))
}

const LOADERS: &[&str] = &["neoforge", "forge", "fabric", "quilt"];

/// Every public version of a pack, newest first.
pub async fn versions(id: &str) -> Result<Vec<Version>> {
    let v = pack(raw_id(id)?).await?;
    let mut list: Vec<Version> = v["versions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|x| !x["private"].as_bool().unwrap_or(false))
        .map(|x| Version {
            id: x["id"].to_string(),
            project_id: format!("{PREFIX}{}", v["id"]),
            name: x["name"].as_str().unwrap_or_default().to_string(),
            version_number: x["name"].as_str().unwrap_or_default().to_string(),
            version_type: match x["type"].as_str().unwrap_or("release").to_lowercase().as_str() {
                "beta" => "beta",
                "alpha" => "alpha",
                _ => "release",
            }
            .into(),
            date_published: iso(x["updated"].as_i64().unwrap_or(0)),
            downloads: 0,
            game_versions: target(&x["targets"], "minecraft").into_iter().collect(),
            loaders: LOADERS.iter().filter(|l| target(&x["targets"], l).is_some()).map(|l| l.to_string()).collect(),
            files: vec![],
            dependencies: vec![],
        })
        .collect();
    list.sort_by(|a, b| b.date_published.cmp(&a.date_published));
    Ok(list)
}

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub targets: Value,
    pub files: Vec<ManifestFile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestFile {
    pub path: String,
    pub name: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub serveronly: bool,
}

/// Fetch a pack version's file list (best release unless a version is given).
pub async fn fetch(id: &str, version_id: Option<&str>) -> Result<(Manifest, PackInfo)> {
    let pid = raw_id(id)?;
    let v = pack(pid).await?;
    let vid = match version_id {
        Some(x) => x.to_string(),
        None => versions(id)
            .await?
            .iter()
            .find(|x| x.version_type == "release")
            .or(versions(id).await?.first())
            .map(|x| x.id.clone())
            .ok_or_else(|| Error::msg("This modpack has no versions"))?,
    };
    let body: Value = net::get_json(&format!("{API}/{pid}/{vid}")).await?;
    if body["status"] != "success" {
        return Err(Error::msg("FTB couldn't load that modpack version"));
    }
    let manifest: Manifest = serde_json::from_value(body.clone())?;
    Ok((
        manifest,
        PackInfo {
            name: v["name"].as_str().unwrap_or("FTB modpack").to_string(),
            version_number: body["name"].as_str().unwrap_or_default().to_string(),
            project_id: Some(format!("{PREFIX}{pid}")),
            version_id: Some(vid),
            icon_url: art(&v, "square").into_iter().next(),
            banner_url: art(&v, "splash").into_iter().next().or_else(|| art(&v, "screenshot").into_iter().next()),
            description: v["synopsis"].as_str().map(String::from).filter(|d| !d.is_empty()),
            ..Default::default()
        },
    ))
}

/// Create the (not yet downloaded) instance for an FTB pack version.
pub async fn prepare(paths: &Paths, manifest: &Manifest, pack: PackInfo) -> Result<Instance> {
    let mc = target(&manifest.targets, "minecraft").ok_or_else(|| Error::msg("The modpack doesn't say which Minecraft version it needs"))?;
    let (loader, loader_version) = LOADERS
        .iter()
        .find_map(|l| target(&manifest.targets, l).map(|v| (*l, v)))
        .map(|(l, v)| {
            let v = if l == "forge" && !v.starts_with(&format!("{mc}-")) { format!("{mc}-{v}") } else { v };
            (l, Some(v))
        })
        .unwrap_or(("vanilla", None));
    let mut inst = instances::create(paths, &pack.name, &mc, loader, loader_version).await?;
    inst.pack = Some(pack);
    instances::save(paths, &inst).await?;
    Ok(inst)
}

fn safe_rel(dir: &str, name: &str) -> Option<PathBuf> {
    let rel = Path::new(dir.trim_start_matches("./")).join(name);
    let ok = !name.contains(['/', '\\']) && rel.components().all(|c| matches!(c, Component::Normal(_)));
    ok.then_some(rel)
}

fn allowed(url: &str) -> bool {
    reqwest::Url::parse(url)
        .ok()
        .filter(|u| u.scheme() == "https")
        .and_then(|u| u.host_str().map(|h| FILE_HOSTS.contains(&h)))
        .unwrap_or(false)
}

/// Download every client file of the pack into the instance.
pub async fn populate(paths: &Paths, inst: &Instance, manifest: &Manifest, progress: &ProgressFn) -> Result<()> {
    let game = instances::game_dir(paths, &inst.id);
    let mut files = Vec::new();
    for f in manifest.files.iter().filter(|f| !f.serveronly) {
        let rel = safe_rel(&f.path, &f.name).ok_or_else(|| Error::msg(format!("Unsafe path in modpack: {}/{}", f.path, f.name)))?;
        if !allowed(&f.url) {
            return Err(Error::msg(format!("{} comes from a host Peak doesn't trust: {}", f.name, f.url)));
        }
        files.push(Download::new(f.url.clone(), game.join(rel), f.sha1.clone(), f.size));
    }
    progress(InstallProgress { stage: "Downloading modpack", done: 0, total: files.len(), bytes: 0 });
    net::download_all(files, net::concurrency(), stage(progress, "Downloading modpack")).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_hosts_are_checked() {
        assert_eq!(safe_rel("./mods", "a.jar"), Some(PathBuf::from("mods").join("a.jar")));
        assert_eq!(safe_rel("./", "options.txt"), Some(PathBuf::from("options.txt")));
        assert!(safe_rel("./../x", "a.jar").is_none());
        assert!(safe_rel("./mods", "../a.jar").is_none());
        assert!(allowed("https://files.feed-the-beast.com/blob/aa/x.cfg"));
        assert!(!allowed("https://evil.example/x.jar"));
    }

    /// cargo test -- --ignored --nocapture ftb_live
    #[tokio::test]
    #[ignore]
    async fn ftb_live() {
        let r = search("", "relevance", 0).await.unwrap();
        println!("ftb packs: {} first: {} ({})", r.total_hits, r.hits[0].title, r.hits[0].project_id);
        let s = search("stoneblock", "relevance", 0).await.unwrap();
        println!("search stoneblock: {:?}", s.hits.iter().map(|h| &h.title).collect::<Vec<_>>());
        let id = &s.hits[0].project_id;
        let p = project(id).await.unwrap();
        println!("project {} body {} chars, gallery {}", p.title, p.body.len(), p.gallery.len());
        let vs = versions(id).await.unwrap();
        println!("versions: {} newest {} {:?} {:?}", vs.len(), vs[0].name, vs[0].game_versions, vs[0].loaders);
        let (m, info) = fetch(id, None).await.unwrap();
        let client = m.files.iter().filter(|f| !f.serveronly).count();
        println!("manifest {} {}: {} files ({} client), mc {:?}", info.name, info.version_number, m.files.len(), client, target(&m.targets, "minecraft"));
        assert!(m.files.iter().all(|f| f.serveronly || (allowed(&f.url) && safe_rel(&f.path, &f.name).is_some())));
    }
}
