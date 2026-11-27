//! "Check my connection": can Peak reach each service it downloads from or
//! signs in with? Any answer from a server counts as reachable (even "not
//! found"); only no answer at all, or a timeout, counts as blocked.

use super::net;
use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub name: String,
    /// What it's needed for.
    pub used_for: String,
    pub host: String,
    pub ok: bool,
    pub ms: Option<u64>,
    pub error: Option<String>,
}

const TARGETS: &[(&str, &str, &str)] = &[
    ("Minecraft versions", "Installing and updating Minecraft", "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"),
    ("Minecraft libraries", "Installing Minecraft", "https://libraries.minecraft.net/"),
    ("Minecraft assets", "Sounds, languages and textures", "https://resources.download.minecraft.net/"),
    ("Microsoft sign-in", "Signing in", "https://login.microsoftonline.com/consumers/v2.0/.well-known/openid-configuration"),
    ("Xbox Live", "Signing in", "https://user.auth.xboxlive.com/"),
    ("Minecraft services", "Signing in, skins", "https://api.minecraftservices.com/"),
    ("Modrinth", "Mods and modpacks", "https://api.modrinth.com/v2/"),
    ("CurseForge", "Mods and modpacks", "https://api.peaklauncher.com/"),
    ("CurseForge downloads", "Mods and modpacks", "https://mediafilez.forgecdn.net/"),
    ("Fabric", "Fabric and Quilt instances", "https://meta.fabricmc.net/v2/versions/loader"),
    ("Forge", "Forge instances", "https://maven.minecraftforge.net/"),
    ("NeoForge", "NeoForge instances", "https://maven.neoforged.net/"),
    ("Java downloads", "Java for the game", "https://api.adoptium.net/v3/info/available_releases"),
    ("GitHub", "Peak updates", "https://api.github.com/"),
];

async fn check(name: &str, used_for: &str, url: &str) -> Check {
    let host = reqwest::Url::parse(url).ok().and_then(|u| u.host_str().map(String::from)).unwrap_or_default();
    let started = Instant::now();
    let res = net::client().get(url).timeout(Duration::from_secs(10)).send().await;
    let (ok, error) = match res {
        Ok(_) => (true, None),
        Err(e) if e.is_timeout() => (false, Some("Timed out".to_string())),
        Err(e) if e.is_connect() => (false, Some("Couldn't connect".to_string())),
        Err(e) => (false, Some(short(&e.to_string()))),
    };
    Check { name: name.into(), used_for: used_for.into(), host, ok, ms: ok.then(|| started.elapsed().as_millis() as u64), error }
}

fn short(e: &str) -> String {
    let e = e.trim();
    if e.chars().count() > 120 { format!("{}…", e.chars().take(120).collect::<String>()) } else { e.to_string() }
}

/// Check every service at once.
pub async fn run() -> Vec<Check> {
    futures::future::join_all(TARGETS.iter().map(|&(n, u, url)| check(n, u, url))).await
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[ignore]
    async fn netcheck_live() {
        for c in super::run().await {
            println!("{:<22} {:<5} {:?} {:?}", c.name, c.ok, c.ms, c.error);
        }
    }
}
