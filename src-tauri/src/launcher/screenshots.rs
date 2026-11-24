//! The Screenshots page: every screenshot from every instance in one place,
//! with small cached previews, the player's own tags, and copy / export.
//! Screenshots stay where Minecraft saved them; tags live in Peak's own file.

use super::instances::{self, Instance};
use super::paths::Paths;
use crate::error::{Error, Result};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    /// "<instance id>/<file name>": stable, used for tags.
    pub key: String,
    pub instance_id: String,
    pub file: String,
    pub path: String,
    /// When it was taken (from Minecraft's file name, else the file's time), RFC 3339.
    pub taken_at: String,
    pub size_bytes: u64,
    pub tags: Vec<String>,
}

fn screenshots_dir(paths: &Paths, id: &str) -> PathBuf {
    instances::game_dir(paths, id).join("screenshots")
}

fn is_image(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".png") || n.ends_with(".jpg") || n.ends_with(".jpeg")
}

/// Every screenshot, newest first.
pub async fn list(paths: &Paths, instances: &[Instance]) -> Vec<Screenshot> {
    let tags = load_tags(paths).await;
    let dirs: Vec<(String, PathBuf)> = instances.iter().map(|i| (i.id.clone(), screenshots_dir(paths, &i.id))).collect();
    let mut out = tokio::task::spawn_blocking(move || {
        let mut out = Vec::new();
        for (id, dir) in dirs {
            for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                let file = e.file_name().to_string_lossy().into_owned();
                let Ok(meta) = e.metadata() else { continue };
                if !meta.is_file() || !is_image(&file) {
                    continue;
                }
                let taken_at = taken_from_name(&file).unwrap_or_else(|| {
                    let t: chrono::DateTime<chrono::Local> = meta.modified().map(Into::into).unwrap_or_else(|_| chrono::Local::now());
                    t.to_rfc3339()
                });
                out.push(Screenshot {
                    key: format!("{id}/{file}"),
                    instance_id: id.clone(),
                    path: e.path().to_string_lossy().into_owned(),
                    file,
                    taken_at,
                    size_bytes: meta.len(),
                    tags: Vec::new(),
                });
            }
        }
        out
    })
    .await
    .unwrap_or_default();
    for s in &mut out {
        s.tags = tags.get(&s.key).cloned().unwrap_or_default();
    }
    out.sort_by(|a, b| b.taken_at.cmp(&a.taken_at));
    out
}

/// Minecraft names screenshots after the local time they were taken:
/// "2026-11-22_14.03.12.png", or "..._2.png" for a second one that second.
fn taken_from_name(file: &str) -> Option<String> {
    use chrono::{Local, NaiveDateTime, TimeZone};
    let stem = file.get(..19)?;
    let naive = NaiveDateTime::parse_from_str(stem, "%Y-%m-%d_%H.%M.%S").ok()?;
    Some(Local.from_local_datetime(&naive).earliest()?.to_rfc3339())
}

// ---- previews ----

const THUMB_WIDTH: u32 = 480;

/// A small JPEG preview of a screenshot, made once and kept in the cache.
pub async fn thumbnail(paths: &Paths, instance_id: &str, file: &str) -> Result<PathBuf> {
    let src = checked(paths, instance_id, file)?;
    let meta = tokio::fs::metadata(&src).await?;
    let stamp = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
    let name = format!("{}-{:x}.jpg", sanitize(&format!("{instance_id}-{file}")), stamp);
    let dir = paths.cache().join("screenshot-previews");
    let out = dir.join(name);
    if out.exists() {
        return Ok(out);
    }
    tokio::fs::create_dir_all(&dir).await?;
    let out2 = out.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let img = image::open(&src).map_err(|e| Error::msg(format!("Couldn't read the screenshot: {e}")))?;
        let small = img.thumbnail(THUMB_WIDTH, THUMB_WIDTH * 2).to_rgb8();
        let tmp = out2.with_extension("part");
        small.save_with_format(&tmp, image::ImageFormat::Jpeg).map_err(|e| Error::msg(e.to_string()))?;
        std::fs::rename(&tmp, &out2)?;
        Ok(())
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))??;
    Ok(out)
}

fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect()
}

/// The real path of a screenshot, refusing anything outside its folder.
pub fn checked(paths: &Paths, instance_id: &str, file: &str) -> Result<PathBuf> {
    let bad = |s: &str| s.is_empty() || s.contains(['/', '\\']) || s.contains("..");
    if bad(instance_id) || bad(file) || !is_image(file) {
        return Err(Error::msg("That isn't a screenshot"));
    }
    let p = screenshots_dir(paths, instance_id).join(file);
    if !p.is_file() {
        return Err(Error::msg("That screenshot isn't there any more"));
    }
    Ok(p)
}

// ---- tags ----

fn tags_file(paths: &Paths) -> PathBuf {
    paths.root.join("screenshot-tags.json")
}

async fn load_tags(paths: &Paths) -> BTreeMap<String, Vec<String>> {
    tokio::fs::read_to_string(tags_file(paths)).await.ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Set one screenshot's tags (trimmed, lower-case, no repeats, at most 12).
pub async fn set_tags(paths: &Paths, key: &str, tags: Vec<String>) -> Result<Vec<String>> {
    let mut clean: Vec<String> = Vec::new();
    for t in tags {
        let t: String = t.trim().to_lowercase().chars().take(24).collect();
        if !t.is_empty() && !clean.contains(&t) && clean.len() < 12 {
            clean.push(t);
        }
    }
    let mut all = load_tags(paths).await;
    if clean.is_empty() {
        all.remove(key);
    } else {
        all.insert(key.to_string(), clean.clone());
    }
    super::paths::write_json(&tags_file(paths), &all).await?;
    Ok(clean)
}

async fn forget_tags(paths: &Paths, keys: &[String]) -> Result<()> {
    let mut all = load_tags(paths).await;
    let before = all.len();
    for k in keys {
        all.remove(k);
    }
    if all.len() != before {
        super::paths::write_json(&tags_file(paths), &all).await?;
    }
    Ok(())
}

// ---- actions ----

/// Copy the picture itself (not the file) to the clipboard.
pub async fn copy_image(path: PathBuf) -> Result<()> {
    let img = tokio::task::spawn_blocking(move || image::open(&path).map(|i| i.to_rgba8()))
        .await
        .map_err(|e| Error::msg(e.to_string()))?
        .map_err(|e| Error::msg(format!("Couldn't read the screenshot: {e}")))?;
    let (w, h) = img.dimensions();
    let data = arboard::ImageData { width: w as usize, height: h as usize, bytes: img.into_raw().into() };
    #[cfg(target_os = "linux")]
    {
        // On Linux the picture is only on the clipboard while something holds
        // it, so a thread keeps it there until another copy replaces it.
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            use arboard::SetExtLinux;
            match arboard::Clipboard::new() {
                Ok(mut cb) => {
                    let _ = tx.send(Ok(()));
                    let _ = cb.set().wait().image(data);
                }
                Err(e) => {
                    let _ = tx.send(Err(e.to_string()));
                }
            }
        });
        return rx.recv().map_err(|e| Error::msg(e.to_string()))?.map_err(|e| Error::msg(format!("Couldn't copy: {e}")));
    }
    #[cfg(not(target_os = "linux"))]
    tokio::task::spawn_blocking(move || {
        arboard::Clipboard::new().and_then(|mut cb| cb.set_image(data)).map_err(|e| Error::msg(format!("Couldn't copy: {e}")))
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))?
}

/// Save several screenshots into one zip.
pub async fn export_zip(files: Vec<PathBuf>, to: PathBuf) -> Result<usize> {
    tokio::task::spawn_blocking(move || -> Result<usize> {
        let out = std::fs::File::create(&to)?;
        let mut zip = zip::ZipWriter::new(out);
        // Pictures are already compressed: store them as they are.
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let mut used = std::collections::HashSet::new();
        for f in &files {
            let base = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "screenshot.png".into());
            let mut name = base.clone();
            let mut n = 2;
            while !used.insert(name.clone()) {
                name = format!("{n}-{base}");
                n += 1;
            }
            zip.start_file(name, opts).map_err(|e| Error::msg(e.to_string()))?;
            std::io::copy(&mut std::fs::File::open(f)?, &mut zip)?;
        }
        zip.finish().map_err(|e| Error::msg(e.to_string()))?;
        Ok(files.len())
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))?
}

/// Move screenshots to the Recycle Bin / Trash (never deleted outright).
pub async fn trash(paths: &Paths, items: Vec<(String, String)>) -> Result<usize> {
    let mut files = Vec::new();
    let mut keys = Vec::new();
    for (id, file) in &items {
        files.push(checked(paths, id, file)?);
        keys.push(format!("{id}/{file}"));
    }
    let n = files.len();
    tokio::task::spawn_blocking(move || trash::delete_all(&files))
        .await
        .map_err(|e| Error::msg(e.to_string()))?
        .map_err(|e| Error::msg(format!("Couldn't move to the bin: {e}")))?;
    forget_tags(paths, &keys).await?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_time_from_minecrafts_names() {
        let t = taken_from_name("2026-11-22_14.03.12.png").unwrap();
        assert!(t.starts_with("2026-11-22T14:03:12"), "{t}");
        assert!(taken_from_name("2026-11-22_14.03.12_2.png").is_some());
        assert!(taken_from_name("my picture.png").is_none());
    }

    #[test]
    fn only_images_count() {
        assert!(is_image("a.PNG") && is_image("b.jpg"));
        assert!(!is_image("notes.txt"));
    }
}
