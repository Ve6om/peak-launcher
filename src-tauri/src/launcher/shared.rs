//! Default game settings and shared packs.
//!
//! Default keybinds and game settings are saved once from an instance (its
//! options.txt) and put into every instance that has them switched on, each
//! time it launches. Nothing goes back the other way, so playing one
//! instance never changes another.
//!
//! Resource and shader packs are shared instead: the shared copy lives in
//! `shared/` and is synced into an instance when it launches and back out
//! when the game closes. Packs are hard-linked where the disk allows it, so
//! they take no extra space. Deleting a pack in one instance removes it from
//! the shared folder too (each instance remembers what it last synced, to
//! tell a deletion apart from a new pack).

use super::paths::Paths;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Shared {
    /// Use the default keybinds.
    pub keybinds: bool,
    /// Use the default game settings: everything else in options.txt
    /// (video, sound, GUI scale, chat...).
    pub options: bool,
    pub resourcepacks: bool,
    pub shaderpacks: bool,
}

impl Shared {
    pub fn any(&self) -> bool {
        self.keybinds || self.options || self.resourcepacks || self.shaderpacks
    }
}

/// Lines of options.txt that belong to one game and aren't part of the defaults.
const NOT_DEFAULTS: &[&str] = &["version", "resourcePacks", "incompatibleResourcePacks", "lastServer"];
const MANIFEST: &str = ".peak-shared.json";

pub fn dir(paths: &Paths) -> PathBuf {
    paths.root.join("shared")
}

fn defaults_file(paths: &Paths) -> PathBuf {
    paths.root.join("default-options.txt")
}

fn defaults_meta(paths: &Paths) -> PathBuf {
    paths.root.join("default-options.json")
}

/// What's saved as the defaults, for the Settings page.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DefaultsInfo {
    /// The instance they were saved from.
    pub from: String,
    pub saved_at: String,
    pub keybinds: usize,
    pub settings: usize,
    pub fov: Option<u32>,
    pub render_distance: Option<u32>,
    pub gui_scale: Option<String>,
    /// Changed in Peak since (or instead of) being copied from an instance.
    pub edited: bool,
    /// Keybinds left out when copying, because the instance stores keys as
    /// numbers (Minecraft 1.12 and older). Only set on the copy itself.
    #[serde(skip_serializing_if = "is_zero")]
    pub skipped_keybinds: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// A keybind value in the form Minecraft 1.13+ reads: "key.keyboard.w",
/// "key.mouse.left" (Forge adds ":NONE" and the like after it). Old versions
/// write numbers instead, which newer ones read as "not bound".
fn names_a_key(value: &str) -> bool {
    value.starts_with("key.keyboard.") || value.starts_with("key.mouse.")
}

/// Drop keybinds that aren't in the named form, so they can never unbind
/// every key in a newer instance. Returns how many went.
fn drop_numbered_keys(lines: &mut Vec<(String, String)>) -> usize {
    let before = lines.len();
    lines.retain(|(k, v)| !k.starts_with("key_") || names_a_key(v));
    before - lines.len()
}

fn applies(key: &str, s: &Shared) -> bool {
    if NOT_DEFAULTS.contains(&key) {
        return false;
    }
    if key.starts_with("key_") { s.keybinds } else { s.options }
}

fn parse(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn render(lines: &[(String, String)]) -> String {
    lines.iter().map(|(k, v)| format!("{k}:{v}\n")).collect()
}

/// Copy the chosen lines from `from` into `into`: lines already there are
/// changed in place, new ones go on the end. Everything else is left alone.
fn merge(into: &mut Vec<(String, String)>, from: &[(String, String)], keep: impl Fn(&str) -> bool) {
    let wanted: HashMap<&str, &str> = from.iter().filter(|(k, _)| keep(k)).map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let mut done = BTreeSet::new();
    for (k, v) in into.iter_mut() {
        if let Some(new) = wanted.get(k.as_str()) {
            *v = new.to_string();
            done.insert(k.clone());
        }
    }
    for (k, v) in from {
        if keep(k) && !done.contains(k) {
            into.push((k.clone(), v.clone()));
            done.insert(k.clone());
        }
    }
}

fn read_options(path: &Path) -> Option<Vec<(String, String)>> {
    std::fs::read_to_string(path).ok().map(|t| parse(&t))
}

/// Minecraft 1.12 and older store keys as numbers, not names, so default
/// keybinds (saved as names) are left out for them.
pub fn names_keys(game_version: &str) -> bool {
    let mut parts = game_version.split('.').map(|p| p.parse::<u32>().ok());
    match (parts.next().flatten(), parts.next().flatten()) {
        (Some(1), Some(minor)) => minor >= 13,
        (Some(major), _) => major > 1,
        // Betas, alphas and the like.
        _ => false,
    }
}

/// Put the defaults (the parts switched on) into an instance.
fn defaults_in(paths: &Paths, game: &Path, s: &Shared, game_version: &str) -> Result<()> {
    let Some(mut defaults) = read_options(&defaults_file(paths)) else { return Ok(()) };
    // Defaults saved before this was checked may still hold numbered keys.
    drop_numbered_keys(&mut defaults);
    let s = &Shared { keybinds: s.keybinds && names_keys(game_version), ..*s };
    if !s.keybinds && !s.options {
        return Ok(());
    }
    let target = game.join("options.txt");
    let mut mine = match read_options(&target) {
        Some(lines) => lines,
        // A new instance: take the version line too, so Minecraft reads the
        // keybinds in the format they were saved in.
        None => defaults.iter().filter(|(k, _)| k == "version").cloned().collect(),
    };
    merge(&mut mine, &defaults, |k| applies(k, s));
    std::fs::create_dir_all(game)?;
    std::fs::write(target, render(&mine))?;
    Ok(())
}

/// Save an instance's keybinds and game settings as the defaults.
pub fn save_defaults(paths: &Paths, game: &Path, from: &str) -> Result<DefaultsInfo> {
    let mine = read_options(&game.join("options.txt"))
        .filter(|l| !l.is_empty())
        .ok_or_else(|| Error::msg("This instance has no settings yet. Play it once, set things up, then save."))?;
    let all = Shared { keybinds: true, options: true, ..Default::default() };
    let mut lines: Vec<(String, String)> = mine.into_iter().filter(|(k, _)| k == "version" || applies(k, &all)).collect();
    let skipped = drop_numbered_keys(&mut lines);
    std::fs::create_dir_all(&paths.root)?;
    std::fs::write(defaults_file(paths), render(&lines))?;
    let info = DefaultsInfo { from: from.to_string(), saved_at: chrono::Utc::now().to_rfc3339(), ..describe(&lines) };
    std::fs::write(defaults_meta(paths), serde_json::to_vec_pretty(&info)?)?;
    Ok(DefaultsInfo { skipped_keybinds: skipped, ..info })
}

fn describe(lines: &[(String, String)]) -> DefaultsInfo {
    let get = |key: &str| lines.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    let keybinds = lines.iter().filter(|(k, _)| k.starts_with("key_")).count();
    DefaultsInfo {
        keybinds,
        settings: lines.iter().filter(|(k, _)| k != "version").count() - keybinds,
        // Saved as -1..1 around the default of 70.
        fov: get("fov").and_then(|v| v.parse::<f32>().ok()).map(|f| (70.0 + f * 40.0).round() as u32),
        render_distance: get("renderDistance").and_then(|v| v.parse().ok()),
        gui_scale: get("guiScale").map(|v| if v == "0" { "Auto".into() } else { v }),
        ..Default::default()
    }
}

/// The saved defaults, if there are any.
pub fn defaults_info(paths: &Paths) -> Option<DefaultsInfo> {
    let lines = read_options(&defaults_file(paths))?;
    let meta: DefaultsInfo = std::fs::read(defaults_meta(paths)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    Some(DefaultsInfo { from: meta.from, saved_at: meta.saved_at, edited: meta.edited, ..describe(&lines) })
}

/// Every line of the saved defaults, for editing in Peak.
pub fn defaults_lines(paths: &Paths) -> Vec<(String, String)> {
    read_options(&defaults_file(paths)).unwrap_or_default()
}

/// Replace the defaults with lines edited in Peak.
pub fn set_defaults(paths: &Paths, lines: Vec<(String, String)>) -> Result<DefaultsInfo> {
    let all = Shared { keybinds: true, options: true, ..Default::default() };
    let mut clean: Vec<(String, String)> = Vec::new();
    for (k, v) in lines {
        let k = k.trim().to_string();
        if k.is_empty() || k.contains([':', '\n', '\r']) || v.contains(['\n', '\r']) {
            return Err(Error::msg(format!("\"{k}\" isn't a valid setting")));
        }
        let numbered_key = k.starts_with("key_") && !names_a_key(&v);
        if (k == "version" || applies(&k, &all)) && !numbered_key && !clean.iter().any(|(x, _)| *x == k) {
            clean.push((k, v));
        }
    }
    if clean.iter().all(|(k, _)| k == "version") {
        clear_defaults(paths)?;
        return Ok(DefaultsInfo::default());
    }
    std::fs::create_dir_all(&paths.root)?;
    std::fs::write(defaults_file(paths), render(&clean))?;
    let old = defaults_info(paths).unwrap_or_default();
    let info = DefaultsInfo { from: old.from, saved_at: chrono::Utc::now().to_rfc3339(), edited: true, ..describe(&clean) };
    std::fs::write(defaults_meta(paths), serde_json::to_vec_pretty(&info)?)?;
    Ok(info)
}

pub fn clear_defaults(paths: &Paths) -> Result<()> {
    for f in [defaults_file(paths), defaults_meta(paths)] {
        match std::fs::remove_file(f) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
    }
    Ok(())
}

/// Put an instance's keys back to Minecraft's own (mods' keys too): the
/// keybind lines are taken out of options.txt and the game fills them in
/// next time it starts. The old file is kept beside it, just in case.
/// Returns how many keybinds were reset.
pub fn reset_keybinds(game: &Path) -> Result<usize> {
    let target = game.join("options.txt");
    let Some(mut lines) = read_options(&target) else { return Ok(0) };
    let before = lines.len();
    lines.retain(|(k, _)| !k.starts_with("key_"));
    let gone = before - lines.len();
    if gone > 0 {
        std::fs::copy(&target, game.join("options-before-key-reset.txt"))?;
        std::fs::write(&target, render(&lines))?;
    }
    Ok(gone)
}

fn names(dir: &Path) -> BTreeSet<String> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.'))
        .collect()
}

/// Hard link where possible (no extra space), else copy. Folders (unzipped
/// packs) are copied file by file the same way.
fn link_or_copy(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for e in std::fs::read_dir(from)? {
            let e = e?;
            link_or_copy(&e.path(), &to.join(e.file_name()))?;
        }
        return Ok(());
    }
    if std::fs::hard_link(from, to).is_err() {
        std::fs::copy(from, to)?;
    }
    Ok(())
}

fn delete(path: &Path) {
    // To the bin, in case it wasn't meant (tests skip the bin).
    if cfg!(test) || trash::delete(path).is_err() {
        let _ = if path.is_dir() { std::fs::remove_dir_all(path) } else { std::fs::remove_file(path) };
    }
}

/// Two-way sync of one pack folder against what was last synced.
fn sync_folder(mine: &Path, shared: &Path, last: &BTreeSet<String>) -> Result<BTreeSet<String>> {
    std::fs::create_dir_all(mine)?;
    std::fs::create_dir_all(shared)?;
    let (here, there) = (names(mine), names(shared));
    let mut now = BTreeSet::new();
    for n in here.union(&there) {
        match (here.contains(n), there.contains(n), last.contains(n)) {
            (true, true, _) => {
                now.insert(n.clone());
            }
            // Removed from the shared folder (by another instance) since.
            (true, false, true) => delete(&mine.join(n)),
            (true, false, false) => {
                link_or_copy(&mine.join(n), &shared.join(n))?;
                now.insert(n.clone());
            }
            // Removed from this instance since.
            (false, true, true) => delete(&shared.join(n)),
            (false, true, false) => {
                link_or_copy(&shared.join(n), &mine.join(n))?;
                now.insert(n.clone());
            }
            (false, false, _) => {}
        }
    }
    Ok(now)
}

#[derive(Default, Serialize, Deserialize)]
struct Manifest {
    #[serde(default)]
    folders: HashMap<String, BTreeSet<String>>,
}

fn packs(paths: &Paths, game: &Path, s: &Shared) -> Result<()> {
    let file = game.join(MANIFEST);
    let mut manifest: Manifest = std::fs::read(&file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    for (folder, on) in [("resourcepacks", s.resourcepacks), ("shaderpacks", s.shaderpacks)] {
        if on {
            let last = manifest.folders.remove(folder).unwrap_or_default();
            let now = sync_folder(&game.join(folder), &dir(paths).join(folder), &last)?;
            manifest.folders.insert(folder.into(), now);
        } else {
            // Turned off: start fresh (a plain merge) if it's turned on again.
            manifest.folders.remove(folder);
        }
    }
    std::fs::create_dir_all(game)?;
    std::fs::write(file, serde_json::to_vec(&manifest)?)?;
    Ok(())
}

/// Before the game starts: put in the defaults and the shared packs.
pub fn before_launch(paths: &Paths, game: &Path, s: &Shared, game_version: &str) -> Result<()> {
    if !s.any() {
        return Ok(());
    }
    if s.keybinds || s.options {
        defaults_in(paths, game, s, game_version)?;
    }
    packs(paths, game, s)
}

/// After the game closes: pass pack changes on to the shared folder.
/// (Defaults only ever go into instances, never back out.)
pub fn after_exit(paths: &Paths, game: &Path, s: &Shared) -> Result<()> {
    if !(s.resourcepacks || s.shaderpacks) {
        return Ok(());
    }
    packs(paths, game, s)
}

/// When defaults or sharing are switched on for an instance that isn't
/// running: apply them now so it shows straight away.
pub fn turned_on(paths: &Paths, game: &Path, s: &Shared, game_version: &str) -> Result<()> {
    before_launch(paths, game, s, game_version)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("peak-shared-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn defaults_go_into_instances_and_never_back() {
        let root = tmp("opts");
        let paths = Paths::new(root.clone());
        let (a, b) = (root.join("a"), root.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join("options.txt"), "version:3955\nkey_key.jump:key.keyboard.space\nfov:0.5\nrenderDistance:16\nguiScale:0\nresourcePacks:[\"vanilla\"]\n").unwrap();
        std::fs::write(b.join("options.txt"), "version:3465\nkey_key.jump:key.keyboard.j\nfov:0.0\nresourcePacks:[\"x\"]\n").unwrap();

        let info = save_defaults(&paths, &a, "A").unwrap();
        assert_eq!((info.keybinds, info.fov, info.render_distance, info.gui_scale.as_deref()), (1, Some(90), Some(16), Some("Auto")));
        assert!(!std::fs::read_to_string(defaults_file(&paths)).unwrap().contains("resourcePacks"));

        let only_keys = Shared { keybinds: true, ..Default::default() };
        before_launch(&paths, &b, &only_keys, "1.21.4").unwrap();
        let b_opts = std::fs::read_to_string(b.join("options.txt")).unwrap();
        assert!(b_opts.contains("key_key.jump:key.keyboard.space"));
        assert!(b_opts.contains("fov:0.0"), "game settings left alone: {b_opts}");
        assert!(b_opts.contains("version:3465"), "keeps its own version");
        assert!(b_opts.contains("resourcePacks:[\"x\"]"));

        // Changing b afterwards doesn't touch the defaults.
        std::fs::write(b.join("options.txt"), "key_key.jump:key.keyboard.k\n").unwrap();
        after_exit(&paths, &b, &only_keys).unwrap();
        assert!(std::fs::read_to_string(defaults_file(&paths)).unwrap().contains("key.keyboard.space"));

        // A brand-new instance gets the version line with the keybinds.
        let c = root.join("c");
        before_launch(&paths, &c, &only_keys, "1.21.4").unwrap();
        assert!(std::fs::read_to_string(c.join("options.txt")).unwrap().starts_with("version:3955\n"));

        assert_eq!(defaults_info(&paths).unwrap().from, "A");

        // Old versions store keys as numbers: default keybinds stay out.
        let old = root.join("old");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("options.txt"), "key_key.jump:57\n").unwrap();
        before_launch(&paths, &old, &only_keys, "1.12.2").unwrap();
        assert_eq!(std::fs::read_to_string(old.join("options.txt")).unwrap(), "key_key.jump:57\n");

        // Copied from an old instance: its numbered keys stay out of the
        // defaults, and old defaults that have them don't unbind anything.
        std::fs::write(old.join("options.txt"), "key_key.jump:57\nkey_key.drop:16\nfov:0.5\n").unwrap();
        let info = save_defaults(&paths, &old, "Old").unwrap();
        assert_eq!((info.keybinds, info.skipped_keybinds), (0, 2));
        std::fs::write(defaults_file(&paths), "key_key.jump:57\nkey_key.drop:key.keyboard.q\n").unwrap();
        std::fs::write(b.join("options.txt"), "key_key.jump:key.keyboard.space\n").unwrap();
        before_launch(&paths, &b, &only_keys, "1.21.4").unwrap();
        let b_opts = std::fs::read_to_string(b.join("options.txt")).unwrap();
        assert!(b_opts.contains("key_key.jump:key.keyboard.space") && b_opts.contains("key_key.drop:key.keyboard.q"), "{b_opts}");
        let info = set_defaults(&paths, vec![("key_key.jump".into(), "57".into()), ("fov".into(), "0.0".into())]).unwrap();
        assert_eq!(info.keybinds, 0);

        // Resetting an instance's keys takes the key lines out, keeping a copy.
        assert_eq!(reset_keybinds(&b).unwrap(), 2);
        assert!(!std::fs::read_to_string(b.join("options.txt")).unwrap().contains("key_"));
        assert!(b.join("options-before-key-reset.txt").exists());

        // Edited in Peak: bad lines refused, game-only lines dropped.
        assert!(set_defaults(&paths, vec![("bad:key".into(), "1".into())]).is_err());
        let info = set_defaults(&paths, vec![("fov".into(), "0.25".into()), ("lastServer".into(), "x".into())]).unwrap();
        assert!(info.edited && info.fov == Some(80));
        assert_eq!(defaults_lines(&paths), vec![("fov".to_string(), "0.25".to_string())]);
        clear_defaults(&paths).unwrap();
        assert!(defaults_info(&paths).is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn packs_sync_both_ways_and_deletions_carry() {
        let root = tmp("packs");
        let paths = Paths::new(root.clone());
        let (a, b) = (root.join("a"), root.join("b"));
        let s = Shared { resourcepacks: true, ..Default::default() };
        std::fs::create_dir_all(a.join("resourcepacks")).unwrap();
        std::fs::write(a.join("resourcepacks/faithful.zip"), b"pack").unwrap();

        before_launch(&paths, &a, &s, "1.21.4").unwrap(); // a's pack joins the shared folder
        before_launch(&paths, &b, &s, "1.21.4").unwrap(); // and reaches b
        assert!(b.join("resourcepacks/faithful.zip").exists());

        // Deleted in b: gone from the shared folder, then from a.
        std::fs::remove_file(b.join("resourcepacks/faithful.zip")).unwrap();
        after_exit(&paths, &b, &s).unwrap();
        assert!(!dir(&paths).join("resourcepacks/faithful.zip").exists());
        before_launch(&paths, &a, &s, "1.21.4").unwrap();
        assert!(!a.join("resourcepacks/faithful.zip").exists());

        // Shader packs weren't turned on, so they're left alone.
        assert!(!dir(&paths).join("shaderpacks").exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
