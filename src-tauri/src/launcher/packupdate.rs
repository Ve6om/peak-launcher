//! Modpack updates: deciding whether an installed pack has a newer version,
//! and moving the old mods aside before a pack switches version.

use super::instances::{self, PackInfo};
use super::modrinth::Version;
use super::paths::Paths;
use crate::error::Result;
use serde::Serialize;

/// A modpack instance with a newer version available.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackUpdate {
    pub instance_id: String,
    pub instance_name: String,
    pub pack_name: String,
    pub current: String,
    pub latest_id: String,
    pub latest: String,
    pub icon_url: Option<String>,
    /// The player skipped this version (still shown as outdated, not asked about).
    pub skipped: bool,
    /// The player turned update reminders off for this pack.
    pub muted: bool,
}

/// The version a pack would update to: the newest release (the newest of
/// anything if it has no releases yet). None when the installed version is
/// already that one, newer, or the player skipped it.
pub fn newer<'a>(versions: &'a [Version], pack: &PackInfo) -> Option<&'a Version> {
    let current_id = pack.version_id.as_deref()?;
    let target = versions.iter().find(|v| v.version_type == "release").or_else(|| versions.first())?;
    if target.id == current_id || pack.skipped_version.as_deref() == Some(target.id.as_str()) {
        return None;
    }
    let current = versions.iter().find(|v| v.id == current_id);
    match current {
        // Dates compare as text (ISO 8601); some sources don't give dates.
        Some(c) if !c.date_published.is_empty() && !target.date_published.is_empty() => {
            (target.date_published > c.date_published).then_some(target)
        }
        // No dates (Technic): compare the build numbers, like 1.10.2 > 1.9.
        Some(c) => (natural_cmp(&target.version_number, &c.version_number) == std::cmp::Ordering::Greater).then_some(target),
        // The installed version is gone from the list (removed by its author):
        // only suggest the update if we can tell it's newer by date.
        None => None,
    }
}

/// Compare version names with numbers as numbers ("1.10" after "1.9").
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    fn parts(s: &str) -> Vec<(bool, String)> {
        let mut out: Vec<(bool, String)> = Vec::new();
        for ch in s.chars() {
            let digit = ch.is_ascii_digit();
            match out.last_mut() {
                Some((d, part)) if *d == digit => part.push(ch),
                _ => out.push((digit, ch.to_string())),
            }
        }
        out
    }
    let (pa, pb) = (parts(a), parts(b));
    for (x, y) in pa.iter().zip(pb.iter()) {
        let ord = match (x.0, y.0) {
            (true, true) => {
                let (nx, ny) = (x.1.trim_start_matches('0'), y.1.trim_start_matches('0'));
                nx.len().cmp(&ny.len()).then_with(|| nx.cmp(ny))
            }
            _ => x.1.to_lowercase().cmp(&y.1.to_lowercase()),
        };
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
    }
    pa.len().cmp(&pb.len())
}

/// Move the instance's mods folder into a backup next to the game folder, so
/// a version change starts from the pack's own mods. Mods the player added
/// themselves stay in the backup. Returns the backup folder, if there was one.
pub async fn back_up_mods(paths: &Paths, id: &str, old_version: &str) -> Result<Option<std::path::PathBuf>> {
    let mods = instances::game_dir(paths, id).join("mods");
    if !mods.exists() {
        return Ok(None);
    }
    let safe: String = old_version.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).take(40).collect();
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    let dest = paths.instance(id).join("backups").join(format!("mods-{safe}-{stamp}"));
    tokio::fs::create_dir_all(dest.parent().unwrap()).await?;
    tokio::fs::rename(&mods, &dest).await?;
    Ok(Some(dest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(id: &str, kind: &str, date: &str) -> Version {
        serde_json::from_value(serde_json::json!({
            "id": id, "project_id": "p", "name": id, "version_number": id,
            "version_type": kind, "date_published": date
        }))
        .unwrap()
    }

    fn pack(current: &str) -> PackInfo {
        PackInfo { version_id: Some(current.into()), ..Default::default() }
    }

    #[test]
    fn finds_a_newer_release() {
        let list = vec![v("3b", "beta", "2026-03-01"), v("2", "release", "2026-02-01"), v("1", "release", "2026-01-01")];
        assert_eq!(newer(&list, &pack("1")).map(|x| x.id.as_str()), Some("2"), "betas are passed over");
        assert!(newer(&list, &pack("2")).is_none(), "already on the newest release");
        assert!(newer(&list, &pack("3b")).is_none(), "a beta newer than the release isn't downgraded");
    }

    #[test]
    fn respects_skipped_versions() {
        let list = vec![v("2", "release", "2026-02-01"), v("1", "release", "2026-01-01")];
        let mut p = pack("1");
        p.skipped_version = Some("2".into());
        assert!(newer(&list, &p).is_none());
        let list3 = vec![v("3", "release", "2026-03-01"), v("2", "release", "2026-02-01"), v("1", "release", "2026-01-01")];
        assert_eq!(newer(&list3, &p).map(|x| x.id.as_str()), Some("3"), "a later update is offered again");
    }

    #[test]
    fn works_without_dates() {
        // Technic: recommended build first, no dates.
        let list = vec![v("1.4", "release", ""), v("1.5", "beta", ""), v("1.3", "beta", "")];
        assert_eq!(newer(&list, &pack("1.3")).map(|x| x.id.as_str()), Some("1.4"));
        assert!(newer(&list, &pack("1.5")).is_none(), "a newer test build isn't downgraded");
    }

    #[test]
    fn compares_build_numbers() {
        use std::cmp::Ordering::*;
        assert_eq!(natural_cmp("1.10.2", "1.9"), Greater);
        assert_eq!(natural_cmp("1.0.4", "1.0.4"), Equal);
        assert_eq!(natural_cmp("2.0", "10.0"), Less);
    }

    #[test]
    fn unknown_or_file_packs_get_nothing() {
        let list = vec![v("2", "release", "2026-02-01")];
        assert!(newer(&list, &pack("gone")).is_none());
        assert!(newer(&list, &PackInfo::default()).is_none(), "packs from a file have no version id");
        assert!(newer(&[], &pack("1")).is_none());
    }

    #[tokio::test]
    async fn moves_mods_to_a_backup() {
        let root = std::env::temp_dir().join(format!("peak-pack-backup-{}", std::process::id()));
        let paths = Paths::new(root.clone());
        let mods = instances::game_dir(&paths, "inst").join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("mine.jar"), b"x").unwrap();
        let dest = back_up_mods(&paths, "inst", "1.2.3 (Forge)").await.unwrap().unwrap();
        assert!(!mods.exists());
        assert!(dest.join("mine.jar").exists());
        assert!(dest.file_name().unwrap().to_string_lossy().starts_with("mods-1.2.3__Forge_-"));
        assert!(back_up_mods(&paths, "inst", "x").await.unwrap().is_none(), "nothing to move");
        std::fs::remove_dir_all(root).ok();
    }
}
