//! Files the user has to download by hand (CurseForge authors can turn off
//! launcher downloads). We keep a list per instance, watch the Downloads
//! folder, and move each file into place once it shows up, the way
//! ATLauncher does.

use super::curseforge::Blocked;
use super::instances;
use super::net;
use super::paths::{self, Paths};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualFile {
    pub name: String,
    pub url: String,
    pub file_name: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: u64,
    /// "mods" | "resourcepacks" | "shaderpacks"
    pub folder: String,
    #[serde(default)]
    pub done: bool,
}

impl From<Blocked> for ManualFile {
    fn from(b: Blocked) -> Self {
        ManualFile { name: b.name, url: b.url, file_name: b.file_name, sha1: b.sha1, size: b.size, folder: b.folder, done: false }
    }
}

fn store(paths: &Paths, inst: &str) -> PathBuf {
    paths.instance(inst).join("manual-downloads.json")
}

pub async fn load(paths: &Paths, inst: &str) -> Result<Vec<ManualFile>> {
    let mut list: Vec<ManualFile> = paths::read_json(&store(paths, inst)).await?.unwrap_or_default();
    for f in &mut list {
        f.url = direct_link(&f.url);
    }
    Ok(list)
}

/// Lists saved by older versions link to CurseForge's /files/<id> page; the
/// /download/<id> page starts the download instead.
fn direct_link(url: &str) -> String {
    if url.starts_with("https://www.curseforge.com/minecraft/") {
        if let Some((base, id)) = url.rsplit_once("/files/") {
            if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
                return format!("{base}/download/{id}");
            }
        }
    }
    url.to_string()
}

pub async fn save(paths: &Paths, inst: &str, list: &[ManualFile]) -> Result<()> {
    if list.is_empty() || list.iter().all(|f| f.done) {
        let _ = tokio::fs::remove_file(store(paths, inst)).await;
        return Ok(());
    }
    paths::write_json(&store(paths, inst), &list).await
}

/// Lower-case, letters and digits only, so browser renames still match.
fn normalize(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase()
}

/// Does a file in Downloads look like `wanted`? Exact name, or the name a
/// browser gives a repeat download ("name (1).jar"), ignoring punctuation.
pub fn name_matches(candidate: &str, wanted: &str) -> bool {
    if candidate == wanted {
        return true;
    }
    let (c_stem, c_ext) = split_ext(candidate);
    let (w_stem, w_ext) = split_ext(wanted);
    if !c_ext.eq_ignore_ascii_case(w_ext) {
        return false;
    }
    // Strip a trailing " (n)" that browsers add for duplicates.
    let c_stem = match c_stem.rfind(" (") {
        Some(i) if c_stem.ends_with(')') && c_stem[i + 2..c_stem.len() - 1].chars().all(|c| c.is_ascii_digit()) => &c_stem[..i],
        _ => c_stem,
    };
    normalize(c_stem) == normalize(w_stem)
}

fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i + 1..]),
        _ => (name, ""),
    }
}

/// Move a file, falling back to copy + delete across drives.
fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    std::fs::copy(from, to)?;
    std::fs::remove_file(from)
}

/// Look in `downloads` for any pending files, move the ones found into the
/// instance, and return the updated list.
pub async fn scan(paths: &Paths, inst_id: &str, downloads: &Path) -> Result<Vec<ManualFile>> {
    let mut list = load(paths, inst_id).await?;
    if list.iter().all(|f| f.done) {
        return Ok(list);
    }
    let game = instances::game_dir(paths, inst_id);
    let mut entries: Vec<(String, PathBuf, u64)> = Vec::new();
    if let Ok(mut rd) = tokio::fs::read_dir(downloads).await {
        while let Some(e) = rd.next_entry().await? {
            let Ok(meta) = e.metadata().await else { continue };
            if meta.is_file() {
                entries.push((e.file_name().to_string_lossy().into_owned(), e.path(), meta.len()));
            }
        }
    }
    let mut changed = false;
    for f in list.iter_mut().filter(|f| !f.done) {
        // Already placed by hand?
        let target = game.join(&f.folder).join(&f.file_name);
        if target.exists() {
            f.done = true;
            changed = true;
            continue;
        }
        for (name, path, size) in &entries {
            if !name_matches(name, &f.file_name) {
                continue;
            }
            if f.size > 0 && *size != f.size {
                continue; // still downloading, or a different file
            }
            if let Some(expected) = &f.sha1 {
                match net::sha1_file(path).await {
                    Ok(h) if h.eq_ignore_ascii_case(expected) => {}
                    _ => continue,
                }
            }
            let (from, to) = (path.clone(), target.clone());
            if tokio::task::spawn_blocking(move || move_file(&from, &to)).await.map(|r| r.is_ok()).unwrap_or(false) {
                f.done = true;
                changed = true;
                break;
            }
        }
    }
    if changed {
        save(paths, inst_id, &list).await?;
    }
    Ok(list)
}

#[cfg(test)]
mod tests {
    #[test]
    fn upgrades_curseforge_links() {
        assert_eq!(
            direct_link("https://www.curseforge.com/minecraft/mc-mods/scape-and-run-parasites/files/4436467"),
            "https://www.curseforge.com/minecraft/mc-mods/scape-and-run-parasites/download/4436467"
        );
        let other = "https://www.curseforge.com/minecraft/mc-mods/foo/files/all";
        assert_eq!(direct_link(other), other);
        assert_eq!(direct_link("https://example.com/files/1"), "https://example.com/files/1");
    }

    use super::*;

    #[test]
    fn matches_browser_renames() {
        assert!(name_matches("jei-1.20.1-forge-15.2.0.jar", "jei-1.20.1-forge-15.2.0.jar"));
        assert!(name_matches("jei-1.20.1-forge-15.2.0 (1).jar", "jei-1.20.1-forge-15.2.0.jar"));
        assert!(name_matches("Some_Mod_1.2.jar", "Some Mod+1.2.jar"));
        assert!(!name_matches("jei-1.20.1-forge-15.2.0.jar.crdownload", "jei-1.20.1-forge-15.2.0.jar"));
        assert!(!name_matches("other-mod.jar", "jei.jar"));
    }

    #[tokio::test]
    async fn scan_moves_found_files() {
        let root = std::env::temp_dir().join(format!("peak-manual-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let downloads = root.join("Downloads");
        std::fs::create_dir_all(&downloads).unwrap();
        let paths = Paths::new(root.join("peak"));
        let inst = instances::create(&paths, "Manual", "1.20.1", "forge", Some("1.20.1-47.3.0".into())).await.unwrap();

        let body = b"pretend mod";
        let sha = {
            use sha1::{Digest, Sha1};
            hex::encode(Sha1::digest(body))
        };
        let list = vec![
            ManualFile { name: "Mod A".into(), url: "https://x".into(), file_name: "mod-a.jar".into(), sha1: Some(sha), size: body.len() as u64, folder: "mods".into(), done: false },
            ManualFile { name: "Pack B".into(), url: "https://y".into(), file_name: "pack-b.zip".into(), sha1: None, size: 0, folder: "resourcepacks".into(), done: false },
        ];
        save(&paths, &inst.id, &list).await.unwrap();

        // Only the mod has been downloaded so far (with a browser rename).
        std::fs::write(downloads.join("mod-a (1).jar"), body).unwrap();
        let after = scan(&paths, &inst.id, &downloads).await.unwrap();
        assert!(after[0].done && !after[1].done);
        let game = instances::game_dir(&paths, &inst.id);
        assert!(game.join("mods/mod-a.jar").exists());
        assert!(!downloads.join("mod-a (1).jar").exists(), "file should be moved out of Downloads");

        std::fs::write(downloads.join("pack-b.zip"), b"zip").unwrap();
        let after = scan(&paths, &inst.id, &downloads).await.unwrap();
        assert!(after.iter().all(|f| f.done));
        assert!(load(&paths, &inst.id).await.unwrap().is_empty(), "list is cleared when everything is done");
        let _ = std::fs::remove_dir_all(root);
    }
}
