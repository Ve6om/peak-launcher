//! peak:// links, so the website can open Peak on a server: the server
//! list's "Play in Peak" button, and invite links a player shares with a
//! friend. Everything in a link is checked, and a link never starts the game
//! by itself (the player still presses Play).
//!
//!   peak://server/<listing id>
//!   peak://join?s=<address>&n=<server name>&mc=<minecraft version>
//!                &p=<modpack id>&pn=<modpack name>&v=<version id>&vn=<version name>

use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

/// A link that arrived before the window was ready to hear about it.
#[derive(Default)]
pub struct PendingLink(pub Mutex<Option<Link>>);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Link {
    /// A server-list entry.
    Server { id: String },
    /// An invite to a server, with the modpack it runs (if any).
    #[serde(rename_all = "camelCase")]
    Join {
        address: String,
        name: String,
        game_version: Option<String>,
        pack: Option<String>,
        pack_name: Option<String>,
        version: Option<String>,
        version_name: Option<String>,
    },
}

/// The listing id in a peak:// link, if it's one Peak understands.
pub fn listing_id(url: &str) -> Option<String> {
    let rest = url.trim().strip_prefix("peak://")?;
    let rest = rest.trim_end_matches('/');
    let id = rest.strip_prefix("server/")?;
    let ok = id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    ok.then(|| id.to_ascii_lowercase())
}

/// Plain text of at most `max` characters, without control characters.
fn text(v: &str, max: usize) -> Option<String> {
    let v = v.trim();
    (!v.is_empty() && v.chars().count() <= max && !v.chars().any(char::is_control)).then(|| v.to_string())
}

/// A modpack or version id: Modrinth ids, "cf:123", "ftb:12", "technic:slug".
fn id(v: &str) -> Option<String> {
    let v = v.trim();
    (!v.is_empty() && v.len() <= 64 && v.chars().all(|c| c.is_ascii_alphanumeric() || "-_.:".contains(c))).then(|| v.to_string())
}

/// An invite link's contents, if it's a valid one.
pub fn join_link(url: &str) -> Option<Link> {
    let url = url.trim();
    if !url.starts_with("peak://join") {
        return None;
    }
    let parsed = reqwest::Url::parse(url).ok()?;
    if parsed.host_str() != Some("join") {
        return None;
    }
    let get = |k: &str| parsed.query_pairs().find(|(key, _)| key == k).map(|(_, v)| v.into_owned());
    let address = get("s")?;
    crate::launcher::servers::parse_address(&address).ok()?;
    let name = get("n").and_then(|n| text(&n, 64)).unwrap_or_else(|| address.clone());
    let pack = match get("p") {
        Some(p) => Some(id(&p)?),
        None => None,
    };
    let version = match get("v") {
        Some(v) if pack.is_some() => Some(id(&v)?),
        _ => None,
    };
    let version_name = get("vn").filter(|_| pack.is_some()).and_then(|v| text(&v, 80));
    let pack_name = get("pn").filter(|_| pack.is_some()).and_then(|v| text(&v, 80));
    let game_version = get("mc").and_then(|v| text(&v, 32)).filter(|v| v.chars().all(|c| c.is_ascii_alphanumeric() || " .-_".contains(c)));
    Some(Link::Join { address: address.trim().to_string(), name, game_version, pack, pack_name, version, version_name })
}

pub fn parse(url: &str) -> Option<Link> {
    listing_id(url).map(|id| Link::Server { id }).or_else(|| join_link(url))
}

/// Bring Peak forward and tell the window what to open.
pub fn handle(app: &AppHandle, urls: impl IntoIterator<Item = String>) {
    let Some(link) = urls.into_iter().find_map(|u| parse(&u)) else { return };
    crate::window::show_main(app);
    if let Some(pending) = app.try_state::<PendingLink>() {
        *pending.0.lock().unwrap() = Some(link.clone());
    }
    let _ = app.emit("open-link", link);
}

/// The link Peak was started with (or got before the page loaded), once.
#[tauri::command]
pub fn take_pending_link(pending: tauri::State<'_, PendingLink>) -> Option<Link> {
    pending.0.lock().unwrap().take()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_server_links() {
        let id = "0f8fad5b-d9cb-469f-a165-70867728950e";
        assert_eq!(listing_id(&format!("peak://server/{id}")).as_deref(), Some(id));
        assert_eq!(listing_id(&format!("peak://server/{id}/")).as_deref(), Some(id));
        assert_eq!(listing_id(&format!("PEAK://server/{id}")), None, "scheme is lower case");
        assert_eq!(listing_id("peak://server/../../etc"), None);
        assert_eq!(listing_id("peak://launch/whatever"), None);
        assert_eq!(listing_id("https://evil.example"), None);
    }

    #[test]
    fn invite_links() {
        let l = parse("peak://join?s=play.example.net%3A25566&n=Summit%20SMP&mc=1.21.1&p=cf%3A1210677&pn=COBBLEVERSE&v=7012345&vn=COBBLEVERSE%201.8.0%20%5BCF%5D");
        assert_eq!(
            l,
            Some(Link::Join {
                address: "play.example.net:25566".into(),
                name: "Summit SMP".into(),
                game_version: Some("1.21.1".into()),
                pack: Some("cf:1210677".into()),
                pack_name: Some("COBBLEVERSE".into()),
                version: Some("7012345".into()),
                version_name: Some("COBBLEVERSE 1.8.0 [CF]".into()),
            })
        );
        // Vanilla: just an address; the name falls back to it.
        assert_eq!(parse("peak://join?s=mc.example.org"), Some(Link::Join { address: "mc.example.org".into(), name: "mc.example.org".into(), game_version: None, pack: None, pack_name: None, version: None, version_name: None }));
        // Anything off is refused.
        assert_eq!(parse("peak://join?s=not%20an%20address"), None);
        assert_eq!(parse("peak://join?n=No%20address"), None);
        assert_eq!(parse("peak://join?s=a.example&p=..%2F..%2Fetc"), None);
        assert_eq!(parse("peak://joinx?s=a.example"), None);
    }
}
