//! The Storage page: what takes up space in Peak's folder, and clearing the
//! parts that are safe to clear (they're downloaded again if ever needed).

use super::instances;
use super::paths::Paths;
use crate::error::Result;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub total: u64,
    pub parts: Vec<Part>,
    pub instances: Vec<InstanceSize>,
    pub cleanable: Vec<Cleanable>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    pub id: &'static str,
    pub name: &'static str,
    pub bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceSize {
    pub id: String,
    pub name: String,
    pub bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cleanable {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub bytes: u64,
    /// A bigger version of the same clean-up (e.g. all logs, not just old ones).
    pub more: Option<More>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct More {
    pub id: &'static str,
    pub label: &'static str,
    pub bytes: u64,
}

/// Cache folders that only hold downloads Peak has finished with. Skins
/// and the version list are left alone (they're small and used for display).
const CACHE_PARTS: &[&str] = &["modpacks", "installers", "loader-data", "server-packs", "technic", "screenshot-previews"];
/// Old logs and crash reports are kept this long.
const LOG_AGE: Duration = Duration::from_secs(30 * 24 * 3600);

fn size(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else { return 0 };
    if meta.is_file() {
        return meta.len();
    }
    if !meta.is_dir() {
        return 0;
    }
    std::fs::read_dir(path).into_iter().flatten().flatten().map(|e| size(&e.path())).sum()
}

/// Game versions no instance uses (with what they're built on followed).
fn unused_versions(paths: &Paths, insts: &[instances::Instance]) -> Vec<PathBuf> {
    let mut keep = BTreeSet::new();
    for inst in insts {
        let mut next = Some(inst.version_id.clone().unwrap_or_else(|| inst.game_version.clone()));
        keep.insert(inst.game_version.clone());
        while let Some(id) = next.take() {
            if !keep.insert(id.clone()) && id != inst.game_version {
                break;
            }
            next = std::fs::read_to_string(paths.version_json(&id))
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                .and_then(|v| v.get("inheritsFrom")?.as_str().map(String::from))
                .filter(|p| !keep.contains(p));
        }
    }
    std::fs::read_dir(paths.versions())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir() && !keep.contains(&e.file_name().to_string_lossy().into_owned()))
        .map(|e| e.path())
        .collect()
}

/// Logs (never the latest one) and crash reports in every instance, older
/// than `age` if given.
fn logs(paths: &Paths, insts: &[instances::Instance], age: Option<Duration>) -> Vec<PathBuf> {
    let cutoff = age.map(|a| SystemTime::now() - a).unwrap_or_else(SystemTime::now);
    let mut out = Vec::new();
    for inst in insts {
        let game = instances::game_dir(paths, &inst.id);
        for folder in ["logs", "crash-reports"] {
            for e in std::fs::read_dir(game.join(folder)).into_iter().flatten().flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let old = e.metadata().and_then(|m| m.modified()).map(|t| t < cutoff).unwrap_or(false);
                let is_log = name.ends_with(".log.gz")
                    || (folder == "logs" && name.ends_with(".log") && name != "latest.log" && name != "debug.log")
                    || (folder == "crash-reports" && name.ends_with(".txt"));
                if old && is_log && e.path().is_file() {
                    out.push(e.path());
                }
            }
        }
    }
    out
}

fn cache_parts(paths: &Paths) -> Vec<PathBuf> {
    CACHE_PARTS.iter().map(|p| paths.cache().join(p)).filter(|p| p.exists()).collect()
}

fn targets(paths: &Paths, insts: &[instances::Instance], id: &str) -> Vec<PathBuf> {
    match id {
        "cache" => cache_parts(paths),
        "versions" => unused_versions(paths, insts),
        "logs" => logs(paths, insts, Some(LOG_AGE)),
        "logs-all" => logs(paths, insts, None),
        _ => vec![],
    }
}

pub async fn usage(paths: &Paths) -> Result<Usage> {
    let insts = instances::list(paths).await?;
    let paths = paths.clone();
    let usage = tokio::task::spawn_blocking(move || {
        let parts = vec![
            Part { id: "instances", name: "Instances", bytes: size(&paths.instances()) },
            Part { id: "assets", name: "Game sounds and textures", bytes: size(&paths.assets()) },
            Part { id: "libraries", name: "Game and mod loader libraries", bytes: size(&paths.libraries()) },
            Part { id: "versions", name: "Minecraft versions", bytes: size(&paths.versions()) },
            Part { id: "java", name: "Java", bytes: size(&paths.runtimes()) },
            Part { id: "cache", name: "Downloads cache", bytes: size(&paths.cache()) },
            Part { id: "shared", name: "Shared packs and settings", bytes: size(&super::shared::dir(&paths)) },
        ];
        let mut sizes: Vec<InstanceSize> = insts
            .iter()
            .map(|i| InstanceSize { id: i.id.clone(), name: i.name.clone(), bytes: size(&paths.instance(&i.id)) })
            .collect();
        sizes.sort_by(|a, b| b.bytes.cmp(&a.bytes));
        let sum = |id| targets(&paths, &insts, id).iter().map(|p| size(p)).sum::<u64>();
        let cleanable = vec![
            Cleanable {
                id: "cache",
                name: "Finished downloads",
                desc: "Modpack files, loader installers and previews Peak has already used",
                bytes: sum("cache"),
                more: None,
            },
            Cleanable {
                id: "versions",
                name: "Minecraft versions no instance uses",
                desc: "Downloaded again if you make an instance with one",
                bytes: sum("versions"),
                more: None,
            },
            Cleanable {
                id: "logs",
                name: "Old logs and crash reports",
                desc: "Clear: older than 30 days. Clear all: every one. Each instance keeps its latest log.",
                bytes: sum("logs"),
                more: Some(More { id: "logs-all", label: "Clear all", bytes: sum("logs-all") }),
            },
        ];
        let total = parts.iter().map(|p| p.bytes).sum();
        Usage { total, parts, instances: sizes, cleanable }
    })
    .await
    .map_err(|e| crate::error::Error::msg(e.to_string()))?;
    Ok(usage)
}

/// Clear one of the `cleanable` parts. Returns the bytes freed.
pub async fn clean(paths: &Paths, id: &str) -> Result<u64> {
    let insts = instances::list(paths).await?;
    let paths = paths.clone();
    let id = id.to_string();
    tokio::task::spawn_blocking(move || {
        let mut freed = 0;
        for p in targets(&paths, &insts, &id) {
            let bytes = size(&p);
            let gone = if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
            if gone.is_ok() {
                freed += bytes;
            }
        }
        freed
    })
    .await
    .map_err(|e| crate::error::Error::msg(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_versions_instances_use_and_what_they_build_on() {
        let root = std::env::temp_dir().join(format!("peak-storage-{}", uuid::Uuid::new_v4()));
        let paths = Paths::new(root.clone());
        for (id, parent) in [("1.21.4", None), ("fabric-loader-0.16.9-1.21.4", Some("1.21.4")), ("1.20.1", None)] {
            std::fs::create_dir_all(paths.version_dir(id)).unwrap();
            let json = match parent {
                Some(p) => format!(r#"{{"id":"{id}","inheritsFrom":"{p}"}}"#),
                None => format!(r#"{{"id":"{id}"}}"#),
            };
            std::fs::write(paths.version_json(id), json).unwrap();
        }
        let inst: instances::Instance = serde_json::from_value(serde_json::json!({
            "id": "a", "name": "A", "gameVersion": "1.21.4", "loader": "fabric",
            "versionId": "fabric-loader-0.16.9-1.21.4", "createdAt": "2026-01-01T00:00:00Z"
        }))
        .unwrap();
        let unused: Vec<String> = unused_versions(&paths, &[inst]).iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(unused, vec!["1.20.1"]);
        let _ = std::fs::remove_dir_all(root);
    }
}
