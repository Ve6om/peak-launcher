//! Mods, resource packs and shaders from Modrinth (https://docs.modrinth.com).
//! No API key is needed; requests identify the launcher via its User-Agent.

use super::instances::{self, Instance};
use super::net::{self, Download};
use super::paths::{self, Paths};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

const API: &str = "https://api.modrinth.com/v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Mod,
    Resourcepack,
    Shader,
}

impl Kind {
    pub fn parse(s: &str) -> Result<Kind> {
        Ok(match s {
            "mod" => Kind::Mod,
            "resourcepack" => Kind::Resourcepack,
            "shader" => Kind::Shader,
            other => return Err(Error::msg(format!("Unknown content type '{other}'"))),
        })
    }
    pub fn folder(self) -> &'static str {
        match self {
            Kind::Mod => "mods",
            Kind::Resourcepack => "resourcepacks",
            Kind::Shader => "shaderpacks",
        }
    }
    fn api_name(self) -> &'static str {
        match self {
            Kind::Mod => "mod",
            Kind::Resourcepack => "resourcepack",
            Kind::Shader => "shader",
        }
    }
}

pub fn content_dir(paths: &Paths, inst: &str, kind: Kind) -> PathBuf {
    instances::game_dir(paths, inst).join(kind.folder())
}

/// Modrinth loader names usable by an instance (Quilt can run Fabric mods).
fn loaders_for(inst: &Instance, kind: Kind) -> Vec<&'static str> {
    if kind != Kind::Mod {
        return vec![];
    }
    match inst.loader.as_str() {
        "fabric" => vec!["fabric"],
        "quilt" => vec!["quilt", "fabric"],
        "forge" => vec!["forge"],
        "neoforge" => vec!["neoforge"],
        _ => vec![],
    }
}

// ---------------------------------------------------------------- search

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub follows: u64,
    pub author: String,
    #[serde(default)]
    pub categories: Vec<String>,
    pub date_modified: String,
    /// Where the mod runs, from Modrinth: required, optional, unsupported or
    /// unknown. None for sources that don't say (CurseForge, FTB, Technic).
    #[serde(default)]
    pub client_side: Option<String>,
    #[serde(default)]
    pub server_side: Option<String>,
}

/// Modrinth search facets for a "which side does it run on" filter.
fn side_facets(side: &str) -> Vec<Vec<String>> {
    let runs = |s: &str| vec![format!("{s}_side:required"), format!("{s}_side:optional")];
    // "Client only" means the server doesn't need it (and the other way
    // round), which includes mods that are optional on the other side.
    let not = |s: &str| vec![format!("{s}_side:unsupported"), format!("{s}_side:optional")];
    match side {
        "client" => vec![runs("client"), not("server")],
        "server" => vec![runs("server"), not("client")],
        "both" => vec![runs("client"), runs("server")],
        _ => vec![],
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub hits: Vec<SearchHit>,
    pub total_hits: u64,
    pub offset: u64,
}

pub async fn search(paths: &Paths, instance_id: &str, kind: Kind, query: &str, sort: &str, side: &str, offset: u32) -> Result<SearchResult> {
    let inst = instances::get(paths, instance_id).await?;
    let loaders = loaders_for(&inst, kind);
    if kind == Kind::Mod && loaders.is_empty() {
        return Err(Error::msg("Vanilla instances can't run mods. Add a mod loader in the instance's settings."));
    }
    let mut facets: Vec<Vec<String>> = vec![
        vec![format!("project_type:{}", kind.api_name())],
        vec![format!("versions:{}", inst.game_version)],
    ];
    if !loaders.is_empty() {
        facets.push(loaders.iter().map(|l| format!("categories:{l}")).collect());
    }
    if kind == Kind::Mod {
        facets.extend(side_facets(side));
    }
    let index = match sort {
        "downloads" | "follows" | "newest" | "updated" => sort,
        _ => "relevance",
    };
    let resp = net::client()
        .get(format!("{API}/search"))
        .query(&[
            ("query", query.to_string()),
            ("facets", serde_json::to_string(&facets)?),
            ("index", index.to_string()),
            ("offset", offset.to_string()),
            ("limit", "24".to_string()),
        ])
        .send()
        .await?
        .error_for_status()?;
    Ok(resp.json().await?)
}

// ---------------------------------------------------------------- projects & versions

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GalleryImage {
    pub url: String,
    #[serde(default)]
    pub featured: bool,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub followers: u64,
    #[serde(default)]
    pub categories: Vec<String>,
    pub project_type: String,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub issues_url: Option<String>,
    #[serde(default)]
    pub wiki_url: Option<String>,
    #[serde(default)]
    pub discord_url: Option<String>,
    #[serde(default)]
    pub gallery: Vec<GalleryImage>,
    #[serde(default)]
    pub client_side: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
    /// Project page outside Modrinth (used for CurseForge projects).
    #[serde(default)]
    pub website_url: Option<String>,
}

pub async fn project(id: &str) -> Result<Project> {
    net::get_json(&format!("{API}/project/{id}")).await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionFile {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    pub size: u64,
    pub hashes: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    pub dependency_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub version_type: String,
    pub date_published: String,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

impl Version {
    pub fn primary_file(&self) -> Option<&VersionFile> {
        self.files.iter().find(|f| f.primary).or_else(|| self.files.first())
    }
}

/// Versions of a project that work with an instance, newest first.
pub async fn compatible_versions(inst: &Instance, project_id: &str, kind: Kind) -> Result<Vec<Version>> {
    let loaders = loaders_for(inst, kind);
    let mut req = net::client()
        .get(format!("{API}/project/{project_id}/version"))
        .query(&[("game_versions", serde_json::to_string(&[&inst.game_version])?)]);
    if !loaders.is_empty() {
        req = req.query(&[("loaders", serde_json::to_string(&loaders)?)]);
    }
    Ok(req.send().await?.error_for_status()?.json().await?)
}

fn best(versions: &[Version]) -> Option<&Version> {
    versions.iter().find(|v| v.version_type == "release").or_else(|| versions.first())
}

// ---------------------------------------------------------------- installed content

/// What we know about an installed file, keyed by its SHA-1.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub version_number: String,
    #[serde(default)]
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct Store {
    #[serde(default)]
    pub items: HashMap<String, Meta>,
    /// "kind/file_name" -> (size, sha1), so we don't rehash unchanged files.
    #[serde(default)]
    hashes: HashMap<String, (u64, String)>,
}

fn store_path(paths: &Paths, inst: &str) -> PathBuf {
    paths.instance(inst).join("content.json")
}

pub(crate) async fn load_store(paths: &Paths, inst: &str) -> Result<Store> {
    Ok(paths::read_json(&store_path(paths, inst)).await?.unwrap_or_default())
}

pub(crate) async fn save_store(paths: &Paths, inst: &str, s: &Store) -> Result<()> {
    paths::write_json(&store_path(paths, inst), s).await
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledItem {
    pub file_name: String,
    pub enabled: bool,
    pub size: u64,
    pub sha1: String,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub title: String,
    pub version_number: Option<String>,
    pub icon_url: Option<String>,
}

fn is_content_file(name: &str) -> bool {
    let base = name.strip_suffix(".disabled").unwrap_or(name);
    base.ends_with(".jar") || base.ends_with(".zip")
}

/// List installed files, identifying unknown ones through Modrinth by hash.
pub async fn list(paths: &Paths, inst: &str, kind: Kind) -> Result<Vec<InstalledItem>> {
    let dir = content_dir(paths, inst, kind);
    let mut store = load_store(paths, inst).await?;
    let mut files: Vec<(String, u64, String)> = Vec::new();
    if let Ok(mut rd) = tokio::fs::read_dir(&dir).await {
        while let Some(e) = rd.next_entry().await? {
            let name = e.file_name().to_string_lossy().into_owned();
            let meta = e.metadata().await?;
            if !meta.is_file() || !is_content_file(&name) {
                continue;
            }
            let key = format!("{}/{}", kind.folder(), name);
            let sha = match store.hashes.get(&key) {
                Some((size, sha)) if *size == meta.len() => sha.clone(),
                _ => {
                    let sha = net::sha1_file(&e.path()).await?;
                    store.hashes.insert(key, (meta.len(), sha.clone()));
                    sha
                }
            };
            files.push((name, meta.len(), sha));
        }
    }

    // Identify files we have no metadata for (e.g. dropped in by hand).
    let unknown: Vec<String> = files.iter().filter(|f| !store.items.contains_key(&f.2)).map(|f| f.2.clone()).collect();
    if !unknown.is_empty() {
        if let Ok(found) = identify(&unknown).await {
            store.items.extend(found);
        }
    }
    save_store(paths, inst, &store).await?;

    let mut out: Vec<InstalledItem> = files
        .into_iter()
        .map(|(name, size, sha)| {
            let m = store.items.get(&sha);
            let display = name.strip_suffix(".disabled").unwrap_or(&name);
            InstalledItem {
                enabled: !name.ends_with(".disabled"),
                title: m.map(|m| m.title.clone()).unwrap_or_else(|| display.trim_end_matches(".jar").trim_end_matches(".zip").to_string()),
                project_id: m.map(|m| m.project_id.clone()),
                version_id: m.map(|m| m.version_id.clone()),
                version_number: m.map(|m| m.version_number.clone()),
                icon_url: m.and_then(|m| m.icon_url.clone()),
                file_name: name,
                size,
                sha1: sha,
            }
        })
        .collect();
    out.sort_by_key(|i| i.title.to_lowercase());
    Ok(out)
}

/// The Modrinth project for a mod id named in a crash ("sodium",
/// "fabric-api"...), if one fits this instance. Mod ids usually match a
/// project's slug; otherwise the best search hit that has a version for the
/// instance's Minecraft and loader.
pub async fn project_for_mod_id(inst: &Instance, mod_id: &str, name: &str) -> Option<String> {
    let id = mod_id.trim().to_lowercase();
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) {
        return None;
    }
    let fits = |pid: String| async move { compatible_versions(inst, &pid, Kind::Mod).await.ok().filter(|v| !v.is_empty()).map(|_| pid) };
    for slug in [id.clone(), id.replace('_', "-")] {
        if let Ok(res) = net::client().get(format!("{API}/project/{slug}")).send().await {
            if let Ok(p) = res.error_for_status() {
                if let Ok(p) = p.json::<serde_json::Value>().await {
                    if let Some(pid) = p.get("id").and_then(|v| v.as_str()) {
                        if let Some(pid) = fits(pid.to_string()).await {
                            return Some(pid);
                        }
                    }
                }
            }
        }
    }
    // Search: by name (or the id as words), then by the start of the id, since
    // ids often run words together ("kotlinforforge" is "kotlin-for-forge").
    let squash = |s: &str| s.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>();
    let target = squash(&id);
    let facets = json!([["project_type:mod"], [format!("versions:{}", inst.game_version)]]).to_string();
    let by_name = if name.trim().is_empty() || name.eq_ignore_ascii_case(mod_id) { id.replace(['-', '_', '.'], " ") } else { name.to_string() };
    let prefix: String = target.chars().take(6).collect();
    let mut loose: Option<String> = None;
    for (query, strict) in [(by_name, false), (prefix, true)] {
        let Some(hits) = search_hits(&query, &facets).await else { continue };
        for (pid, slug, title) in hits {
            let same = squash(&slug) == target || squash(&title) == target;
            if same {
                if let Some(pid) = fits(pid).await {
                    return Some(pid);
                }
            } else if !strict && loose.is_none() {
                loose = Some(pid);
            }
        }
    }
    // Nothing with that exact id: the top search hit for its name, if it fits.
    match loose {
        Some(pid) => fits(pid).await,
        None => None,
    }
}

/// (project id, slug, title) of the top mod search hits.
async fn search_hits(query: &str, facets: &str) -> Option<Vec<(String, String, String)>> {
    if query.trim().is_empty() {
        return None;
    }
    let res: serde_json::Value = net::client()
        .get(format!("{API}/search"))
        .query(&[("query", query), ("limit", "8"), ("facets", facets)])
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    Some(
        res.get("hits")?
            .as_array()?
            .iter()
            .filter_map(|h| {
                Some((h.get("project_id")?.as_str()?.to_string(), h.get("slug")?.as_str()?.to_string(), h.get("title")?.as_str()?.to_string()))
            })
            .collect(),
    )
}

/// The exact same files on Modrinth (by SHA-1), keyed by hash. Used for
/// CurseForge files their authors don't let launchers download: when the
/// identical file is also on Modrinth, it can come from there instead.
pub async fn same_files(hashes: &[String]) -> Result<HashMap<String, VersionFile>> {
    if hashes.is_empty() {
        return Ok(HashMap::new());
    }
    let versions: HashMap<String, Version> = net::client()
        .post(format!("{API}/version_files"))
        .json(&json!({ "hashes": hashes, "algorithm": "sha1" }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(versions
        .into_iter()
        .filter_map(|(hash, v)| {
            let file = v.files.into_iter().find(|f| f.hashes.get("sha1").is_some_and(|h| h.eq_ignore_ascii_case(&hash)))?;
            // Only ever Modrinth's own file host.
            file.url.starts_with("https://cdn.modrinth.com/").then_some((hash, file))
        })
        .collect())
}

/// Look up files on Modrinth by SHA-1 and return metadata for the ones found.
async fn identify(hashes: &[String]) -> Result<HashMap<String, Meta>> {
    let versions: HashMap<String, Version> = net::client()
        .post(format!("{API}/version_files"))
        .json(&json!({ "hashes": hashes, "algorithm": "sha1" }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let ids: Vec<&String> = versions.values().map(|v| &v.project_id).collect::<HashSet<_>>().into_iter().collect();
    let projects = projects_by_id(&ids).await.unwrap_or_default();
    Ok(versions
        .into_iter()
        .map(|(hash, v)| {
            let p = projects.get(&v.project_id);
            (
                hash,
                Meta {
                    title: p.map(|p| p.title.clone()).unwrap_or_else(|| v.name.clone()),
                    icon_url: p.and_then(|p| p.icon_url.clone()),
                    project_id: v.project_id,
                    version_id: v.id,
                    version_number: v.version_number,
                },
            )
        })
        .collect())
}

async fn projects_by_id(ids: &[&String]) -> Result<HashMap<String, Project>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let list: Vec<Project> = net::client()
        .get(format!("{API}/projects"))
        .query(&[("ids", serde_json::to_string(ids)?)])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(list.into_iter().map(|p| (p.id.clone(), p)).collect())
}

/// One file to install: the requested project or one of its dependencies.
/// Shared by Modrinth and CurseForge.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub version_number: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    pub kind: Kind,
    /// Title of the project that needs this one (None for what was requested).
    #[serde(default)]
    pub required_by: Option<String>,
    pub is_root: bool,
    pub url: String,
    pub file_name: String,
    #[serde(default)]
    pub sha1: Option<String>,
    pub size: u64,
    /// Its author doesn't allow launcher downloads: `url` is the page to
    /// download it from in the browser.
    #[serde(default)]
    pub manual: bool,
}

/// Projects currently installed (by files on disk), across all content kinds.
pub(crate) async fn installed_projects(paths: &Paths, inst_id: &str) -> Result<HashSet<String>> {
    let mut out = HashSet::new();
    for k in [Kind::Mod, Kind::Resourcepack, Kind::Shader] {
        out.extend(list(paths, inst_id, k).await?.into_iter().filter_map(|i| i.project_id));
    }
    Ok(out)
}

/// Work out what installing a project involves: the project itself plus any
/// required dependencies that aren't installed yet, each with the reason it is
/// needed. Nothing is downloaded.
pub async fn plan(paths: &Paths, inst_id: &str, kind: Kind, project_id: &str, version_id: Option<&str>) -> Result<Vec<PlanItem>> {
    let inst = instances::get(paths, inst_id).await?;
    let installed = installed_projects(paths, inst_id).await?;
    // Mod ids in the mods folder, to spot dependencies installed from
    // CurseForge (or by hand) that Modrinth doesn't know are there.
    let mods_dir = content_dir(paths, inst_id, Kind::Mod);
    let keys = tokio::task::spawn_blocking(move || super::moddeps::installed_keys(&mods_dir)).await.unwrap_or_default();

    // (project, pinned version, kind, required by)
    let mut queue: Vec<(String, Option<String>, Kind, Option<String>)> =
        vec![(project_id.to_string(), version_id.map(String::from), kind, None)];
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();

    while let Some((pid, vid, k, parent)) = queue.pop() {
        if !seen.insert(pid.clone()) || seen.len() > 60 {
            continue;
        }
        let is_root = pid == project_id;
        // Dependencies already installed are skipped; the requested project is
        // always (re)installed so picking another version works.
        if !is_root && installed.contains(&pid) {
            continue;
        }
        if !is_root && k == Kind::Mod {
            if let Ok(p) = project(&pid).await {
                if super::moddeps::is_installed(&keys, &p.slug, &p.title) {
                    continue;
                }
            }
        }
        let version: Version = match vid {
            Some(v) => net::get_json(&format!("{API}/version/{v}")).await?,
            None => {
                let list = compatible_versions(&inst, &pid, k).await?;
                match best(&list) {
                    Some(v) => v.clone(),
                    None if is_root => {
                        return Err(Error::msg(format!(
                            "No version for Minecraft {} {} is available.",
                            inst.game_version, inst.loader
                        )));
                    }
                    None => continue, // no build for this game version; nothing we can add
                }
            }
        };
        let file = version.primary_file().ok_or_else(|| Error::msg("This version has no files"))?.clone();
        let p = project(&pid).await?;
        let dep_kind = if p.project_type == "mod" { Kind::Mod } else { k };
        for d in version.dependencies.iter().filter(|d| d.dependency_type == "required") {
            let dep_pid = match (&d.project_id, &d.version_id) {
                (Some(p), _) => p.clone(),
                (None, Some(v)) => net::get_json::<Version>(&format!("{API}/version/{v}")).await?.project_id,
                _ => continue,
            };
            queue.push((dep_pid, d.version_id.clone(), dep_kind, Some(p.title.clone())));
        }
        out.push(PlanItem {
            project_id: pid.clone(),
            version_id: version.id.clone(),
            title: p.title.clone(),
            version_number: version.version_number.clone(),
            icon_url: p.icon_url.clone(),
            kind: k,
            required_by: parent,
            is_root,
            url: file.url.clone(),
            file_name: file.filename.clone(),
            sha1: file.hashes.get("sha1").cloned(),
            size: file.size,
            manual: false,
        });
    }
    Ok(out)
}

/// Hosts content may be downloaded from.
const CONTENT_HOSTS: &[&str] = &["cdn.modrinth.com", "edge.forgecdn.net", "mediafilez.forgecdn.net", "github.com", "raw.githubusercontent.com"];

/// Install exactly these items (no extra dependency resolution). Returns titles.
pub async fn install_items(paths: &Paths, inst_id: &str, items: &[PlanItem]) -> Result<Vec<String>> {
    // Files launchers aren't allowed to download go on the instance's
    // "download it yourself" list; the app opens that next.
    let (by_hand, items): (Vec<&PlanItem>, Vec<&PlanItem>) = items.iter().partition(|it| it.manual);
    if !by_hand.is_empty() {
        let mut list = super::manual::load(paths, inst_id).await?;
        for it in by_hand {
            if !it.url.starts_with("https://www.curseforge.com/") || it.file_name.contains(['/', '\\']) || it.file_name.contains("..") {
                return Err(Error::msg(format!("Refusing to send {} to {}", it.title, it.url)));
            }
            list.retain(|f| f.file_name != it.file_name);
            list.push(super::manual::ManualFile {
                name: it.title.clone(),
                url: it.url.clone(),
                file_name: it.file_name.clone(),
                sha1: it.sha1.clone(),
                size: it.size,
                folder: it.kind.folder().to_string(),
                done: false,
            });
        }
        super::manual::save(paths, inst_id, &list).await?;
    }
    let items: Vec<PlanItem> = items.into_iter().cloned().collect();
    let items = &items[..];
    for it in items {
        let host_ok = reqwest::Url::parse(&it.url)
            .ok()
            .filter(|u| u.scheme() == "https")
            .and_then(|u| u.host_str().map(|h| CONTENT_HOSTS.contains(&h)))
            .unwrap_or(false);
        if !host_ok {
            return Err(Error::msg(format!("Refusing to download {} from {}", it.title, it.url)));
        }
        if it.file_name.contains(['/', '\\']) || it.file_name.contains("..") {
            return Err(Error::msg("Invalid file name"));
        }
    }
    // Copies already there (another version of the same project), replaced
    // only once the new files have downloaded, so a failed download keeps
    // the old version.
    let mut old: Vec<(usize, String, bool)> = Vec::new();
    for (i, it) in items.iter().enumerate() {
        for item in list(paths, inst_id, it.kind).await? {
            if item.project_id.as_deref() == Some(&it.project_id) {
                old.push((i, item.file_name, item.enabled));
            }
        }
    }
    let downloads: Vec<Download> = items
        .iter()
        .map(|it| Download::new(it.url.clone(), content_dir(paths, inst_id, it.kind).join(&it.file_name), it.sha1.clone(), Some(it.size)))
        .collect();
    net::download_all(downloads, net::concurrency(), |_| {}).await?;
    // A disabled mod stays disabled when its version changes.
    let stays_off = |i: usize| old.iter().any(|(j, _, enabled)| *j == i && !enabled) && !old.iter().any(|(j, _, enabled)| *j == i && *enabled);
    let final_name = |i: usize| if stays_off(i) { format!("{}.disabled", items[i].file_name) } else { items[i].file_name.clone() };
    // The old files are kept aside (not deleted), so the update can be undone.
    for (i, file_name, _) in &old {
        if *file_name != items[*i].file_name {
            let kind = items[*i].kind;
            if super::undo::keep(paths, inst_id, kind.folder(), file_name, &final_name(*i)).await.is_err() {
                let _ = tokio::fs::remove_file(content_dir(paths, inst_id, kind).join(file_name)).await;
            }
        }
    }
    for (i, it) in items.iter().enumerate() {
        if stays_off(i) {
            let dir = content_dir(paths, inst_id, it.kind);
            let _ = tokio::fs::rename(dir.join(&it.file_name), dir.join(final_name(i))).await;
        }
    }
    let mut store = load_store(paths, inst_id).await?;
    for it in items {
        if let Some(sha) = &it.sha1 {
            store.items.insert(
                sha.to_lowercase(),
                Meta {
                    project_id: it.project_id.clone(),
                    version_id: it.version_id.clone(),
                    title: it.title.clone(),
                    version_number: it.version_number.clone(),
                    icon_url: it.icon_url.clone(),
                },
            );
        }
    }
    save_store(paths, inst_id, &store).await?;
    Ok(items.iter().map(|i| i.title.clone()).collect())
}

/// Install a project and all of its missing required dependencies.
pub async fn install(paths: &Paths, inst_id: &str, kind: Kind, project_id: &str, version_id: Option<&str>) -> Result<Vec<String>> {
    let items = plan(paths, inst_id, kind, project_id, version_id).await?;
    install_items(paths, inst_id, &items).await
}

pub async fn set_enabled(paths: &Paths, inst: &str, kind: Kind, file_name: &str, enabled: bool) -> Result<()> {
    let dir = content_dir(paths, inst, kind);
    let base = file_name.strip_suffix(".disabled").unwrap_or(file_name);
    if base.contains(['/', '\\']) || base.contains("..") {
        return Err(Error::msg("Invalid file name"));
    }
    let (from, to) = if enabled {
        (dir.join(format!("{base}.disabled")), dir.join(base))
    } else {
        (dir.join(base), dir.join(format!("{base}.disabled")))
    };
    if from.exists() {
        tokio::fs::rename(from, to).await?;
    }
    Ok(())
}

pub async fn remove(paths: &Paths, inst: &str, kind: Kind, file_name: &str) -> Result<()> {
    if file_name.contains(['/', '\\']) || file_name.contains("..") || !is_content_file(file_name) {
        return Err(Error::msg("Invalid file name"));
    }
    let path = content_dir(paths, inst, kind).join(file_name);
    if path.exists() {
        tokio::fs::remove_file(path).await?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub file_name: String,
    pub project_id: String,
    pub version_id: String,
    pub version_number: String,
}

/// What changed in a version (Markdown), and its page.
pub async fn changelog(project_id: &str, version_id: &str) -> Result<(Option<String>, String)> {
    let ok = |s: &str| !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric());
    if !ok(project_id) || !ok(version_id) {
        return Err(Error::msg("Bad version id"));
    }
    let v: Value = net::get_json(&format!("{API}/version/{version_id}")).await?;
    let text = v["changelog"].as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from);
    Ok((text, format!("https://modrinth.com/project/{project_id}/version/{version_id}")))
}

/// Newer compatible versions for installed files (one request for all of them).
pub async fn check_updates(paths: &Paths, inst_id: &str, kind: Kind) -> Result<Vec<UpdateInfo>> {
    let inst = instances::get(paths, inst_id).await?;
    let items = list(paths, inst_id, kind).await?;
    let known: Vec<&InstalledItem> = items.iter().filter(|i| i.version_id.is_some()).collect();
    if known.is_empty() {
        return Ok(vec![]);
    }
    let mut body = json!({
        "hashes": known.iter().map(|i| &i.sha1).collect::<Vec<_>>(),
        "algorithm": "sha1",
        "game_versions": [inst.game_version],
    });
    let loaders = loaders_for(&inst, kind);
    if !loaders.is_empty() {
        body["loaders"] = Value::from(loaders);
    }
    let latest: HashMap<String, Version> = net::client()
        .post(format!("{API}/version_files/update"))
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(known
        .into_iter()
        .filter_map(|i| {
            let v = latest.get(&i.sha1)?;
            (Some(&v.id) != i.version_id.as_ref()).then(|| UpdateInfo {
                file_name: i.file_name.clone(),
                project_id: v.project_id.clone(),
                version_id: v.id.clone(),
                version_number: v.version_number.clone(),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mod ids from crash messages turn into installable projects.
    /// cargo test -- --ignored --nocapture finds_projects_for_mod_ids
    #[tokio::test]
    #[ignore = "network"]
    async fn finds_projects_for_mod_ids() {
        let inst = |v: &str, l: &str| -> Instance {
            serde_json::from_value(json!({ "id": "t", "name": "T", "gameVersion": v, "loader": l, "createdAt": "2026-01-01T00:00:00Z" })).unwrap()
        };
        let fabric = inst("1.21.1", "fabric");
        assert_eq!(project_for_mod_id(&fabric, "sodium", "Sodium").await.as_deref(), Some("AANobbMI"));
        assert_eq!(project_for_mod_id(&fabric, "fabric-api", "fabric-api").await.as_deref(), Some("P7dR8mSH"));
        let forge = inst("1.20.1", "forge");
        let kff = project_for_mod_id(&forge, "kotlinforforge", "kotlinforforge").await;
        assert_eq!(kff.as_deref(), Some("ordsPcFz"), "kotlinforforge is kotlin-for-forge");
    }

    /// A CurseForge file that launchers can't download (Entity Culling for
    /// CobbleVerse) is the same file on Modrinth.
    /// cargo test -- --ignored --nocapture finds_same_file_on_modrinth
    #[tokio::test]
    #[ignore = "network"]
    async fn finds_same_file_on_modrinth() {
        let h = "1a28b70786ba203b160a406080232ef9834e500c".to_string();
        let found = same_files(&[h.clone(), "0000000000000000000000000000000000000000".into()]).await.unwrap();
        assert_eq!(found.len(), 1);
        assert!(found[&h].url.starts_with("https://cdn.modrinth.com/data/NNAgCjsB/"));
    }

    /// cargo test -- --ignored --nocapture modrinth_plan_reasons
    #[tokio::test]
    #[ignore]
    async fn modrinth_plan_reasons() {
        let dir = std::env::temp_dir().join(format!("peak-plan-test-{}", std::process::id()));
        let paths = Paths::new(dir.clone());
        let inst = instances::create(&paths, "Plan", "1.21.4", "fabric", Some("0.16.9".into())).await.unwrap();
        let items = plan(&paths, &inst.id, Kind::Mod, "mOgUt4GM", None).await.unwrap();
        for i in &items {
            println!("{} {} root={} required_by={:?}", i.title, i.version_number, i.is_root, i.required_by);
        }
        assert!(items[0].is_root && items[0].required_by.is_none());
        assert!(items.iter().skip(1).all(|i| !i.is_root && i.required_by.is_some()));
        // Nothing is downloaded by planning.
        assert!(list(&paths, &inst.id, Kind::Mod).await.unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
