//! CurseForge (https://docs.curseforge.com/rest-api). Needs an API key from
//! https://console.curseforge.com, set in Settings or at build time with
//! PEAK_CF_API_KEY.
//!
//! Results are converted to the same shapes as Modrinth's so the UI can show
//! both. CurseForge ids are prefixed with "cf:" ("cf:238222") so every other
//! command knows where an id came from.
//!
//! Authors can turn off third-party downloads for their files. Those files
//! have no download URL; we never work around that and instead tell the user
//! where to download them by hand.

use super::apicache::{self, ResponseCache};
use super::install::{InstallProgress, ProgressFn, stage};
use super::instances::{self, Instance, PackInfo};
use super::modrinth::{self, Dependency, GalleryImage, Kind, Meta, PlanItem, Project, SearchHit, SearchResult, UpdateInfo, Version, VersionFile};
use super::net::{self, Download};
use super::paths::Paths;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

const API: &str = "https://api.curseforge.com/v1";
const MINECRAFT: u32 = 432;
pub const PREFIX: &str = "cf:";

/// Built-in key, if the launcher was built with one.
pub const BUILT_IN_KEY: Option<&str> = option_env!("PEAK_CF_API_KEY");

/// A proxy that adds the API key on the server, so release builds don't ship
/// one (e.g. "https://api.peaklauncher.com/v1"). Set with PEAK_CF_PROXY.
pub const PROXY: Option<&str> = option_env!("PEAK_CF_PROXY");

/// Can CurseForge be used without the user entering their own key?
pub fn works_without_user_key() -> bool {
    BUILT_IN_KEY.map(|k| !k.trim().is_empty()).unwrap_or(false) || PROXY.map(|p| !p.trim().is_empty()).unwrap_or(false)
}

pub fn is_cf(id: &str) -> bool {
    id.starts_with(PREFIX)
}

fn raw_id(id: &str) -> Result<u64> {
    id.trim_start_matches(PREFIX).parse().map_err(|_| Error::msg(format!("Bad CurseForge id '{id}'")))
}

fn class_id(kind: Option<Kind>) -> u32 {
    match kind {
        Some(Kind::Mod) => 6,
        Some(Kind::Resourcepack) => 12,
        Some(Kind::Shader) => 6552,
        None => 4471, // modpacks
    }
}

/// CurseForge's class for data packs.
const DATA_PACKS: u32 = 6945;

fn kind_for_class(class: Option<u32>) -> Kind {
    match class {
        Some(12) => Kind::Resourcepack,
        Some(6552) => Kind::Shader,
        _ => Kind::Mod,
    }
}

/// CurseForge's mod loader ids for search.
fn loader_type(loader: &str) -> Option<u32> {
    match loader {
        "forge" => Some(1),
        "fabric" | "quilt" => Some(4),
        "neoforge" => Some(6),
        _ => None,
    }
}

/// Loader labels that appear in a file's gameVersions list.
fn accepted_labels(loader: &str) -> &'static [&'static str] {
    match loader {
        "fabric" => &["fabric"],
        "quilt" => &["quilt", "fabric"],
        "forge" => &["forge"],
        "neoforge" => &["neoforge"],
        _ => &[],
    }
}

// ---------------------------------------------------------------- HTTP

/// Recent GET responses, shared by every Client (one is made per command).
static CACHE: LazyLock<Mutex<ResponseCache>> = LazyLock::new(|| Mutex::new(ResponseCache::new(apicache::CAPACITY)));

/// Forget cached responses, e.g. when the API key (and so the server) changes.
pub fn clear_cache() {
    CACHE.lock().unwrap().clear();
}

/// How long a GET response may be reused, by endpoint; None means never.
fn cache_ttl(path: &str) -> Option<Duration> {
    const MIN: u64 = 60;
    let id = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    let secs = match parts.as_slice() {
        ["mods", "search"] => 5 * MIN,
        ["mods", m, "files"] if id(m) => 5 * MIN,
        ["mods", m] if id(m) => 10 * MIN,
        ["mods", m, "description"] if id(m) => 60 * MIN,
        ["mods", m, "files", f] if id(m) && id(f) => 60 * MIN,
        _ => return None,
    };
    Some(Duration::from_secs(secs))
}

/// Talks to CurseForge directly with a key, or through the key-holding proxy.
#[derive(Clone)]
pub struct Client {
    base: String,
    key: Option<String>,
    /// Skip cached responses (still refreshes the cache).
    fresh: bool,
}

impl Client {
    /// The user's own key wins, then a key built into the app, then the proxy.
    pub fn new(settings_key: &str) -> Result<Client> {
        Self::choose(settings_key, BUILT_IN_KEY, PROXY)
    }

    fn choose(settings_key: &str, built_in: Option<&str>, proxy: Option<&str>) -> Result<Client> {
        let nonempty = |s: Option<&str>| s.map(str::trim).filter(|s| !s.is_empty()).map(String::from);
        if let Some(key) = nonempty(Some(settings_key)).or(nonempty(built_in)) {
            return Ok(Client { base: API.to_string(), key: Some(key), fresh: false });
        }
        if let Some(proxy) = nonempty(proxy) {
            return Ok(Client { base: proxy.trim_end_matches('/').to_string(), key: None, fresh: false });
        }
        Err(Error::msg("CurseForge needs an API key. Add yours in Settings → Integrations."))
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let rb = net::client().request(method, format!("{}{path}", self.base));
        match &self.key {
            Some(k) => rb.header("x-api-key", k),
            None => rb,
        }
    }

    /// A copy that always asks the server, for results that must be current
    /// (update checks).
    pub fn fresh(&self) -> Client {
        Client { fresh: true, ..self.clone() }
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, String)]) -> Result<T> {
        Ok(self.get_json::<Data<T>>(path, query).await?.data)
    }

    /// GET and parse the whole response body, reusing a recent response when
    /// the endpoint allows it.
    async fn get_json<T: DeserializeOwned>(&self, path: &str, query: &[(&str, String)]) -> Result<T> {
        let req = self.request(reqwest::Method::GET, path).query(query).build()?;
        let url = req.url().to_string();
        let ttl = cache_ttl(path);
        if ttl.is_some() && !self.fresh {
            let hit = CACHE.lock().unwrap().get(&url, Instant::now());
            if let Some(v) = hit.and_then(|body| serde_json::from_str(&body).ok()) {
                return Ok(v);
            }
        }
        let body = Self::body(net::client().execute(req).await?).await?;
        let value: T = serde_json::from_str(&body)?;
        // Only responses that came back OK and parsed get here.
        if let Some(ttl) = ttl {
            CACHE.lock().unwrap().put(url, body, ttl, Instant::now());
        }
        Ok(value)
    }

    async fn body(resp: reqwest::Response) -> Result<String> {
        match resp.status().as_u16() {
            401 | 403 => Err(Error::msg("CurseForge refused the request. If you added your own API key, check it in Settings → Integrations.")),
            _ => Ok(resp.error_for_status()?.text().await?),
        }
    }

    async fn post<T: DeserializeOwned>(&self, path: &str, body: serde_json::Value) -> Result<T> {
        let resp = self.request(reqwest::Method::POST, path).json(&body).send().await?;
        Self::parse(resp).await
    }

    /// Look files up by CurseForge fingerprint. Returns fingerprint -> (modId, fileId).
    pub async fn fingerprint_matches(&self, fingerprints: &[u32]) -> Result<Vec<(u32, (u64, u64))>> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Matches {
            #[serde(default)]
            exact_matches: Vec<Match>,
        }
        #[derive(Deserialize)]
        struct Match {
            file: MatchFile,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct MatchFile {
            id: u64,
            mod_id: u64,
            file_fingerprint: u32,
        }
        let mut out = Vec::new();
        for chunk in fingerprints.chunks(500) {
            let m: Matches = self.post(&format!("/fingerprints/{MINECRAFT}"), json!({ "fingerprints": chunk })).await?;
            out.extend(m.exact_matches.into_iter().map(|x| (x.file.file_fingerprint, (x.file.mod_id, x.file.id))));
        }
        Ok(out)
    }

    async fn parse<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T> {
        match resp.status().as_u16() {
            401 | 403 => Err(Error::msg("CurseForge refused the request. If you added your own API key, check it in Settings → Integrations.")),
            _ => Ok(resp.error_for_status()?.json::<Data<T>>().await?.data),
        }
    }
}

#[derive(Deserialize)]
struct Data<T> {
    data: T,
}

// ---------------------------------------------------------------- API types

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfMod {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub download_count: f64,
    #[serde(default)]
    pub thumbs_up_count: Option<f64>,
    #[serde(default)]
    pub logo: Option<CfLogo>,
    #[serde(default)]
    pub authors: Vec<CfName>,
    #[serde(default)]
    pub categories: Vec<CfName>,
    #[serde(default)]
    pub date_modified: String,
    #[serde(default)]
    pub links: Option<CfLinks>,
    #[serde(default)]
    pub screenshots: Vec<CfShot>,
    #[serde(default)]
    pub class_id: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfLogo {
    // Some packs (e.g. Prominence II) come with an empty thumbnail and only
    // the full logo; empty counts as missing, so the full logo is used.
    #[serde(default, deserialize_with = "non_empty")]
    pub thumbnail_url: Option<String>,
    #[serde(default, deserialize_with = "non_empty")]
    pub url: Option<String>,
}

fn non_empty<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Option<String>, D::Error> {
    let v: Option<String> = serde::Deserialize::deserialize(d)?;
    Ok(v.filter(|s| !s.trim().is_empty()))
}

#[derive(Debug, Clone, Deserialize)]
pub struct CfName {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfLinks {
    #[serde(default)]
    pub website_url: Option<String>,
    #[serde(default)]
    pub wiki_url: Option<String>,
    #[serde(default)]
    pub issues_url: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CfShot {
    pub url: String,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfFile {
    pub id: u64,
    pub mod_id: u64,
    pub display_name: String,
    pub file_name: String,
    #[serde(default)]
    pub release_type: u8,
    #[serde(default)]
    pub file_date: String,
    #[serde(default)]
    pub file_length: u64,
    #[serde(default)]
    pub download_count: f64,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub hashes: Vec<CfHash>,
    #[serde(default)]
    pub dependencies: Vec<CfDep>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CfHash {
    pub value: String,
    pub algo: u8,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfDep {
    pub mod_id: u64,
    pub relation_type: u8,
}

const REQUIRED_DEPENDENCY: u8 = 3;

impl CfFile {
    fn sha1(&self) -> Option<String> {
        self.hashes.iter().find(|h| h.algo == 1).map(|h| h.value.to_lowercase())
    }
    fn works_with(&self, game_version: &str, labels: &[&str]) -> bool {
        let versions_ok = self.game_versions.iter().any(|v| v == game_version);
        let loader_ok = labels.is_empty() || self.game_versions.iter().any(|v| labels.contains(&v.to_lowercase().as_str()));
        versions_ok && loader_ok
    }
}

// ---------------------------------------------------------------- conversions

fn to_hit(m: CfMod) -> SearchHit {
    SearchHit {
        project_id: format!("{PREFIX}{}", m.id),
        slug: m.slug,
        title: m.name,
        description: m.summary,
        icon_url: m.logo.and_then(|l| l.thumbnail_url.or(l.url)),
        downloads: m.download_count as u64,
        follows: m.thumbs_up_count.unwrap_or(0.0) as u64,
        author: m.authors.first().map(|a| a.name.clone()).unwrap_or_default(),
        categories: m.categories.into_iter().map(|c| c.name).collect(),
        date_modified: m.date_modified,
        client_side: None,
        server_side: None,
    }
}

fn project_type(class: Option<u32>) -> &'static str {
    match class {
        Some(12) => "resourcepack",
        Some(6552) => "shader",
        Some(4471) => "modpack",
        _ => "mod",
    }
}

fn to_project(m: CfMod, body: String) -> Project {
    let links = m.links.clone();
    Project {
        id: format!("{PREFIX}{}", m.id),
        slug: m.slug.clone(),
        title: m.name.clone(),
        description: m.summary.clone(),
        body,
        icon_url: m.logo.as_ref().and_then(|l| l.url.clone().or(l.thumbnail_url.clone())),
        downloads: m.download_count as u64,
        followers: m.thumbs_up_count.unwrap_or(0.0) as u64,
        categories: m.categories.iter().map(|c| c.name.clone()).collect(),
        project_type: project_type(m.class_id).to_string(),
        source_url: links.as_ref().and_then(|l| l.source_url.clone()),
        issues_url: links.as_ref().and_then(|l| l.issues_url.clone()),
        wiki_url: links.as_ref().and_then(|l| l.wiki_url.clone()),
        discord_url: None,
        gallery: m.screenshots.into_iter().map(|s| GalleryImage { url: s.url, featured: false, title: s.title }).collect(),
        client_side: None,
        updated: Some(m.date_modified),
        website_url: links.and_then(|l| l.website_url),
    }
}

fn to_version(f: &CfFile) -> Version {
    let loaders: Vec<String> = f
        .game_versions
        .iter()
        .map(|v| v.to_lowercase())
        .filter(|v| ["fabric", "forge", "neoforge", "quilt"].contains(&v.as_str()))
        .collect();
    let game_versions: Vec<String> =
        f.game_versions.iter().filter(|v| v.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)).cloned().collect();
    let mut hashes = HashMap::new();
    if let Some(sha) = f.sha1() {
        hashes.insert("sha1".to_string(), sha);
    }
    Version {
        id: f.id.to_string(),
        project_id: format!("{PREFIX}{}", f.mod_id),
        name: f.display_name.clone(),
        version_number: f.display_name.clone(),
        version_type: match f.release_type {
            2 => "beta",
            3 => "alpha",
            _ => "release",
        }
        .to_string(),
        date_published: f.file_date.clone(),
        downloads: f.download_count as u64,
        game_versions,
        loaders,
        files: vec![VersionFile {
            url: f.download_url.clone().unwrap_or_default(),
            filename: f.file_name.clone(),
            primary: true,
            size: f.file_length,
            hashes,
        }],
        dependencies: f
            .dependencies
            .iter()
            .filter(|d| d.relation_type == REQUIRED_DEPENDENCY)
            .map(|d| Dependency {
                version_id: None,
                project_id: Some(format!("{PREFIX}{}", d.mod_id)),
                dependency_type: "required".into(),
            })
            .collect(),
    }
}

// ---------------------------------------------------------------- search & details

fn sort_field(sort: &str) -> u32 {
    match sort {
        "downloads" => 6,
        "updated" => 3,
        "newest" => 11,
        "follows" => 12,
        _ => 2, // popularity, the closest to "relevance"
    }
}

/// Search content for an instance (`kind` set) or modpacks (`kind` None).
pub async fn search(cf: &Client, inst: Option<&Instance>, kind: Option<Kind>, query: &str, sort: &str, offset: u32) -> Result<SearchResult> {
    #[derive(Deserialize)]
    struct Page {
        data: Vec<CfMod>,
        pagination: Pagination,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Pagination {
        total_count: u64,
    }
    // CurseForge only pages through the first 10,000 results.
    let offset = offset.min(9_976);
    let mut q: Vec<(&str, String)> = vec![
        ("gameId", MINECRAFT.to_string()),
        ("classId", class_id(kind).to_string()),
        ("searchFilter", query.to_string()),
        ("sortField", sort_field(sort).to_string()),
        ("sortOrder", "desc".into()),
        ("index", offset.to_string()),
        ("pageSize", "24".into()),
    ];
    if let Some(inst) = inst {
        q.push(("gameVersion", inst.game_version.clone()));
        if kind == Some(Kind::Mod) {
            let lt = loader_type(&inst.loader)
                .ok_or_else(|| Error::msg("Vanilla instances can't run mods. Add a mod loader in the instance's settings."))?;
            q.push(("modLoaderType", lt.to_string()));
        }
    }
    let page: Page = cf.get_json("/mods/search", &q).await?;
    Ok(SearchResult {
        total_hits: page.pagination.total_count.min(10_000),
        offset: offset as u64,
        hits: page.data.into_iter().map(to_hit).collect(),
    })
}

pub async fn project(cf: &Client, id: &str) -> Result<Project> {
    let mid = raw_id(id)?;
    let (mod_path, desc_path) = (format!("/mods/{mid}"), format!("/mods/{mid}/description"));
    let (m, body) = tokio::join!(cf.get::<CfMod>(&mod_path, &[]), cf.get::<String>(&desc_path, &[]));
    Ok(to_project(m?, body.unwrap_or_default()))
}

async fn files(cf: &Client, mod_id: u64, game_version: Option<&str>) -> Result<Vec<CfFile>> {
    let mut q = vec![("pageSize", "50".to_string())];
    if let Some(gv) = game_version {
        q.push(("gameVersion", gv.to_string()));
    }
    let mut list: Vec<CfFile> = cf.get(&format!("/mods/{mod_id}/files"), &q).await?;
    list.sort_by(|a, b| b.file_date.cmp(&a.file_date));
    Ok(list)
}

/// Files of a mod that work with an instance, newest first.
pub async fn compatible_versions(cf: &Client, inst: &Instance, id: &str, kind: Kind) -> Result<Vec<Version>> {
    let labels = if kind == Kind::Mod { accepted_labels(&inst.loader) } else { &[] };
    let list = files(cf, raw_id(id)?, Some(&inst.game_version)).await?;
    Ok(list.iter().filter(|f| f.works_with(&inst.game_version, labels)).map(to_version).collect())
}

/// Every file of a modpack, newest first.
pub async fn modpack_versions(cf: &Client, id: &str) -> Result<Vec<Version>> {
    Ok(files(cf, raw_id(id)?, None).await?.iter().map(to_version).collect())
}

// ---------------------------------------------------------------- install into an instance

/// What installing a CurseForge project involves: the project plus any
/// required dependencies that aren't installed yet, each with the reason it is
/// needed. Nothing is downloaded.
pub async fn plan(cf: &Client, paths: &Paths, inst_id: &str, kind: Kind, id: &str, file_id: Option<&str>) -> Result<Vec<PlanItem>> {
    let inst = instances::get(paths, inst_id).await?;
    let installed = modrinth::installed_projects(paths, inst_id).await?;
    let mods_dir = modrinth::content_dir(paths, inst_id, Kind::Mod);
    let keys = tokio::task::spawn_blocking(move || super::moddeps::installed_keys(&mods_dir)).await.unwrap_or_default();
    let root = raw_id(id)?;
    let first_file = file_id.map(|f| f.parse::<u64>()).transpose().map_err(|_| Error::msg("Bad file id"))?;

    let mut queue: Vec<(u64, Option<u64>, Option<String>)> = vec![(root, first_file, None)];
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    while let Some((mid, fid, parent)) = queue.pop() {
        if !seen.insert(mid) || seen.len() > 60 {
            continue;
        }
        let pid = format!("{PREFIX}{mid}");
        let is_root = mid == root;
        if !is_root && installed.contains(&pid) {
            continue;
        }
        let m: CfMod = cf.get(&format!("/mods/{mid}"), &[]).await?;
        let k = if is_root { kind } else { kind_for_class(m.class_id) };
        // A dependency already there from Modrinth (or added by hand).
        if !is_root && k == Kind::Mod && super::moddeps::is_installed(&keys, &m.slug, &m.name) {
            continue;
        }
        let file: CfFile = match fid {
            Some(f) => cf.get(&format!("/mods/{mid}/files/{f}"), &[]).await?,
            None => {
                let labels = if k == Kind::Mod { accepted_labels(&inst.loader) } else { &[] };
                let list = files(cf, mid, Some(&inst.game_version)).await?;
                let compatible: Vec<&CfFile> = list.iter().filter(|f| f.works_with(&inst.game_version, labels)).collect();
                match compatible.iter().find(|f| f.release_type == 1).or(compatible.first()) {
                    Some(f) => (*f).clone(),
                    None if is_root => {
                        return Err(Error::msg(format!("No version for Minecraft {} {} is available.", inst.game_version, inst.loader)));
                    }
                    None => continue,
                }
            }
        };
        let (url, manual) = match file.download_url.clone() {
            Some(url) => (url, false),
            // Not for launchers on CurseForge: the identical file from
            // Modrinth if it's there, else downloaded in the browser.
            None => {
                let same = match file.sha1() {
                    Some(h) => modrinth::same_files(std::slice::from_ref(&h)).await.ok().and_then(|found| found.get(&h).map(|f| f.url.clone())),
                    None => None,
                };
                match same {
                    Some(url) => (url, false),
                    None => (download_page(Some(&m), mid, file.id), true),
                }
            }
        };
        for d in file.dependencies.iter().filter(|d| d.relation_type == REQUIRED_DEPENDENCY) {
            queue.push((d.mod_id, None, Some(m.name.clone())));
        }
        out.push(PlanItem {
            project_id: pid,
            version_id: file.id.to_string(),
            title: m.name.clone(),
            version_number: file.display_name.clone(),
            icon_url: m.logo.as_ref().and_then(|l| l.thumbnail_url.clone().or(l.url.clone())),
            kind: k,
            required_by: parent,
            is_root,
            url,
            file_name: file.file_name.clone(),
            sha1: file.sha1(),
            size: file.file_length,
            manual,
        });
    }
    Ok(out)
}

/// Install a CurseForge project and all of its missing required dependencies.
pub async fn install(cf: &Client, paths: &Paths, inst_id: &str, kind: Kind, id: &str, file_id: Option<&str>) -> Result<Vec<String>> {
    let items = plan(cf, paths, inst_id, kind, id, file_id).await?;
    modrinth::install_items(paths, inst_id, &items).await
}

async fn record(paths: &Paths, inst_id: &str, items: Vec<(Option<String>, Meta)>) -> Result<()> {
    let mut store = modrinth::load_store(paths, inst_id).await?;
    for (sha, meta) in items {
        if let Some(sha) = sha {
            store.items.insert(sha, meta);
        }
    }
    modrinth::save_store(paths, inst_id, &store).await
}

/// Newer compatible files for CurseForge content in an instance.
pub async fn check_updates(cf: &Client, paths: &Paths, inst_id: &str, kind: Kind) -> Result<Vec<UpdateInfo>> {
    // Always ask for current file lists here, never cached ones.
    let cf = &cf.fresh();
    let inst = instances::get(paths, inst_id).await?;
    let labels = if kind == Kind::Mod { accepted_labels(&inst.loader) } else { &[] };
    let mut out = Vec::new();
    for item in modrinth::list(paths, inst_id, kind).await? {
        let (Some(pid), Some(vid)) = (&item.project_id, &item.version_id) else { continue };
        if !is_cf(pid) {
            continue;
        }
        let list = files(cf, raw_id(pid)?, Some(&inst.game_version)).await?;
        let newest = list.iter().filter(|f| f.works_with(&inst.game_version, labels) && f.release_type == 1).max_by(|a, b| a.file_date.cmp(&b.file_date));
        if let Some(f) = newest {
            if &f.id.to_string() != vid && f.download_url.is_some() {
                out.push(UpdateInfo {
                    file_name: item.file_name.clone(),
                    project_id: pid.clone(),
                    version_id: f.id.to_string(),
                    version_number: f.display_name.clone(),
                });
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------- modpacks

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub minecraft: ManifestMc,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub files: Vec<ManifestFile>,
    #[serde(default = "default_overrides")]
    pub overrides: String,
}

fn default_overrides() -> String {
    "overrides".into()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestMc {
    pub version: String,
    #[serde(default)]
    pub mod_loaders: Vec<ManifestLoader>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestLoader {
    pub id: String,
    #[serde(default)]
    pub primary: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestFile {
    #[serde(rename = "projectID")]
    pub project_id: u64,
    #[serde(rename = "fileID")]
    pub file_id: u64,
    #[serde(default = "yes")]
    pub required: bool,
}

fn yes() -> bool {
    true
}

/// "forge-47.3.0" -> ("forge", "1.20.1-47.3.0"), "fabric-0.16.9" -> ("fabric", "0.16.9").
pub fn parse_loader(id: &str, mc: &str) -> Option<(&'static str, String)> {
    let (name, version) = id.split_once('-')?;
    Some(match name {
        "forge" => ("forge", if version.starts_with(&format!("{mc}-")) { version.to_string() } else { format!("{mc}-{version}") }),
        "neoforge" => ("neoforge", version.to_string()),
        "fabric" => ("fabric", version.to_string()),
        "quilt" => ("quilt", version.to_string()),
        _ => return None,
    })
}

/// Download a modpack's zip (best release unless a file is given).
/// The pack and the file to install (the given one, else the newest release).
async fn pack_file(cf: &Client, id: &str, file_id: Option<&str>) -> Result<(u64, CfMod, CfFile)> {
    let mid = raw_id(id)?;
    let m: CfMod = cf.get(&format!("/mods/{mid}"), &[]).await?;
    let file: CfFile = match file_id {
        Some(f) => cf.get(&format!("/mods/{mid}/files/{f}"), &[]).await?,
        None => {
            let list = files(cf, mid, None).await?;
            list.iter().find(|f| f.release_type == 1).or(list.first()).cloned().ok_or_else(|| Error::msg("This modpack has no files"))?
        }
    };
    Ok((mid, m, file))
}

fn pack_info(mid: u64, m: &CfMod, file: &CfFile) -> PackInfo {
    PackInfo {
        name: m.name.clone(),
        version_number: file.display_name.clone(),
        project_id: Some(format!("{PREFIX}{mid}")),
        version_id: Some(file.id.to_string()),
        banner_url: m.screenshots.first().map(|s| s.url.clone()),
        description: Some(m.summary.clone()).filter(|d| !d.is_empty()),
        icon_url: m.logo.as_ref().and_then(|l| l.thumbnail_url.clone().or(l.url.clone())),
        ..Default::default()
    }
}

/// A pack whose author turned off launcher downloads: what to download by hand.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualPack {
    pub name: String,
    pub version: String,
    pub file_id: String,
    pub file_name: String,
    pub size: u64,
    pub sha1: Option<String>,
    /// The file's page on CurseForge, which starts the download.
    pub download_page: String,
    pub icon_url: Option<String>,
}

/// If this pack has to be downloaded by hand, what and from where; else None.
pub async fn manual_pack(cf: &Client, id: &str, file_id: Option<&str>) -> Result<Option<ManualPack>> {
    let (_, m, file) = pack_file(cf, id, file_id).await?;
    if file.download_url.is_some() {
        return Ok(None);
    }
    Ok(Some(ManualPack {
        download_page: format!("https://www.curseforge.com/minecraft/modpacks/{}/download/{}", m.slug, file.id),
        name: m.name.clone(),
        version: file.display_name.clone(),
        file_id: file.id.to_string(),
        file_name: file.file_name.clone(),
        size: file.file_length,
        sha1: file.sha1(),
        icon_url: m.logo.as_ref().and_then(|l| l.thumbnail_url.clone().or(l.url.clone())),
    }))
}

/// The pack's details for a zip the player downloaded by hand, after checking
/// it's the right file.
pub async fn manual_pack_info(cf: &Client, id: &str, file_id: &str, zip: &Path) -> Result<PackInfo> {
    let (mid, m, file) = pack_file(cf, id, Some(file_id)).await?;
    if let Some(want) = file.sha1() {
        let got = net::sha1_file(zip).await?;
        if !got.eq_ignore_ascii_case(&want) {
            return Err(Error::msg(format!("That file isn't {} ({}). Download it again from CurseForge.", m.name, file.display_name)));
        }
    }
    Ok(pack_info(mid, &m, &file))
}

pub async fn fetch_modpack(cf: &Client, paths: &Paths, id: &str, file_id: Option<&str>) -> Result<(PathBuf, PackInfo)> {
    let (mid, m, file) = pack_file(cf, id, file_id).await?;
    let url = file.download_url.clone().ok_or_else(|| {
        Error::msg(format!(
            "{} can't be downloaded by launchers. Download the zip from https://www.curseforge.com/minecraft/modpacks/{} and import it instead.",
            m.name, m.slug
        ))
    })?;
    let dest = paths.cache().join("modpacks").join(&file.file_name);
    net::download_all(vec![Download::new(url, dest.clone(), file.sha1(), Some(file.file_length))], 1, |_| {}).await?;
    Ok((dest, pack_info(mid, &m, &file)))
}

fn read_manifest(zip_path: &Path) -> Result<Manifest> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(zip_path)?)?;
    let mut entry = zip.by_name("manifest.json").map_err(|_| Error::msg("This isn't a CurseForge modpack (no manifest.json)"))?;
    let mut buf = String::new();
    entry.read_to_string(&mut buf)?;
    Ok(serde_json::from_str(&buf)?)
}

pub fn is_curseforge_pack(zip_path: &Path) -> bool {
    read_manifest(zip_path).is_ok()
}

/// Read a CurseForge pack zip and create its (not yet downloaded) instance.
pub async fn prepare_modpack(paths: &Paths, zip_path: &Path, pack: Option<PackInfo>) -> Result<(Instance, Manifest)> {
    let p = zip_path.to_owned();
    let manifest = tokio::task::spawn_blocking(move || read_manifest(&p)).await.map_err(|e| Error::msg(e.to_string()))??;
    let mc = manifest.minecraft.version.clone();
    let primary = manifest.minecraft.mod_loaders.iter().find(|l| l.primary).or(manifest.minecraft.mod_loaders.first());
    let (loader, loader_version) = match primary.and_then(|l| parse_loader(&l.id, &mc)) {
        Some((l, v)) => (l, Some(v)),
        None => ("vanilla", None),
    };
    let name = if manifest.name.trim().is_empty() { "CurseForge modpack".to_string() } else { manifest.name.clone() };
    let mut inst = instances::create(paths, &name, &mc, loader, loader_version).await?;
    inst.pack = Some(pack.unwrap_or_else(|| PackInfo {
        name: name.clone(),
        version_number: manifest.version.clone(),
        project_id: None,
        version_id: None,
        icon_url: None,
        banner_url: None,
        description: None,
        ..Default::default()
    }));
    instances::save(paths, &inst).await?;
    Ok((inst, manifest))
}

#[derive(Debug, Clone, Serialize)]
pub struct Blocked {
    pub name: String,
    pub url: String,
    /// What the downloaded file is called, its hash, and where it belongs.
    pub file_name: String,
    pub sha1: Option<String>,
    pub size: u64,
    pub folder: String,
}

/// Download a pack's files and copy its overrides. Returns files whose authors
/// don't allow launcher downloads, so the user can fetch them by hand.
pub async fn populate_modpack(cf: &Client, paths: &Paths, inst: &Instance, zip_path: &Path, manifest: &Manifest, progress: &ProgressFn) -> Result<Vec<Blocked>> {
    let wanted: Vec<&ManifestFile> = manifest.files.iter().filter(|f| f.required).collect();
    let mut blocked = Vec::new();
    if !wanted.is_empty() {
        progress(InstallProgress { stage: "Resolving modpack files", done: 0, total: 0, bytes: 0 });
        let file_ids: Vec<u64> = wanted.iter().map(|f| f.file_id).collect();
        let mod_ids: Vec<u64> = wanted.iter().map(|f| f.project_id).collect::<HashSet<_>>().into_iter().collect();
        let mut files: Vec<CfFile> = Vec::new();
        for chunk in file_ids.chunks(500) {
            files.extend(cf.post::<Vec<CfFile>>("/mods/files", json!({ "fileIds": chunk })).await?);
        }
        let mut mods: HashMap<u64, CfMod> = HashMap::new();
        for chunk in mod_ids.chunks(500) {
            for m in cf.post::<Vec<CfMod>>("/mods", json!({ "modIds": chunk })).await? {
                mods.insert(m.id, m);
            }
        }

        let mut downloads = Vec::new();
        let mut metas = Vec::new();
        // Files CurseForge won't hand to launchers, with what they'd be recorded as.
        let mut held: Vec<(Blocked, Meta)> = Vec::new();
        let game = instances::game_dir(paths, &inst.id);
        for f in &files {
            let m = mods.get(&f.mod_id);
            let class = m.and_then(|m| m.class_id);
            let kind = kind_for_class(class);
            // Data packs go in a datapacks folder (as the CurseForge app and
            // Modrinth packs do); packs load them with a mod like Paxi.
            let folder = if class == Some(DATA_PACKS) { "datapacks" } else { kind.folder() };
            let name = m.map(|m| m.name.clone()).unwrap_or_else(|| f.display_name.clone());
            let meta = Meta {
                project_id: format!("{PREFIX}{}", f.mod_id),
                version_id: f.id.to_string(),
                title: name.clone(),
                version_number: f.display_name.clone(),
                icon_url: m.and_then(|m| m.logo.as_ref().and_then(|l| l.thumbnail_url.clone().or(l.url.clone()))),
            };
            match &f.download_url {
                Some(url) => {
                    downloads.push(Download::new(
                        url.clone(),
                        game.join(folder).join(&f.file_name),
                        f.sha1(),
                        Some(f.file_length),
                    ));
                    metas.push((f.sha1(), meta));
                }
                None => held.push((
                    Blocked {
                        url: download_page(m, f.mod_id, f.id),
                        name,
                        file_name: f.file_name.clone(),
                        sha1: f.sha1(),
                        size: f.file_length,
                        folder: folder.to_string(),
                    },
                    meta,
                )),
            }
        }

        // The identical file on Modrinth (same SHA-1) can be downloaded
        // instead; only what isn't there is left for the player.
        let hashes: Vec<String> = held.iter().filter_map(|(b, _)| b.sha1.clone()).collect();
        let on_modrinth = modrinth::same_files(&hashes).await.unwrap_or_default();
        for (b, meta) in held {
            match b.sha1.as_ref().and_then(|h| on_modrinth.get(h)) {
                Some(file) => {
                    downloads.push(Download::new(file.url.clone(), game.join(&b.folder).join(&b.file_name), b.sha1.clone(), Some(b.size)));
                    metas.push((b.sha1.clone(), meta));
                }
                None => blocked.push(b),
            }
        }
        net::download_all(downloads, net::concurrency(), stage(progress, "Downloading modpack")).await?;
        record(paths, &inst.id, metas).await?;
    }

    progress(InstallProgress { stage: "Copying modpack files", done: 0, total: 0, bytes: 0 });
    let game = instances::game_dir(paths, &inst.id);
    let (src, prefix) = (zip_path.to_owned(), manifest.overrides.trim_matches('/').to_string());
    tokio::task::spawn_blocking(move || -> Result<()> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&src)?)?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            let Some(name) = entry.enclosed_name() else { continue };
            let Ok(rel) = name.strip_prefix(&prefix) else { continue };
            if entry.is_dir() || rel.as_os_str().is_empty() || rel.components().any(|c| !matches!(c, Component::Normal(_))) {
                continue;
            }
            let out = game.join(rel);
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::io::copy(&mut entry, &mut std::fs::File::create(&out)?)?;
        }
        Ok(())
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))??;
    Ok(blocked)
}

/// The file's download page on curseforge.com, which starts the download
/// straight away (the /files/ page only shows details). Built from the
/// project's own page address, so mods, data packs, resource packs and the
/// rest each get the right section of the site.
pub fn download_page(m: Option<&CfMod>, mod_id: u64, file_id: u64) -> String {
    let site = m
        .and_then(|m| m.links.as_ref()?.website_url.clone())
        .filter(|u| u.starts_with("https://www.curseforge.com/minecraft/"))
        .or_else(|| {
            let m = m.filter(|m| !m.slug.is_empty())?;
            let section = match m.class_id {
                Some(12) => "texture-packs",
                Some(6552) => "shaders",
                Some(DATA_PACKS) => "data-packs",
                Some(17) => "worlds",
                Some(4546) => "customization",
                _ => "mc-mods",
            };
            Some(format!("https://www.curseforge.com/minecraft/{section}/{}", m.slug))
        });
    match site {
        Some(site) => format!("{}/download/{file_id}", site.trim_end_matches('/')),
        // Without either, send them to the project; curseforge.com redirects by id.
        None => format!("https://www.curseforge.com/projects/{mod_id}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_links_use_the_right_part_of_the_site() {
        let m = |v: serde_json::Value| serde_json::from_value::<CfMod>(v).unwrap();
        let base = |class: u32, site: Option<&str>| {
            let mut v = serde_json::json!({ "id": 1, "name": "X", "slug": "tierify-prominence-tweaks", "summary": "", "downloadCount": 0,
                "dateModified": "", "classId": class });
            if let Some(site) = site {
                v["links"] = serde_json::json!({ "websiteUrl": site });
            }
            v
        };
        let with_site = m(base(6945, Some("https://www.curseforge.com/minecraft/data-packs/tierify-prominence-tweaks")));
        assert_eq!(download_page(Some(&with_site), 1, 8794856), "https://www.curseforge.com/minecraft/data-packs/tierify-prominence-tweaks/download/8794856");
        let no_site = m(base(6945, None));
        assert_eq!(download_page(Some(&no_site), 1, 5), "https://www.curseforge.com/minecraft/data-packs/tierify-prominence-tweaks/download/5");
        assert_eq!(download_page(None, 42, 5), "https://www.curseforge.com/projects/42");
    }

    #[test]
    fn parses_loaders() {
        assert_eq!(parse_loader("forge-47.3.0", "1.20.1"), Some(("forge", "1.20.1-47.3.0".into())));
        assert_eq!(parse_loader("fabric-0.16.9", "1.21.4"), Some(("fabric", "0.16.9".into())));
        assert_eq!(parse_loader("neoforge-21.1.77", "1.21.1"), Some(("neoforge", "21.1.77".into())));
        assert_eq!(parse_loader("quilt-0.27.1", "1.21.1"), Some(("quilt", "0.27.1".into())));
        assert_eq!(parse_loader("liteloader-1", "1.12.2"), None);
    }

    #[test]
    fn parses_api_shapes() {
        let file: CfFile = serde_json::from_str(
            r#"{"id":5000123,"modId":238222,"displayName":"JEI 19.21.0","fileName":"jei-1.21.1-neoforge-19.21.0.jar",
               "releaseType":1,"fileDate":"2025-01-02T03:04:05Z","fileLength":1234,"downloadCount":99,
               "downloadUrl":"https://edge.forgecdn.net/files/5000/123/jei.jar",
               "gameVersions":["1.21.1","NeoForge","Client"],
               "hashes":[{"value":"ABCDEF","algo":1},{"value":"00","algo":2}],
               "dependencies":[{"modId":1,"relationType":3},{"modId":2,"relationType":2}]}"#,
        )
        .unwrap();
        assert!(file.works_with("1.21.1", &["neoforge"]));
        assert!(!file.works_with("1.21.1", &["fabric"]));
        assert!(!file.works_with("1.21.4", &["neoforge"]));
        let v = to_version(&file);
        assert_eq!(v.project_id, "cf:238222");
        assert_eq!(v.files[0].hashes["sha1"], "abcdef");
        assert_eq!(v.loaders, vec!["neoforge"]);
        assert_eq!(v.game_versions, vec!["1.21.1"]);
        assert_eq!(v.dependencies.len(), 1);

        let manifest: Manifest = serde_json::from_str(
            r#"{"minecraft":{"version":"1.20.1","modLoaders":[{"id":"forge-47.3.0","primary":true}]},
               "manifestType":"minecraftModpack","name":"Test Pack","version":"1.0",
               "files":[{"projectID":238222,"fileID":5000123,"required":true}],"overrides":"overrides"}"#,
        )
        .unwrap();
        assert_eq!(manifest.files[0].file_id, 5000123);
        assert_eq!(manifest.minecraft.mod_loaders[0].id, "forge-47.3.0");
    }

    /// Live check: with a real key (PEAK_CF_TEST_KEY) search Minecraft mods;
    /// without one, a fake key must be rejected cleanly.
    /// cargo test -- --ignored --nocapture curseforge_live
    #[tokio::test]
    #[ignore]
    async fn curseforge_live() {
        match std::env::var("PEAK_CF_TEST_KEY") {
            Ok(key) => {
                let cf = Client::new(&key).unwrap();
                let r = search(&cf, None, Some(Kind::Mod), "jei", "relevance", 0).await.unwrap();
                println!("hits: {} first: {}", r.total_hits, r.hits[0].title);
                let p = project(&cf, &r.hits[0].project_id).await.unwrap();
                println!("project: {} body {} chars", p.title, p.body.len());
            }
            Err(_) => {
                let cf = Client::new("not-a-real-key").unwrap();
                let err = search(&cf, None, Some(Kind::Mod), "jei", "relevance", 0).await.unwrap_err().to_string();
                println!("fake key -> {err}");
                assert!(err.contains("refused"), "unexpected error: {err}");
            }
        }
    }

    #[test]
    fn picks_key_then_proxy() {
        let own = Client::choose("mine", Some("built"), Some("https://api.example.com/v1/")).unwrap();
        assert_eq!((own.base.as_str(), own.key.as_deref()), (API, Some("mine")));
        let built = Client::choose("", Some("built"), Some("https://api.example.com/v1")).unwrap();
        assert_eq!(built.key.as_deref(), Some("built"));
        let proxied = Client::choose(" ", None, Some("https://api.example.com/v1/")).unwrap();
        assert_eq!((proxied.base.as_str(), proxied.key), ("https://api.example.com/v1", None));
        assert!(Client::choose("", None, None).is_err());
    }

    #[test]
    fn rejects_missing_key() {
        if BUILT_IN_KEY.is_none() {
            assert!(Client::new("").is_err());
        }
        assert!(Client::new("abc").is_ok());
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// A tiny local server that counts requests per path. /mods/500 always
    /// fails; everything else answers {"data": <count>}.
    async fn server() -> (String, Arc<Mutex<HashMap<String, usize>>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let hits: Arc<Mutex<HashMap<String, usize>>> = Arc::default();
        let counter = hits.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else { return };
                let counter = counter.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 8192];
                    let mut got = 0;
                    // Read the headers, then any body the request announced.
                    loop {
                        let n = sock.read(&mut buf[got..]).await.unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        got += n;
                        let text = String::from_utf8_lossy(&buf[..got]).to_string();
                        if let Some(end) = text.find("\r\n\r\n") {
                            let len = text
                                .lines()
                                .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                                .unwrap_or(0);
                            if got >= end + 4 + len {
                                break;
                            }
                        }
                    }
                    let text = String::from_utf8_lossy(&buf[..got]).to_string();
                    let path = text.split_whitespace().nth(1).unwrap_or("/").split('?').next().unwrap_or("/").to_string();
                    let n = {
                        let mut h = counter.lock().unwrap();
                        let e = h.entry(path.clone()).or_insert(0);
                        *e += 1;
                        *e
                    };
                    let (status, body) = if path.ends_with("/mods/500") { ("500 Internal Server Error", "{}".to_string()) } else { ("200 OK", format!("{{\"data\":{n}}}")) };
                    let resp = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                    let _ = sock.write_all(resp.as_bytes()).await;
                });
            }
        });
        (base, hits)
    }

    fn count(hits: &Arc<Mutex<HashMap<String, usize>>>, path: &str) -> usize {
        hits.lock().unwrap().get(path).copied().unwrap_or(0)
    }

    #[test]
    fn ttl_per_endpoint() {
        let m = |s: Option<Duration>| s.map(|d| d.as_secs() / 60);
        assert_eq!(m(cache_ttl("/mods/search")), Some(5));
        assert_eq!(m(cache_ttl("/mods/238222/files")), Some(5));
        assert_eq!(m(cache_ttl("/mods/238222")), Some(10));
        assert_eq!(m(cache_ttl("/mods/238222/description")), Some(60));
        assert_eq!(m(cache_ttl("/mods/238222/files/5101366")), Some(60));
        assert_eq!(cache_ttl("/mods"), None);
        assert_eq!(cache_ttl("/mods/files"), None);
        assert_eq!(cache_ttl("/fingerprints/432"), None);
    }

    // One test so the shared cache isn't cleared by another test mid-way.
    #[tokio::test]
    async fn client_uses_cache_only_where_it_should() {
        let (base, hits) = server().await;
        let cf = Client::choose("", None, Some(&base)).unwrap();

        // Same GET twice: one request. A different query is a different entry.
        let a: u32 = cf.get("/mods/1", &[]).await.unwrap();
        let b: u32 = cf.get("/mods/1", &[]).await.unwrap();
        assert_eq!((a, b), (1, 1));
        assert_eq!(count(&hits, "/mods/1"), 1);
        let _: u32 = cf.get("/mods/1/files", &[("gameVersion", "1.21.1".into())]).await.unwrap();
        let _: u32 = cf.get("/mods/1/files", &[("gameVersion", "1.20.1".into())]).await.unwrap();
        assert_eq!(count(&hits, "/mods/1/files"), 2);

        // POSTs always reach the server.
        let _: u32 = cf.post("/mods", json!({ "modIds": [1] })).await.unwrap();
        let _: u32 = cf.post("/mods", json!({ "modIds": [1] })).await.unwrap();
        assert_eq!(count(&hits, "/mods"), 2);

        // Errors aren't stored.
        assert!(cf.get::<u32>("/mods/500", &[]).await.is_err());
        assert!(cf.get::<u32>("/mods/500", &[]).await.is_err());
        assert_eq!(count(&hits, "/mods/500"), 2);

        // A fresh client skips the cache and stores the new answer.
        let f: u32 = cf.fresh().get("/mods/1", &[]).await.unwrap();
        assert_eq!(f, 2);
        let again: u32 = cf.get("/mods/1", &[]).await.unwrap();
        assert_eq!(again, 2);
        assert_eq!(count(&hits, "/mods/1"), 2);

        // Changing the key clears it.
        clear_cache();
        let _: u32 = cf.get("/mods/1", &[]).await.unwrap();
        assert_eq!(count(&hits, "/mods/1"), 3);
    }
}
