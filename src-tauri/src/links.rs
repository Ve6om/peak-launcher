//! peak:// links, so the website's "Play in Peak" button can open Peak on a
//! server from the server list. Only server-list ids are accepted; a link
//! never starts the game by itself (the player still presses Play).
//!
//!   peak://server/<listing id>

use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

/// A link that arrived before the window was ready to hear about it.
#[derive(Default)]
pub struct PendingLink(pub Mutex<Option<String>>);

/// The listing id in a peak:// link, if it's one Peak understands.
pub fn listing_id(url: &str) -> Option<String> {
    let rest = url.trim().strip_prefix("peak://")?;
    let rest = rest.trim_end_matches('/');
    let id = rest.strip_prefix("server/")?;
    let ok = id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    ok.then(|| id.to_ascii_lowercase())
}

/// Bring Peak forward and tell the window which server to open.
pub fn handle(app: &AppHandle, urls: impl IntoIterator<Item = String>) {
    let Some(id) = urls.into_iter().find_map(|u| listing_id(&u)) else { return };
    crate::window::show_main(app);
    if let Some(pending) = app.try_state::<PendingLink>() {
        *pending.0.lock().unwrap() = Some(id.clone());
    }
    let _ = app.emit("open-server-link", id);
}

/// The link Peak was started with (or got before the page loaded), once.
#[tauri::command]
pub fn take_pending_link(pending: tauri::State<'_, PendingLink>) -> Option<String> {
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
}
