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
    super::paths::write_json(&meta_path, &Cached { url, model: model.clone(), fetched_at: Some(Utc::now()) }).await?;
    read(&png_path, model).await.map(Some)
}

async fn read(path: &std::path::Path, model: String) -> Result<Skin> {
    let bytes = tokio::fs::read(path).await?;
    Ok(Skin { png: format!("data:image/png;base64,{}", B64.encode(bytes)), model })
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
