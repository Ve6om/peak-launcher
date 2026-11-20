//! Account skins, straight from Mojang (no third-party skin sites, which
//! would learn every player's account id). Looked up by profile id on
//! Mojang's public session server, downloaded from textures.minecraft.net and
//! cached on disk, because Mojang limits how often one profile can be fetched.

use super::net;
use super::paths::Paths;
use crate::error::{Error, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How long a cached skin is used before checking Mojang again.
const FRESH_FOR_MINUTES: i64 = 60;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skin {
    /// The skin image as a data: URL (a few KB), ready for an <img> or canvas.
    pub png: String,
    /// "slim" (Alex-style arms) or "classic".
    pub model: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Cached {
    url: String,
    model: String,
    fetched_at: Option<DateTime<Utc>>,
    /// The cape the account wears, if any (saved next to the skin).
    #[serde(default)]
    cape_url: Option<String>,
}

/// Only plain hex ids (Minecraft profile UUIDs without dashes) are used in
/// file names and URLs.
fn clean_id(id: &str) -> Option<String> {
    let id = id.replace('-', "").to_lowercase();
    (id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit())).then_some(id)
}

/// Pull the skin URL and arm model out of Mojang's base64 "textures" property.
fn parse_textures(profile: &Value) -> Option<(String, String)> {
    let value = profile["properties"].as_array()?.iter().find(|p| p["name"] == "textures")?["value"].as_str()?;
    let decoded: Value = serde_json::from_slice(&B64.decode(value).ok()?).ok()?;
    let skin = &decoded["textures"]["SKIN"];
    let url = skin["url"].as_str()?.replacen("http://", "https://", 1);
    if !url.starts_with("https://textures.minecraft.net/") {
        return None;
    }
    let model = if skin["metadata"]["model"] == "slim" { "slim" } else { "classic" };
    Some((url, model.to_string()))
}

/// The skin for a Minecraft profile id, from the cache when it's recent.
/// None when the account has no custom skin (Minecraft shows a default one).
pub async fn get(paths: &Paths, profile_id: &str, refresh: bool) -> Result<Option<Skin>> {
    let id = clean_id(profile_id).ok_or_else(|| Error::msg("Not a Minecraft profile id"))?;
    let dir = paths.cache().join("skins");
    let png_path = dir.join(format!("{id}.png"));
    let meta_path = dir.join(format!("{id}.json"));
    let cached: Option<Cached> = super::paths::read_json(&meta_path).await.ok().flatten();

    let fresh = cached
        .as_ref()
        .and_then(|c| c.fetched_at)
        .is_some_and(|t| Utc::now() - t < Duration::minutes(FRESH_FOR_MINUTES));
    if !refresh && fresh && png_path.exists() {
        return read(&png_path, cached.unwrap_or_default().model).await.map(Some);
    }

    // Ask Mojang; if that fails (offline, rate limited) fall back to the cache.
    let profile: Value = match net::get_json(&format!("https://sessionserver.mojang.com/session/minecraft/profile/{id}")).await {
        Ok(p) => p,
        Err(e) => {
            return match (&cached, png_path.exists()) {
                (Some(c), true) => read(&png_path, c.model.clone()).await.map(Some),
                _ => Err(e),
            };
        }
    };
    // The cape comes in the same answer; keep it next to the skin.
    let cape_path = dir.join(format!("{id}.cape.png"));
    let cape = cape_url(&profile).map(|u| u.replacen("http://", "https://", 1)).filter(|u| u.starts_with("https://textures.minecraft.net/"));
    if cape.is_none() {
        let _ = tokio::fs::remove_file(&cape_path).await;
    } else if cached.as_ref().and_then(|c| c.cape_url.as_ref()) != cape.as_ref() || !cape_path.exists() {
        if let Some(png) = fetch_png(cape.as_deref().unwrap_or_default()).await {
            tokio::fs::create_dir_all(&dir).await?;
            let bytes = B64.decode(png.trim_start_matches("data:image/png;base64,")).unwrap_or_default();
            tokio::fs::write(&cape_path, bytes).await?;
        }
    }
    let Some((url, model)) = parse_textures(&profile) else {
        return Ok(None);
    };
    let same_image = cached.as_ref().is_some_and(|c| c.url == url) && png_path.exists();
    if !same_image {
        let bytes = net::client().get(&url).send().await?.error_for_status()?.bytes().await?;
        if !bytes.starts_with(b"\x89PNG") {
            return Err(Error::msg("Mojang sent a skin that isn't a PNG"));
        }
        tokio::fs::create_dir_all(&dir).await?;
        tokio::fs::write(&png_path, &bytes).await?;
    }
    super::paths::write_json(&meta_path, &Cached { url, model: model.clone(), fetched_at: Some(Utc::now()), cape_url: cape }).await?;
    read(&png_path, model).await.map(Some)
}

/// The cape the account wears (saved when its skin was looked up), as a
/// data: URL, or None.
pub async fn cape(paths: &Paths, profile_id: &str) -> Result<Option<String>> {
    let Some(id) = clean_id(profile_id) else { return Ok(None) };
    match tokio::fs::read(paths.cache().join("skins").join(format!("{id}.cape.png"))).await {
        Ok(bytes) if bytes.starts_with(b"\x89PNG") => Ok(Some(format!("data:image/png;base64,{}", B64.encode(bytes)))),
        _ => Ok(None),
    }
}

async fn read(path: &std::path::Path, model: String) -> Result<Skin> {
    let bytes = tokio::fs::read(path).await?;
    Ok(Skin { png: format!("data:image/png;base64,{}", B64.encode(bytes)), model })
}

// ---------------------------------------------------------------- changing skins and capes
//
// Minecraft's own profile API (the one minecraft.net and the official
// launcher use), with the account's Minecraft token.

const PROFILE_API: &str = "https://api.minecraftservices.com/minecraft/profile";

#[derive(Debug, Clone, Deserialize)]
struct ApiTexture {
    id: String,
    state: String,
    url: String,
    #[serde(default)]
    variant: Option<String>,
    #[serde(default)]
    alias: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiProfile {
    #[serde(default)]
    skins: Vec<ApiTexture>,
    #[serde(default)]
    capes: Vec<ApiTexture>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cape {
    pub id: String,
    /// The cape's name, e.g. "Migrator".
    pub name: String,
    pub active: bool,
    /// The cape image as a data: URL, for previews.
    pub png: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Wardrobe {
    /// The skin in use, or None for Minecraft's default.
    pub skin: Option<Skin>,
    pub capes: Vec<Cape>,
}

/// Turn the API's answer into an error the player can act on.
async fn checked(resp: reqwest::Response) -> Result<reqwest::Response> {
    match resp.status().as_u16() {
        200..=299 => Ok(resp),
        401 => Err(Error::msg("Your Minecraft sign-in has expired. Sign in to this account again.")),
        429 => Err(Error::msg("Minecraft only allows a few skin changes in a short time. Wait a minute, then try again.")),
        400 => {
            let body: Value = resp.json().await.unwrap_or_default();
            let why = body["errorMessage"].as_str().or_else(|| body["error"].as_str()).unwrap_or("it isn't a valid skin");
            Err(Error::msg(format!("Minecraft didn't accept that: {why}")))
        }
        code => Err(Error::msg(format!("Minecraft's profile service returned an error ({code}). Try again later."))),
    }
}

async fn fetch_png(url: &str) -> Option<String> {
    let url = url.replacen("http://", "https://", 1);
    if !url.starts_with("https://textures.minecraft.net/") {
        return None;
    }
    let bytes = net::client().get(&url).send().await.ok()?.error_for_status().ok()?.bytes().await.ok()?;
    bytes.starts_with(b"\x89PNG").then(|| format!("data:image/png;base64,{}", B64.encode(&bytes)))
}

/// Make the cached skin match the profile right away (Mojang's public profile
/// lookup can lag behind a change by a minute or so).
async fn remember(paths: &Paths, profile_id: &str, profile: &ApiProfile) -> Result<Option<Skin>> {
    let Some(id) = clean_id(profile_id) else { return Ok(None) };
    let dir = paths.cache().join("skins");
    let Some(active) = profile.skins.iter().find(|s| s.state == "ACTIVE") else {
        let _ = tokio::fs::remove_file(dir.join(format!("{id}.png"))).await;
        let _ = tokio::fs::remove_file(dir.join(format!("{id}.json"))).await;
        return Ok(None);
    };
    let model = if active.variant.as_deref().is_some_and(|v| v.eq_ignore_ascii_case("slim")) { "slim" } else { "classic" };
    let Some(png) = fetch_png(&active.url).await else { return Ok(None) };
    let bytes = B64.decode(png.trim_start_matches("data:image/png;base64,")).unwrap_or_default();
    tokio::fs::create_dir_all(&dir).await?;
    tokio::fs::write(dir.join(format!("{id}.png")), &bytes).await?;
    let _ = record(paths, &id, &bytes, model).await;
    super::paths::write_json(
        &dir.join(format!("{id}.json")),
        &Cached { url: active.url.replacen("http://", "https://", 1), model: model.into(), fetched_at: Some(Utc::now()), cape_url: None },
    )
    .await?;
    Ok(Some(Skin { png, model: model.into() }))
}

async fn to_wardrobe(paths: &Paths, profile_id: &str, profile: ApiProfile) -> Result<Wardrobe> {
    let skin = remember(paths, profile_id, &profile).await?;
    if let Some(id) = clean_id(profile_id) {
        let cape_path = paths.cache().join("skins").join(format!("{id}.cape.png"));
        match profile.capes.iter().find(|c| c.state == "ACTIVE").map(|c| c.url.clone()) {
            Some(url) => {
                if let Some(png) = fetch_png(&url).await {
                    let bytes = B64.decode(png.trim_start_matches("data:image/png;base64,")).unwrap_or_default();
                    let _ = tokio::fs::write(&cape_path, bytes).await;
                }
            }
            None => {
                let _ = tokio::fs::remove_file(&cape_path).await;
            }
        }
    }
    let mut capes = Vec::new();
    for c in profile.capes {
        capes.push(Cape {
            name: c.alias.clone().unwrap_or_else(|| "Cape".into()),
            active: c.state == "ACTIVE",
            png: fetch_png(&c.url).await,
            id: c.id,
        });
    }
    Ok(Wardrobe { skin, capes })
}

/// The account's skin and the capes it owns.
pub async fn wardrobe(paths: &Paths, profile_id: &str, token: &str) -> Result<Wardrobe> {
    let resp = net::client().get(PROFILE_API).bearer_auth(token).send().await?;
    let profile: ApiProfile = checked(resp).await?.json().await?;
    to_wardrobe(paths, profile_id, profile).await
}

/// Check a skin file: a PNG of 64x64 (or the old 64x32) pixels.
pub fn validate_skin(bytes: &[u8]) -> Result<()> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") || bytes.len() < 24 || &bytes[12..16] != b"IHDR" {
        return Err(Error::msg("That isn't a PNG image. Skins are PNG files."));
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let h = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    if w != 64 || (h != 64 && h != 32) {
        return Err(Error::msg(format!("Skins must be 64x64 pixels (or the old 64x32). This image is {w}x{h}.")));
    }
    if bytes.len() > 512 * 1024 {
        return Err(Error::msg("That file is too large to be a skin."));
    }
    Ok(())
}

/// Read and check a skin file the player picked, for previewing it.
pub async fn read_skin_file(path: &std::path::Path) -> Result<String> {
    let bytes = tokio::fs::read(path).await?;
    validate_skin(&bytes)?;
    Ok(format!("data:image/png;base64,{}", B64.encode(bytes)))
}

fn variant_name(model: &str) -> &'static str {
    if model == "slim" { "slim" } else { "classic" }
}

/// Upload a new skin.
pub async fn upload(paths: &Paths, profile_id: &str, token: &str, png: Vec<u8>, model: &str) -> Result<Wardrobe> {
    validate_skin(&png)?;
    let part = reqwest::multipart::Part::bytes(png).file_name("skin.png").mime_str("image/png")?;
    let form = reqwest::multipart::Form::new().text("variant", variant_name(model)).part("file", part);
    let resp = net::client().post(format!("{PROFILE_API}/skins")).bearer_auth(token).multipart(form).send().await?;
    let profile: ApiProfile = checked(resp).await?.json().await?;
    to_wardrobe(paths, profile_id, profile).await
}

/// Switch the current skin between classic and slim arms.
pub async fn set_model(paths: &Paths, profile_id: &str, token: &str, model: &str) -> Result<Wardrobe> {
    let current = wardrobe_raw(token).await?;
    let active = current.skins.iter().find(|s| s.state == "ACTIVE").ok_or_else(|| Error::msg("This account uses the default skin; upload a skin first."))?;
    let body = serde_json::json!({ "variant": variant_name(model), "url": active.url.replacen("http://", "https://", 1) });
    let resp = net::client().post(format!("{PROFILE_API}/skins")).bearer_auth(token).json(&body).send().await?;
    let profile: ApiProfile = checked(resp).await?.json().await?;
    to_wardrobe(paths, profile_id, profile).await
}

async fn wardrobe_raw(token: &str) -> Result<ApiProfile> {
    let resp = net::client().get(PROFILE_API).bearer_auth(token).send().await?;
    Ok(checked(resp).await?.json().await?)
}

/// Go back to Minecraft's default skin.
pub async fn reset(paths: &Paths, profile_id: &str, token: &str) -> Result<Wardrobe> {
    let resp = net::client().delete(format!("{PROFILE_API}/skins/active")).bearer_auth(token).send().await?;
    let profile: ApiProfile = checked(resp).await?.json().await?;
    to_wardrobe(paths, profile_id, profile).await
}

/// Show one of the account's capes, or none.
pub async fn set_cape(paths: &Paths, profile_id: &str, token: &str, cape_id: Option<&str>) -> Result<Wardrobe> {
    let resp = match cape_id {
        Some(id) => net::client().put(format!("{PROFILE_API}/capes/active")).bearer_auth(token).json(&serde_json::json!({ "capeId": id })).send().await?,
        None => net::client().delete(format!("{PROFILE_API}/capes/active")).bearer_auth(token).send().await?,
    };
    let profile: ApiProfile = checked(resp).await?.json().await?;
    to_wardrobe(paths, profile_id, profile).await
}

// ---------------------------------------------------------------- skin history
//
// Mojang keeps no history of past skins, so Peak keeps its own: a copy of
// every skin an account has used while signed in here, newest first.

/// How many past skins are kept per account.
const HISTORY_LIMIT: usize = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryEntry {
    /// SHA-256 of the image, also its file name.
    id: String,
    model: String,
    used_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PastSkin {
    pub id: String,
    pub png: String,
    pub model: String,
    pub used_at: DateTime<Utc>,
}

fn history_dir(paths: &Paths, profile_id: &str) -> Option<std::path::PathBuf> {
    clean_id(profile_id).map(|id| paths.root.join("skin-history").join(id))
}

fn is_history_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Put a skin at the top of the account's history.
async fn record(paths: &Paths, profile_id: &str, png: &[u8], model: &str) -> Result<()> {
    use sha2::{Digest, Sha256};
    let Some(dir) = history_dir(paths, profile_id) else { return Ok(()) };
    let id = hex::encode(Sha256::digest(png));
    let index_path = dir.join("history.json");
    let mut entries: Vec<HistoryEntry> = super::paths::read_json(&index_path).await.ok().flatten().unwrap_or_default();
    if entries.first().is_some_and(|e| e.id == id && e.model == model) {
        return Ok(()); // already the newest; don't rewrite for every refresh
    }
    entries.retain(|e| e.id != id);
    entries.insert(0, HistoryEntry { id: id.clone(), model: model.into(), used_at: Utc::now() });
    tokio::fs::create_dir_all(&dir).await?;
    let file = dir.join(format!("{id}.png"));
    if !file.exists() {
        tokio::fs::write(&file, png).await?;
    }
    let keep = HISTORY_LIMIT.min(entries.len());
    for old in entries.drain(keep..) {
        let _ = tokio::fs::remove_file(dir.join(format!("{}.png", old.id))).await;
    }
    super::paths::write_json(&index_path, &entries).await
}

/// The account's past skins, newest first (the one in use is first).
pub async fn history(paths: &Paths, profile_id: &str) -> Result<Vec<PastSkin>> {
    let Some(dir) = history_dir(paths, profile_id) else { return Ok(Vec::new()) };
    let entries: Vec<HistoryEntry> = super::paths::read_json(&dir.join("history.json")).await.ok().flatten().unwrap_or_default();
    let mut out = Vec::new();
    for e in entries {
        if !is_history_id(&e.id) {
            continue;
        }
        if let Ok(bytes) = tokio::fs::read(dir.join(format!("{}.png", e.id))).await {
            out.push(PastSkin { png: format!("data:image/png;base64,{}", B64.encode(bytes)), id: e.id, model: e.model, used_at: e.used_at });
        }
    }
    Ok(out)
}

/// The image of a saved skin, for wearing it again.
pub async fn history_png(paths: &Paths, profile_id: &str, skin_id: &str) -> Result<Vec<u8>> {
    let dir = history_dir(paths, profile_id).filter(|_| is_history_id(skin_id)).ok_or_else(|| Error::msg("That saved skin wasn't found."))?;
    tokio::fs::read(dir.join(format!("{skin_id}.png"))).await.map_err(|_| Error::msg("That saved skin wasn't found."))
}

/// Forget one saved skin.
pub async fn history_remove(paths: &Paths, profile_id: &str, skin_id: &str) -> Result<()> {
    let Some(dir) = history_dir(paths, profile_id) else { return Ok(()) };
    if !is_history_id(skin_id) {
        return Ok(());
    }
    let index_path = dir.join("history.json");
    let mut entries: Vec<HistoryEntry> = super::paths::read_json(&index_path).await.ok().flatten().unwrap_or_default();
    entries.retain(|e| e.id != skin_id);
    let _ = tokio::fs::remove_file(dir.join(format!("{skin_id}.png"))).await;
    super::paths::write_json(&index_path, &entries).await
}

// ---------------------------------------------------------------- player lookup

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    /// The profile id, with dashes.
    pub id: String,
    /// The name with Mojang's capitalisation.
    pub name: String,
    pub skin: Option<Skin>,
    pub cape: Option<String>,
    /// Names the player has used, newest first (from Crafty.gg).
    pub names: Vec<PastName>,
    /// Skins the player has worn, newest first (from Crafty.gg).
    pub past_skins: Vec<Skin>,
    /// Capes the player has, the one they wear first (from Crafty.gg).
    pub capes: Vec<PastCape>,
    /// False when Crafty.gg couldn't be reached, so the history is unknown.
    pub history_found: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PastName {
    pub name: String,
    /// When Crafty.gg recorded the change (can be later than the real one);
    /// None for the first name.
    pub changed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PastCape {
    /// The cape's name, e.g. "Migrator".
    pub name: String,
    /// The cape image as a data: URL.
    pub png: String,
    /// The one the player wears now.
    pub current: bool,
}

/// What Crafty.gg knows about a player's past.
#[derive(Debug, Default)]
struct History {
    names: Vec<PastName>,
    skins: Vec<Skin>,
    capes: Vec<PastCape>,
}

// ---------------------------------------------------------------- Crafty.gg history
//
// Mojang no longer shares past names or skins. Crafty.gg keeps a record and
// has a public API (no key needed for light use), so the lookup asks it.

const CRAFTY_API: &str = "https://api.crafty.gg/api/v2/players";
const PAST_SKINS_LIMIT: usize = 40;

#[derive(Debug, Default, Deserialize)]
struct CraftyName {
    username: String,
    #[serde(default)]
    changed_at: Option<String>,
    #[serde(default)]
    hidden: bool,
}

#[derive(Debug, Default, Deserialize)]
struct CraftySkin {
    #[serde(default)]
    texture: Option<String>,
    #[serde(default)]
    changed_at: Option<String>,
    #[serde(default)]
    slim: bool,
    #[serde(default)]
    hidden: bool,
}

#[derive(Debug, Default, Deserialize)]
struct CraftyCape {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    texture: Option<String>,
    #[serde(default)]
    current: bool,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    removed: bool,
}

#[derive(Debug, Default, Deserialize)]
struct CraftyPlayer {
    #[serde(default)]
    usernames: Vec<CraftyName>,
    #[serde(default)]
    skins: Vec<CraftySkin>,
    #[serde(default)]
    capes: Vec<CraftyCape>,
}

/// A cape image: a PNG of a sensible size (capes are 64x32, some HD ones larger).
fn is_cape_png(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.len() >= 24 && &bytes[12..16] == b"IHDR" && bytes.len() <= 256 * 1024
}

/// Turn Crafty's answer into names (newest first), skins and capes, leaving
/// out anything the player chose to hide and anything that isn't an image.
fn read_crafty(player: CraftyPlayer) -> History {
    let mut names: Vec<CraftyName> = player.usernames.into_iter().filter(|n| !n.hidden && valid_name(&n.username)).collect();
    // ISO dates sort as text; the original name (oldest) goes last.
    names.sort_by(|a, b| b.changed_at.cmp(&a.changed_at));
    let count = names.len();
    let names = names
        .into_iter()
        .enumerate()
        .map(|(i, n)| PastName { name: n.username, changed_at: if i + 1 == count { None } else { n.changed_at } })
        .collect();
    let mut skins = player.skins;
    skins.sort_by(|a, b| b.changed_at.cmp(&a.changed_at));
    let skins = skins
        .into_iter()
        .filter(|s| !s.hidden)
        .filter_map(|s| {
            let bytes = B64.decode(s.texture?.trim()).ok()?;
            validate_skin(&bytes).ok()?;
            Some(Skin { png: format!("data:image/png;base64,{}", B64.encode(&bytes)), model: if s.slim { "slim" } else { "classic" }.into() })
        })
        .take(PAST_SKINS_LIMIT)
        .collect();
    let mut capes: Vec<PastCape> = player
        .capes
        .into_iter()
        .filter(|c| !c.hidden && !c.removed)
        .filter_map(|c| {
            let bytes = B64.decode(c.texture?.trim()).ok()?;
            is_cape_png(&bytes).then(|| PastCape {
                name: c.name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "Cape".into()),
                png: format!("data:image/png;base64,{}", B64.encode(&bytes)),
                current: c.current,
            })
        })
        .collect();
    // The one being worn first.
    capes.sort_by_key(|c| !c.current);
    History { names, skins, capes }
}

/// The player's past names and skins from Crafty.gg, or None if it couldn't
/// be reached or doesn't know the player.
async fn crafty_history(id: &str) -> Option<History> {
    #[derive(Deserialize)]
    struct Answer {
        data: CraftyPlayer,
    }
    let resp = net::client().get(format!("{CRAFTY_API}/{id}")).header("Accept", "application/json").send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let answer: Answer = resp.json().await.ok()?;
    Some(read_crafty(answer.data))
}

/// A player's profile id by name, from Crafty.gg.
async fn crafty_id(name: &str) -> Option<String> {
    let resp = net::client().get(format!("{CRAFTY_API}/{name}")).header("Accept", "application/json").send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let answer: Value = resp.json().await.ok()?;
    answer["data"]["uuid"].as_str().and_then(clean_id)
}

/// Minecraft names: up to 16 letters, digits and underscores (a few very old
/// accounts have names shorter than today's minimum of 3).
fn valid_name(name: &str) -> bool {
    (1..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn dashed(id: &str) -> String {
    format!("{}-{}-{}-{}-{}", &id[0..8], &id[8..12], &id[12..16], &id[16..20], &id[20..32])
}

fn cape_url(profile: &Value) -> Option<String> {
    let value = profile["properties"].as_array()?.iter().find(|p| p["name"] == "textures")?["value"].as_str()?;
    let decoded: Value = serde_json::from_slice(&B64.decode(value).ok()?).ok()?;
    decoded["textures"]["CAPE"]["url"].as_str().map(str::to_string)
}

fn lookup_error(code: u16, missing: String) -> Error {
    match code {
        204 | 400 | 404 => Error::msg(missing),
        429 => Error::msg("Mojang is limiting lookups right now. Try again in a minute."),
        code => Error::msg(format!("Mojang's lookup returned an error ({code}). Try again later.")),
    }
}

/// Look up any player by name (or profile id): their id, skin and cape.
pub async fn lookup(query: &str) -> Result<Player> {
    let query = query.trim();
    let id = match clean_id(query) {
        Some(id) => id,
        None => {
            if !valid_name(query) {
                return Err(Error::msg("Minecraft names are up to 16 letters, numbers and underscores."));
            }
            let resp = net::client().get(format!("{PROFILE_API}/lookup/name/{query}")).send().await?;
            let code = resp.status().as_u16();
            if code == 200 {
                let found: Value = resp.json().await?;
                found["id"].as_str().and_then(clean_id).ok_or_else(|| Error::msg("Mojang sent an unexpected answer."))?
            } else if let (false, Some(id)) = (matches!(code, 204 | 400 | 404), crafty_id(query).await) {
                // Mojang is busy or limiting lookups; Crafty.gg knows most names too.
                id
            } else {
                return Err(lookup_error(code, format!("No Minecraft player is called \"{query}\".")));
            }
        }
    };
    if let Some(p) = recent(&id) {
        return Ok(p);
    }
    let resp = net::client().get(format!("https://sessionserver.mojang.com/session/minecraft/profile/{id}")).send().await?;
    let code = resp.status().as_u16();
    let player = if code == 200 {
        let profile: Value = resp.json().await?;
        let name = profile["name"].as_str().unwrap_or(query).to_string();
        let textures = async {
            let skin = match parse_textures(&profile) {
                Some((url, model)) => fetch_png(&url).await.map(|png| Skin { png, model }),
                None => None,
            };
            let cape = match cape_url(&profile) {
                Some(url) => fetch_png(&url).await,
                None => None,
            };
            (skin, cape)
        };
        let ((skin, cape), history) = tokio::join!(textures, crafty_history(&id));
        let history_found = history.is_some();
        let History { mut names, skins: past_skins, capes } = history.unwrap_or_default();
        if names.is_empty() {
            names.push(PastName { name: name.clone(), changed_at: None });
        }
        Player { id: dashed(&id), name, skin, cape, names, past_skins, capes, history_found }
    } else if matches!(code, 204 | 400 | 404) {
        return Err(lookup_error(code, "No Minecraft player has that id.".into()));
    } else {
        // Mojang only allows looking up the same player about once a minute.
        // Crafty.gg's copy of the current skin is the next best thing.
        match crafty_history(&id).await {
            Some(History { names, skins: past_skins, capes }) if !names.is_empty() => Player {
                id: dashed(&id),
                name: names[0].name.clone(),
                skin: past_skins.first().cloned(),
                cape: capes.iter().find(|c| c.current).map(|c| c.png.clone()),
                names,
                past_skins,
                capes,
                history_found: true,
            },
            _ => return Err(lookup_error(code, "No Minecraft player has that id.".into())),
        }
    };
    remember_lookup(&id, &player);
    Ok(player)
}

/// Recent lookups, so looking the same player up again doesn't hit Mojang's
/// once-a-minute limit.
static RECENT: std::sync::Mutex<Vec<(String, std::time::Instant, Player)>> = std::sync::Mutex::new(Vec::new());
const RECENT_FOR: std::time::Duration = std::time::Duration::from_secs(5 * 60);

fn recent(id: &str) -> Option<Player> {
    let mut list = RECENT.lock().ok()?;
    list.retain(|(_, at, _)| at.elapsed() < RECENT_FOR);
    list.iter().find(|(i, _, _)| i == id).map(|(_, _, p)| p.clone())
}

fn remember_lookup(id: &str, player: &Player) {
    if let Ok(mut list) = RECENT.lock() {
        list.retain(|(i, at, _)| i != id && at.elapsed() < RECENT_FOR);
        list.push((id.to_string(), std::time::Instant::now(), player.clone()));
        if list.len() > 20 {
            list.remove(0);
        }
    }
}

#[cfg(test)]
mod history_tests {
    use super::*;

    #[tokio::test]
    async fn keeps_newest_first_without_duplicates() {
        let root = std::env::temp_dir().join(format!("peak-skin-history-{}", std::process::id()));
        let paths = Paths::new(root.clone());
        let pid = "069a79f444e94726a5befca90e38aaf5";
        record(&paths, pid, b"\x89PNG one", "classic").await.unwrap();
        record(&paths, pid, b"\x89PNG two", "slim").await.unwrap();
        record(&paths, pid, b"\x89PNG one", "classic").await.unwrap();
        let h = history(&paths, pid).await.unwrap();
        assert_eq!(h.len(), 2);
        assert_eq!(B64.decode(h[0].png.trim_start_matches("data:image/png;base64,")).unwrap(), b"\x89PNG one");
        assert_eq!(h[1].model, "slim");
        assert_eq!(history_png(&paths, pid, &h[1].id).await.unwrap(), b"\x89PNG two");
        assert!(history_png(&paths, pid, "../../accounts").await.is_err());
        history_remove(&paths, pid, &h[1].id).await.unwrap();
        assert_eq!(history(&paths, pid).await.unwrap().len(), 1);
        assert!(history(&paths, "../evil").await.unwrap().is_empty());
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn trims_to_the_limit() {
        let root = std::env::temp_dir().join(format!("peak-skin-limit-{}", std::process::id()));
        let paths = Paths::new(root.clone());
        let pid = "069a79f444e94726a5befca90e38aaf5";
        for i in 0..HISTORY_LIMIT + 3 {
            record(&paths, pid, format!("png {i}").as_bytes(), "classic").await.unwrap();
        }
        assert_eq!(history(&paths, pid).await.unwrap().len(), HISTORY_LIMIT);
        let files = std::fs::read_dir(history_dir(&paths, pid).unwrap()).unwrap().count();
        assert_eq!(files, HISTORY_LIMIT + 1, "old images are deleted too");
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn names_and_ids() {
        assert!(valid_name("jeb_"));
        assert!(valid_name("Notch"));
        assert!(!valid_name("has space"));
        assert!(!valid_name("../x"));
        assert!(!valid_name("seventeen_chars_x"));
        assert_eq!(dashed("069a79f444e94726a5befca90e38aaf5"), "069a79f4-44e9-4726-a5be-fca90e38aaf5");
    }

    #[test]
    fn reads_crafty_history() {
        let png = {
            let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
            b.extend(64u32.to_be_bytes());
            b.extend(64u32.to_be_bytes());
            B64.encode(b)
        };
        let p: CraftyPlayer = serde_json::from_value(serde_json::json!({
            "usernames": [
                { "username": "Dream", "changed_at": "2023-08-06T23:31:18.381+02:00", "hidden": false },
                { "username": "Secret", "changed_at": "2016-01-01T00:00:00.000+01:00", "hidden": true },
                { "username": "DeltaNinja", "changed_at": "2012-01-10T01:00:00.000+01:00", "hidden": false }
            ],
            "skins": [
                { "texture": png, "slim": true, "hidden": false },
                { "texture": "bm90IGEgcG5n", "slim": false },
                { "texture": png, "slim": false, "hidden": true }
            ],
            "capes": [
                { "name": "Mojang", "texture": png, "current": false },
                { "name": "Mojang Studios", "texture": png, "current": true },
                { "name": "Hidden one", "texture": png, "hidden": true },
                { "name": "Broken", "texture": "bm90IGEgcG5n" }
            ],
            "something_else": 1
        }))
        .unwrap();
        let History { names, skins, capes } = read_crafty(p);
        let cape_names: Vec<_> = capes.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(cape_names, ["Mojang Studios", "Mojang"], "worn cape first, hidden and broken left out");
        assert!(capes[0].current);
        let list: Vec<_> = names.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(list, ["Dream", "DeltaNinja"], "newest first, hidden left out");
        assert!(names[0].changed_at.is_some());
        assert_eq!(names[1].changed_at, None, "the first name has no change date");
        assert_eq!(skins.len(), 1, "only real, visible skins");
        assert_eq!(skins[0].model, "slim");
    }

    /// cargo test --lib -- --ignored --nocapture lookup_live
    #[tokio::test]
    #[ignore]
    async fn lookup_live() {
        let p = lookup("jeb_").await.unwrap();
        println!("{} {} skin={} cape={} names={:?} past_skins={}", p.name, p.id, p.skin.is_some(), p.cape.is_some(), p.names.iter().map(|n| &n.name).collect::<Vec<_>>(), p.past_skins.len());
        assert_eq!(p.id, "853c80ef-3c37-49fd-aa49-938b674adae6");
        assert!(p.cape.is_some());
        assert!(lookup("this name has spaces").await.is_err());
        let dream = lookup("Dream").await.unwrap();
        println!("names={:?} past_skins={} history_found={}", dream.names.iter().map(|n| (&n.name, &n.changed_at)).collect::<Vec<_>>(), dream.past_skins.len(), dream.history_found);
        assert!(dream.names.iter().any(|n| n.name == "DeltaNinja"), "Crafty knows Dream's first name");
        let jeb = lookup("jeb_").await.unwrap();
        println!("jeb capes={:?}", jeb.capes.iter().map(|c| (&c.name, c.current)).collect::<Vec<_>>());
        assert!(jeb.capes.len() >= 2);
    }
}

#[cfg(test)]
mod edit_tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        b.extend(w.to_be_bytes());
        b.extend(h.to_be_bytes());
        b.extend([8, 6, 0, 0, 0]);
        b
    }

    #[test]
    fn accepts_only_skin_sized_pngs() {
        assert!(validate_skin(&png(64, 64)).is_ok());
        assert!(validate_skin(&png(64, 32)).is_ok(), "old-style skins are fine");
        assert!(validate_skin(&png(128, 128)).unwrap_err().to_string().contains("128x128"));
        assert!(validate_skin(&png(64, 48)).is_err());
        assert!(validate_skin(b"GIF89a not a png").is_err());
        assert!(validate_skin(b"").is_err());
    }

    #[test]
    fn reads_the_profile_api_shape() {
        let p: ApiProfile = serde_json::from_value(serde_json::json!({
            "id": "x", "name": "Steve",
            "skins": [{ "id": "s1", "state": "ACTIVE", "url": "http://textures.minecraft.net/texture/abc", "variant": "SLIM" }],
            "capes": [{ "id": "c1", "state": "INACTIVE", "url": "http://textures.minecraft.net/texture/def", "alias": "Migrator" }]
        }))
        .unwrap();
        assert_eq!(p.skins[0].variant.as_deref(), Some("SLIM"));
        assert_eq!(p.capes[0].alias.as_deref(), Some("Migrator"));
        assert_eq!(variant_name("slim"), "slim");
        assert_eq!(variant_name("anything"), "classic");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn profile(textures: Value) -> Value {
        let encoded = B64.encode(serde_json::to_vec(&json!({ "textures": textures })).unwrap());
        json!({ "id": "x", "name": "x", "properties": [{ "name": "textures", "value": encoded }] })
    }

    #[test]
    fn reads_skin_url_and_model() {
        let slim = profile(json!({ "SKIN": { "url": "http://textures.minecraft.net/texture/abc", "metadata": { "model": "slim" } } }));
        assert_eq!(parse_textures(&slim), Some(("https://textures.minecraft.net/texture/abc".into(), "slim".into())));
        let classic = profile(json!({ "SKIN": { "url": "https://textures.minecraft.net/texture/def" } }));
        assert_eq!(parse_textures(&classic).unwrap().1, "classic");
    }

    #[test]
    fn ignores_missing_or_foreign_skins() {
        assert_eq!(parse_textures(&profile(json!({}))), None, "no custom skin");
        let elsewhere = profile(json!({ "SKIN": { "url": "https://evil.example/skin.png" } }));
        assert_eq!(parse_textures(&elsewhere), None, "only Mojang's texture server");
        assert_eq!(parse_textures(&json!({ "properties": [] })), None);
    }

    #[test]
    fn only_profile_ids_are_accepted() {
        assert_eq!(clean_id("069a79f4-44e9-4726-a5be-fca90e38aaf5").as_deref(), Some("069a79f444e94726a5befca90e38aaf5"));
        assert_eq!(clean_id("../../etc/passwd"), None);
        assert_eq!(clean_id("not-a-uuid"), None);
    }

    /// Fetch a real skin (Notch's) into a temporary folder:
    /// cargo test --lib -- --ignored --nocapture skins_live
    #[tokio::test]
    #[ignore]
    async fn skins_live() {
        let root = std::env::temp_dir().join(format!("peak-skins-{}", std::process::id()));
        let paths = Paths::new(root.clone());
        let skin = get(&paths, "069a79f444e94726a5befca90e38aaf5", false).await.unwrap().expect("Notch has a skin");
        println!("model {}, data URL {} chars", skin.model, skin.png.len());
        assert!(skin.png.starts_with("data:image/png;base64,iVBOR"));
        // Second call comes from the cache.
        let again = get(&paths, "069a79f444e94726a5befca90e38aaf5", false).await.unwrap().unwrap();
        assert_eq!(again.png, skin.png);
        std::fs::remove_dir_all(root).ok();
    }
}
