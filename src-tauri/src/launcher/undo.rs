//! "Undo the update": when mods, resource packs or shaders are updated or
//! switched to another version, the old files are kept aside instead of
//! deleted, so the last update can be put back in one click (from the
//! Content tab, or the crash helper when a game crashes right after).
//!
//! Changes a few minutes apart count as one update (Update all installs mods
//! one after another). Anything older is let go when a new update starts.

use super::instances;
use super::paths::{self, Paths};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Changes closer together than this are one update.
const SAME_UPDATE_MINUTES: i64 = 10;
const FILE: &str = "undo.json";

#[derive(Debug, Default, Serialize, Deserialize)]
struct Record {
    at: Option<chrono::DateTime<chrono::Utc>>,
    entries: Vec<Entry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Entry {
    /// Folder in the game directory, e.g. "mods".
    folder: String,
    /// The old file's name, as it was.
    original: String,
    /// Where it's kept, inside the undo folder.
    kept: String,
    /// The file that replaced it.
    added: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoInfo {
    pub count: usize,
    pub when: String,
}

fn dir(paths: &Paths, inst: &str) -> PathBuf {
    paths.instance(inst).join("peak-undo")
}

async fn load(paths: &Paths, inst: &str) -> Record {
    paths::read_json(&dir(paths, inst).join(FILE)).await.ok().flatten().unwrap_or_default()
}

/// Keep `original` (in `folder` of the game directory) aside, as part of the
/// current update, because `added` replaced it.
pub async fn keep(paths: &Paths, inst: &str, folder: &str, original: &str, added: &str) -> Result<()> {
    let undo = dir(paths, inst);
    let mut rec = load(paths, inst).await;
    let now = chrono::Utc::now();
    let fresh = rec.at.is_some_and(|t| (now - t).num_minutes() < SAME_UPDATE_MINUTES);
    if !fresh {
        // A new update: let the previous one go.
        let _ = tokio::fs::remove_dir_all(&undo).await;
        rec = Record::default();
    }
    tokio::fs::create_dir_all(undo.join("files")).await?;
    let kept = format!("{}-{}", rec.entries.len(), original);
    let from = instances::game_dir(paths, inst).join(folder).join(original);
    move_file(&from, &undo.join("files").join(&kept)).await?;
    rec.entries.push(Entry { folder: folder.into(), original: original.into(), kept, added: added.into() });
    rec.at = Some(now);
    paths::write_json(&undo.join(FILE), &rec).await
}

async fn move_file(from: &Path, to: &Path) -> Result<()> {
    if tokio::fs::rename(from, to).await.is_err() {
        tokio::fs::copy(from, to).await?;
        tokio::fs::remove_file(from).await?;
    }
    Ok(())
}

/// The update that can be undone, if any.
pub async fn info(paths: &Paths, inst: &str) -> Option<UndoInfo> {
    let rec = load(paths, inst).await;
    let at = rec.at?;
    (!rec.entries.is_empty()).then(|| UndoInfo { count: rec.entries.len(), when: at.to_rfc3339() })
}

/// Put the last update back. Returns how many files were restored.
pub async fn undo(paths: &Paths, inst: &str) -> Result<usize> {
    let rec = load(paths, inst).await;
    let undo = dir(paths, inst);
    let game = instances::game_dir(paths, inst);
    let mut n = 0;
    for e in rec.entries.iter().rev() {
        let folder = game.join(&e.folder);
        let kept = undo.join("files").join(&e.kept);
        if !kept.exists() || e.folder.contains(['/', '\\']) || e.original.contains(['/', '\\']) || e.added.contains(['/', '\\']) {
            continue;
        }
        if e.added != e.original {
            let _ = tokio::fs::remove_file(folder.join(&e.added)).await;
        }
        tokio::fs::create_dir_all(&folder).await?;
        move_file(&kept, &folder.join(&e.original)).await?;
        n += 1;
    }
    let _ = tokio::fs::remove_dir_all(&undo).await;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn keeps_and_puts_back_the_old_files() {
        let root = std::env::temp_dir().join(format!("peak-undo-{}", uuid::Uuid::new_v4()));
        let paths = Paths::new(root.clone());
        let mods = instances::game_dir(&paths, "a").join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("sodium-0.5.jar"), b"old").unwrap();
        std::fs::write(mods.join("lithium-0.1.jar.disabled"), b"old-off").unwrap();

        // Update two mods (the new files are already downloaded).
        std::fs::write(mods.join("sodium-0.6.jar"), b"new").unwrap();
        keep(&paths, "a", "mods", "sodium-0.5.jar", "sodium-0.6.jar").await.unwrap();
        std::fs::write(mods.join("lithium-0.2.jar.disabled"), b"new-off").unwrap();
        keep(&paths, "a", "mods", "lithium-0.1.jar.disabled", "lithium-0.2.jar.disabled").await.unwrap();
        assert!(!mods.join("sodium-0.5.jar").exists());
        assert_eq!(info(&paths, "a").await.unwrap().count, 2);

        assert_eq!(undo(&paths, "a").await.unwrap(), 2);
        assert_eq!(std::fs::read(mods.join("sodium-0.5.jar")).unwrap(), b"old");
        assert_eq!(std::fs::read(mods.join("lithium-0.1.jar.disabled")).unwrap(), b"old-off");
        assert!(!mods.join("sodium-0.6.jar").exists() && !mods.join("lithium-0.2.jar.disabled").exists());
        assert!(info(&paths, "a").await.is_none());
        let _ = std::fs::remove_dir_all(root);
    }
}
