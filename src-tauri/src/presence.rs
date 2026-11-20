//! Discord Rich Presence: "Playing Peak Launcher" on the player's Discord
//! profile, with what they're doing (browsing modpacks, installing, playing
//! a pack on a server). It talks only to the Discord app on this computer.
//!
//! One background thread owns the connection. The window sends it a card
//! whenever what the player is doing changes; identical cards are skipped,
//! and if Discord isn't running the thread just waits and tries again at
//! most once a minute, so this costs nothing while idle.

use discord_rich_presence::activity::{Activity, Assets, Button, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use serde::Deserialize;
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The "Peak Launcher" app on Discord's developer portal.
const CLIENT_ID: &str = "1573237727766511646";
/// Shown when there's no modpack logo (Discord accepts image links).
const PEAK_LOGO: &str = "https://www.peaklauncher.com/img/logo.png";
const RETRY: Duration = Duration::from_secs(60);

/// What to show, as sent by the window.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Card {
    /// First line, e.g. "Playing RLCraft".
    pub details: String,
    /// Second line, e.g. "Forge 1.12.2 · on Apollo Network".
    pub state: Option<String>,
    /// When this started (Unix seconds), for Discord's elapsed timer.
    pub start: Option<i64>,
    /// A modpack logo (https) for the big picture; Peak's logo otherwise.
    pub image: Option<String>,
    /// Hover text for the big picture.
    pub image_text: Option<String>,
    /// One button, e.g. "Get Peak" or "View server".
    pub button_label: Option<String>,
    pub button_url: Option<String>,
}

enum Msg {
    Show(Card),
    Clear,
}

#[derive(Default)]
pub struct Presence(Mutex<Option<Sender<Msg>>>);

/// Discord's limits: 2 to 128 characters per line.
fn line(s: &str) -> Option<String> {
    let s: String = s.trim().chars().take(128).collect();
    match s.chars().count() {
        0 => None,
        1 => Some(format!("{s} ")),
        _ => Some(s),
    }
}

fn https(url: &Option<String>) -> Option<String> {
    url.as_ref().filter(|u| u.starts_with("https://") && u.len() <= 256).cloned()
}

fn send(client: &mut DiscordIpcClient, card: &Card) -> bool {
    let Some(details) = line(&card.details) else { return true };
    let state = card.state.as_deref().and_then(line);
    let image = https(&card.image).unwrap_or_else(|| PEAK_LOGO.to_string());
    let image_text = card.image_text.as_deref().and_then(line);
    let mut assets = Assets::new().large_image(image.as_str());
    if let Some(t) = &image_text {
        assets = assets.large_text(t.as_str());
    }
    if https(&card.image).is_some() {
        assets = assets.small_image(PEAK_LOGO).small_text("Peak Launcher");
    }
    let mut activity = Activity::new().details(details.as_str()).assets(assets);
    if let Some(s) = &state {
        activity = activity.state(s.as_str());
    }
    if let Some(start) = card.start {
        activity = activity.timestamps(Timestamps::new().start(start));
    }
    let label = card.button_label.as_deref().and_then(line);
    let url = https(&card.button_url);
    if let (Some(label), Some(url)) = (&label, &url) {
        activity = activity.buttons(vec![Button::new(label.as_str(), url.as_str())]);
    }
    client.set_activity(activity).is_ok()
}

fn run(rx: std::sync::mpsc::Receiver<Msg>) {
    let mut client: Option<DiscordIpcClient> = None;
    let mut wanted: Option<Card> = None;
    let mut shown: Option<Card> = None;
    let mut last_try: Option<Instant> = None;
    loop {
        match rx.recv_timeout(RETRY) {
            Ok(Msg::Show(card)) => wanted = Some(card),
            Ok(Msg::Clear) => {
                wanted = None;
                if let Some(c) = client.as_mut() {
                    let _ = c.clear_activity();
                }
                shown = None;
                continue;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        let Some(card) = wanted.clone() else { continue };
        if shown.as_ref() == Some(&card) && client.is_some() {
            continue;
        }
        if client.is_none() {
            // Discord not running (or closed): try again at most once a minute.
            if last_try.is_some_and(|t| t.elapsed() < RETRY) {
                continue;
            }
            last_try = Some(Instant::now());
            let mut c = DiscordIpcClient::new(CLIENT_ID);
            if c.connect().is_err() {
                continue;
            }
            client = Some(c);
        }
        if let Some(c) = client.as_mut() {
            if send(c, &card) {
                shown = Some(card);
            } else {
                // Discord went away; reconnect next time.
                let _ = c.close();
                client = None;
                shown = None;
            }
        }
    }
    if let Some(mut c) = client {
        let _ = c.clear_activity();
        let _ = c.close();
    }
}

impl Presence {
    fn sender(&self) -> Sender<Msg> {
        let mut guard = self.0.lock().unwrap();
        if let Some(tx) = guard.as_ref() {
            return tx.clone();
        }
        let (tx, rx) = channel();
        std::thread::Builder::new().name("discord-presence".into()).spawn(move || run(rx)).ok();
        *guard = Some(tx.clone());
        tx
    }
}

/// Show a card on the player's Discord profile.
#[tauri::command]
pub fn presence_show(presence: tauri::State<'_, Presence>, card: Card) {
    let _ = presence.sender().send(Msg::Show(card));
}

/// Remove Peak's card (the switch in Settings was turned off).
#[tauri::command]
pub fn presence_clear(presence: tauri::State<'_, Presence>) {
    if presence.0.lock().unwrap().is_some() {
        let _ = presence.sender().send(Msg::Clear);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_fit_discord() {
        assert_eq!(line(""), None);
        assert_eq!(line("A").as_deref(), Some("A "), "Discord needs at least 2 characters");
        assert_eq!(line(&"x".repeat(300)).unwrap().chars().count(), 128);
        assert_eq!(https(&Some("http://x".into())), None);
        assert_eq!(https(&Some("https://cdn.modrinth.com/a.png".into())).as_deref(), Some("https://cdn.modrinth.com/a.png"));
    }
}
