//! Mojang version manifest and per-version JSON formats.

use super::net;
use super::paths::{self, Paths};
use super::rules::Rule;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManifest {
    pub latest: Latest,
    pub versions: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub release_time: String,
    #[serde(default)]
    pub sha1: Option<String>,
}

/// Fetch the manifest, falling back to the cached copy when offline.
pub async fn manifest(paths: &Paths) -> Result<VersionManifest> {
    let cache = paths.cache().join("version_manifest.json");
    match net::get_json::<VersionManifest>(MANIFEST_URL).await {
        Ok(m) => {
            let _ = paths::write_json(&cache, &m).await;
            Ok(m)
        }
        Err(e) => match paths::read_json::<VersionManifest>(&cache).await? {
            Some(m) => Ok(m),
            None => Err(e),
        },
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(default)]
    pub main_class: String,
    #[serde(default)]
    pub arguments: Option<Arguments>,
    #[serde(default)]
    pub minecraft_arguments: Option<String>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub downloads: Option<VersionDownloads>,
    #[serde(default)]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(default)]
    pub java_version: Option<JavaVersion>,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub logging: Option<Logging>,
    /// Mod loader profiles extend a vanilla version.
    #[serde(default)]
    pub inherits_from: Option<String>,
}

impl VersionJson {
    /// Combine a loader profile with the version it inherits from, the same
    /// way the official launcher does: child values win, libraries and
    /// arguments are combined (child libraries first so loaders can override).
    pub fn merge(self, parent: VersionJson) -> VersionJson {
        let key = |l: &Library| {
            let parts: Vec<&str> = l.name.split('@').next().unwrap_or("").split(':').collect();
            match parts.as_slice() {
                [g, a, _, c, ..] => format!("{g}:{a}:{c}"),
                [g, a, ..] => format!("{g}:{a}"),
                _ => l.name.clone(),
            }
        };
        // Only drop parent entries the child replaces. A profile can list the
        // same artifact more than once with different OS rules (1.18.2 has an
        // osx-only lwjgl 3.2.1 next to 3.2.2 for everything else), so entries
        // within one profile must never be collapsed or the wrong one survives.
        let overridden: std::collections::HashSet<String> = self.libraries.iter().map(key).collect();
        let mut libraries = self.libraries;
        libraries.extend(parent.libraries.into_iter().filter(|l| !overridden.contains(&key(l))));
        let arguments = match (parent.arguments, self.arguments) {
            (Some(mut p), Some(c)) => {
                p.game.extend(c.game);
                p.jvm.extend(c.jvm);
                Some(p)
            }
            (p, c) => c.or(p),
        };
        VersionJson {
            id: self.id,
            main_class: if self.main_class.is_empty() { parent.main_class } else { self.main_class },
            minecraft_arguments: self.minecraft_arguments.or(parent.minecraft_arguments),
            arguments,
            libraries,
            downloads: self.downloads.or(parent.downloads),
            asset_index: self.asset_index.or(parent.asset_index),
            assets: self.assets.or(parent.assets),
            java_version: self.java_version.or(parent.java_version),
            kind: self.kind.or(parent.kind),
            logging: self.logging.or(parent.logging),
            inherits_from: parent.inherits_from,
        }
    }
}

/// Load a version and everything it inherits from, fully merged.
pub async fn resolve(paths: &Paths, id: &str) -> Result<VersionJson> {
    let mut version = load_version_json(paths, id).await?;
    let mut depth = 0;
    while let Some(parent_id) = version.inherits_from.clone() {
        depth += 1;
        if depth > 5 {
            return Err(Error::msg("Version inheritance is too deep"));
        }
        let parent = load_version_json(paths, &parent_id).await?;
        version = version.merge(parent);
    }
    Ok(version)
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Arg>,
    #[serde(default)]
    pub jvm: Vec<Arg>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Arg {
    Plain(String),
    Ruled { rules: Vec<Rule>, value: ArgValue },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionDownloads {
    pub client: Option<Artifact>,
    #[serde(default)]
    pub server: Option<Artifact>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Artifact {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub component: String,
    #[allow(dead_code)]
    pub major_version: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Logging {
    pub client: Option<LoggingClient>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingClient {
    pub argument: String,
    pub file: LogFile,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogFile {
    pub id: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    #[serde(default)]
    pub rules: Option<Vec<Rule>>,
    #[serde(default)]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryDownloads {
    #[serde(default)]
    pub artifact: Option<Artifact>,
    #[serde(default)]
    pub classifiers: Option<HashMap<String, Artifact>>,
}

impl Library {
    /// Old-style (pre-1.19) natives classifier for this OS, e.g. "natives-windows".
    pub fn native_classifier(&self) -> Option<String> {
        let natives = self.natives.as_ref()?;
        let c = natives.get(super::rules::os_name())?;
        Some(c.replace("${arch}", if super::rules::is_64bit() { "64" } else { "32" }))
    }
}

/// Convert a maven coordinate ("group:artifact:version[:classifier][@ext]")
/// into its repository-relative path.
pub fn maven_path(name: &str) -> Option<String> {
    let (coords, ext) = match name.split_once('@') {
        Some((c, e)) => (c, e),
        None => (name, "jar"),
    };
    let parts: Vec<&str> = coords.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let (group, artifact, version) = (parts[0].replace('.', "/"), parts[1], parts[2]);
    let file = match parts.get(3) {
        Some(classifier) => format!("{artifact}-{version}-{classifier}.{ext}"),
        None => format!("{artifact}-{version}.{ext}"),
    };
    Some(format!("{group}/{artifact}/{version}/{file}"))
}

pub async fn load_version_json(paths: &Paths, id: &str) -> Result<VersionJson> {
    paths::read_json::<VersionJson>(&paths.version_json(id))
        .await?
        .ok_or_else(|| Error::msg(format!("Version {id} is not installed")))
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
    #[serde(default)]
    pub map_to_resources: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

impl AssetObject {
    pub fn rel_path(&self) -> String {
        format!("{}/{}", &self.hash[..2], self.hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(id: &str, libs: serde_json::Value) -> VersionJson {
        serde_json::from_value(serde_json::json!({ "id": id, "libraries": libs })).unwrap()
    }

    #[test]
    fn merge_keeps_parent_entries_split_by_os() {
        let parent = version(
            "1.18.2",
            serde_json::json!([
                { "name": "org.lwjgl:lwjgl:3.2.1", "rules": [{ "action": "allow", "os": { "name": "osx" } }] },
                { "name": "org.lwjgl:lwjgl:3.2.2", "rules": [{ "action": "allow" }, { "action": "disallow", "os": { "name": "osx" } }] },
                { "name": "com.google.guava:guava:21.0" },
            ]),
        );
        let child = version("1.18.2-forge", serde_json::json!([{ "name": "com.google.guava:guava:31.0.1-jre" }]));
        let names: Vec<_> = child.merge(parent).libraries.into_iter().map(|l| l.name).collect();
        assert_eq!(names, ["com.google.guava:guava:31.0.1-jre", "org.lwjgl:lwjgl:3.2.1", "org.lwjgl:lwjgl:3.2.2"]);
    }
}
