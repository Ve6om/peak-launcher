//! Discover servers: the public server list from peaklauncher.com, which
//! players can browse, filter and join in one click (modded servers install
//! their pack first). Voting happens on the server's page on the website,
//! where each vote is checked.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

/// The live list. Set PEAK_SERVER_LIST to test against the website running
/// on your own computer, e.g. http://127.0.0.1:8788/api/servers.
const LIVE_LIST: &str = "https://www.peaklauncher.com/api/servers";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListingPack {
    /// Modrinth project id, or "cf:<id>" / "technic:<slug>" like everywhere else in Peak.
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub id: String,
    pub name: String,
    pub address: String,
    pub tagline: String,
    pub description: String,
    pub tags: Vec<String>,
    pub region: String,
    pub language: String,
    /// The Minecraft version to join with.
    pub game_version: String,
    /// "vanilla", "fabric", "forge", "neoforge" or "quilt".
    pub loader: String,
    #[serde(default)]
    pub pack: Option<ListingPack>,
    /// Votes in the last 30 days.
    #[serde(default)]
    pub votes: u32,
    #[serde(default)]
    pub featured: bool,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub website: Option<String>,
    #[serde(default)]
    pub discord: Option<String>,
    /// The owner's banner (an approved one), as a full URL.
    #[serde(default)]
    pub banner_url: Option<String>,
    /// The server's page on the website (where players vote), as a full URL.
    #[serde(default)]
    pub page_url: Option<String>,
}

fn source() -> String {
    std::env::var("PEAK_SERVER_LIST")
        .ok()
        .filter(|u| u.starts_with("https://") || u.starts_with("http://127.0.0.1") || u.starts_with("http://localhost"))
        .unwrap_or_else(|| LIVE_LIST.to_string())
}

/// Make the list's links (banners, server pages) full URLs on the list's own site.
fn resolve(list: &mut [Listing], base: &str) {
    let Ok(base) = reqwest::Url::parse(base) else { return };
    for l in list {
        l.banner_url = l.banner_url.take().and_then(|b| base.join(&b).ok()).map(|u| u.to_string());
        let id_ok = l.id.len() == 36 && l.id.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
        l.page_url = if id_ok { base.join(&format!("/servers/{}/", l.id)).ok().map(|u| u.to_string()) } else { None };
    }
}

/// The server list.
pub async fn list() -> Result<Vec<Listing>> {
    let url = source();
    let resp = super::net::client()
        .get(&url)
        .send()
        .await
        .map_err(|_| Error::msg("Couldn't load the server list. Check your connection and try again."))?;
    if !resp.status().is_success() {
        return Err(Error::msg(format!("The server list answered with an error ({}). Try again later.", resp.status().as_u16())));
    }
    let mut list: Vec<Listing> = resp.json().await?;
    resolve(&mut list, &url);
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing(id: &str, banner: Option<&str>) -> Listing {
        serde_json::from_value(serde_json::json!({
            "id": id, "name": "A", "address": "a.example.com", "tagline": "", "description": "", "tags": [],
            "region": "Europe", "language": "English", "gameVersion": "1.21.4", "loader": "vanilla",
            "votes": 3, "bannerUrl": banner, "somethingNew": true
        }))
        .unwrap()
    }

    #[test]
    fn links_become_full_urls() {
        let id = "0f8fad5b-d9cb-469f-a165-70867728950e";
        let mut list = vec![listing(id, Some("/api/servers/x/banner?v=1")), listing("../evil", None)];
        resolve(&mut list, "https://www.peaklauncher.com/api/servers");
        assert_eq!(list[0].banner_url.as_deref(), Some("https://www.peaklauncher.com/api/servers/x/banner?v=1"));
        assert_eq!(list[0].page_url.as_deref(), Some(&*format!("https://www.peaklauncher.com/servers/{id}/")));
        assert_eq!(list[1].page_url, None, "odd ids don't make links");
        assert_eq!(list[1].banner_url, None);
    }

    #[test]
    fn defaults_to_the_live_list() {
        if std::env::var("PEAK_SERVER_LIST").is_err() {
            assert_eq!(source(), LIVE_LIST);
        }
    }
}
