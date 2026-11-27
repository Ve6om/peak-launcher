//! An instance's single-player worlds: what each one is (from its level.dat),
//! renaming, copying and deleting them, their data packs, and backups.
//!
//! Backups are zip files under the instance's `backups/worlds/<world>/`.
//! Automatic ones (made when the game closes) are named `...-auto.zip` and
//! only the newest few are kept; ones made by hand are never removed by Peak.

use super::paths::Paths;
use crate::error::{Error, Result};
use base64::Engine;
use serde::Serialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

// ---------------------------------------------------------------- NBT

/// A tag from a level.dat, kept exactly so it can be written back unchanged.
/// Names and strings stay as raw (Java's modified UTF-8) bytes.
#[derive(Debug, Clone, PartialEq)]
enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Bytes(Vec<u8>),
    Str(Vec<u8>),
    List(u8, Vec<Tag>),
    Compound(Vec<(Vec<u8>, Tag)>),
    Ints(Vec<i32>),
    Longs(Vec<i64>),
}

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.b.get(self.p..self.p.checked_add(n)?)?;
        self.p += n;
        Some(s)
    }
    fn arr<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }
    fn len(&mut self) -> Option<usize> {
        let n = i32::from_be_bytes(self.arr()?);
        // A count no file could hold is a broken file.
        (0..=self.b.len() as i32).contains(&n).then_some(n as usize)
    }
    fn string(&mut self) -> Option<Vec<u8>> {
        let n = u16::from_be_bytes(self.arr()?) as usize;
        Some(self.take(n)?.to_vec())
    }
    fn tag(&mut self, id: u8, depth: u8) -> Option<Tag> {
        if depth > 64 {
            return None;
        }
        Some(match id {
            1 => Tag::Byte(i8::from_be_bytes(self.arr()?)),
            2 => Tag::Short(i16::from_be_bytes(self.arr()?)),
            3 => Tag::Int(i32::from_be_bytes(self.arr()?)),
            4 => Tag::Long(i64::from_be_bytes(self.arr()?)),
            5 => Tag::Float(f32::from_be_bytes(self.arr()?)),
            6 => Tag::Double(f64::from_be_bytes(self.arr()?)),
            7 => {
                let n = self.len()?;
                Tag::Bytes(self.take(n)?.to_vec())
            }
            8 => Tag::Str(self.string()?),
            9 => {
                let item = self.take(1)?[0];
                let n = self.len()?;
                let mut v = Vec::with_capacity(n.min(4096));
                for _ in 0..n {
                    v.push(self.tag(item, depth + 1)?);
                }
                Tag::List(item, v)
            }
            10 => {
                let mut v = Vec::new();
                loop {
                    let t = self.take(1)?[0];
                    if t == 0 {
                        break;
                    }
                    let name = self.string()?;
                    v.push((name, self.tag(t, depth + 1)?));
                }
                Tag::Compound(v)
            }
            11 => {
                let n = self.len()?;
                Tag::Ints((0..n).map(|_| self.arr().map(i32::from_be_bytes)).collect::<Option<_>>()?)
            }
            12 => {
                let n = self.len()?;
                Tag::Longs((0..n).map(|_| self.arr().map(i64::from_be_bytes)).collect::<Option<_>>()?)
            }
            _ => return None,
        })
    }
}

impl Tag {
    fn id(&self) -> u8 {
        match self {
            Tag::Byte(_) => 1,
            Tag::Short(_) => 2,
            Tag::Int(_) => 3,
            Tag::Long(_) => 4,
            Tag::Float(_) => 5,
            Tag::Double(_) => 6,
            Tag::Bytes(_) => 7,
            Tag::Str(_) => 8,
            Tag::List(..) => 9,
            Tag::Compound(_) => 10,
            Tag::Ints(_) => 11,
            Tag::Longs(_) => 12,
        }
    }
    fn write(&self, out: &mut Vec<u8>) {
        let string = |out: &mut Vec<u8>, s: &[u8]| {
            out.extend((s.len() as u16).to_be_bytes());
            out.extend(s);
        };
        match self {
            Tag::Byte(v) => out.extend(v.to_be_bytes()),
            Tag::Short(v) => out.extend(v.to_be_bytes()),
            Tag::Int(v) => out.extend(v.to_be_bytes()),
            Tag::Long(v) => out.extend(v.to_be_bytes()),
            Tag::Float(v) => out.extend(v.to_be_bytes()),
            Tag::Double(v) => out.extend(v.to_be_bytes()),
            Tag::Bytes(v) => {
                out.extend((v.len() as i32).to_be_bytes());
                out.extend(v);
            }
            Tag::Str(s) => string(out, s),
            Tag::List(item, v) => {
                out.push(if v.is_empty() { *item } else { v[0].id() });
                out.extend((v.len() as i32).to_be_bytes());
                for t in v {
                    t.write(out);
                }
            }
            Tag::Compound(v) => {
                for (name, t) in v {
                    out.push(t.id());
                    string(out, name);
                    t.write(out);
                }
                out.push(0);
            }
            Tag::Ints(v) => {
                out.extend((v.len() as i32).to_be_bytes());
                v.iter().for_each(|x| out.extend(x.to_be_bytes()));
            }
            Tag::Longs(v) => {
                out.extend((v.len() as i32).to_be_bytes());
                v.iter().for_each(|x| out.extend(x.to_be_bytes()));
            }
        }
    }
    fn get(&self, name: &str) -> Option<&Tag> {
        match self {
            Tag::Compound(v) => v.iter().find(|(n, _)| n == name.as_bytes()).map(|(_, t)| t),
            _ => None,
        }
    }
    fn get_mut(&mut self, name: &str) -> Option<&mut Tag> {
        match self {
            Tag::Compound(v) => v.iter_mut().find(|(n, _)| n == name.as_bytes()).map(|(_, t)| t),
            _ => None,
        }
    }
    fn path(&self, names: &[&str]) -> Option<&Tag> {
        names.iter().try_fold(self, |t, n| t.get(n))
    }
    fn text(&self) -> Option<String> {
        match self {
            Tag::Str(s) => Some(String::from_utf8_lossy(s).into_owned()),
            _ => None,
        }
    }
    fn int(&self) -> Option<i64> {
        match *self {
            Tag::Byte(v) => Some(v as i64),
            Tag::Short(v) => Some(v as i64),
            Tag::Int(v) => Some(v as i64),
            Tag::Long(v) => Some(v),
            _ => None,
        }
    }
}

/// A whole NBT file: the root compound's name and the compound.
struct Nbt {
    name: Vec<u8>,
    root: Tag,
}

fn parse(bytes: &[u8]) -> Option<Nbt> {
    let mut r = Reader { b: bytes, p: 0 };
    if r.take(1)?[0] != 10 {
        return None;
    }
    let name = r.string()?;
    Some(Nbt { name, root: r.tag(10, 0)? })
}

fn render(nbt: &Nbt) -> Vec<u8> {
    let mut out = vec![10];
    out.extend((nbt.name.len() as u16).to_be_bytes());
    out.extend(&nbt.name);
    nbt.root.write(&mut out);
    out
}

fn read_level(world: &Path) -> Option<Nbt> {
    let file = std::fs::File::open(world.join("level.dat")).ok()?;
    let mut bytes = Vec::new();
    // level.dat is gzipped; cap it so a broken file can't eat memory.
    flate2::read::GzDecoder::new(file).take(64 << 20).read_to_end(&mut bytes).ok()?;
    parse(&bytes)
}

fn write_level(world: &Path, nbt: &Nbt) -> Result<()> {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&render(nbt))?;
    let bytes = gz.finish()?;
    // Minecraft keeps the last good copy as level.dat_old; do the same.
    let path = world.join("level.dat");
    let _ = std::fs::copy(&path, world.join("level.dat_old"));
    let tmp = world.join("level.dat.peak-tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

// ---------------------------------------------------------------- worlds

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct World {
    /// The folder under saves/, which is how the game and Peak name it.
    pub folder: String,
    /// The name shown in the game.
    pub name: String,
    /// "survival", "creative", "adventure", "spectator".
    pub mode: Option<String>,
    pub hardcore: bool,
    pub cheats: bool,
    /// When it was last played (ms since 1970).
    pub last_played: Option<i64>,
    /// As text: it doesn't fit in a JavaScript number.
    pub seed: Option<String>,
    /// The Minecraft version it was last played in.
    pub version: Option<String>,
    pub size: u64,
    /// icon.png as a data: URL.
    pub icon: Option<String>,
    pub datapacks: usize,
    pub backups: usize,
}

fn saves(paths: &Paths, inst: &str) -> PathBuf {
    super::instances::game_dir(paths, inst).join("saves")
}

/// The folder for a world, checked to be a plain name inside saves/.
fn world_dir(paths: &Paths, inst: &str, folder: &str) -> Result<PathBuf> {
    if folder.is_empty() || folder.contains(['/', '\\']) || folder == "." || folder == ".." {
        return Err(Error::msg("That isn't a world in this instance."));
    }
    let dir = saves(paths, inst).join(folder);
    if !dir.join("level.dat").is_file() {
        return Err(Error::msg("That world isn't there any more."));
    }
    Ok(dir)
}

fn dir_size(dir: &Path) -> u64 {
    walkdir(dir).filter_map(|p| p.metadata().ok()).filter(|m| m.is_file()).map(|m| m.len()).sum()
}

/// Every file and folder under `dir` (not following links).
fn walkdir(dir: &Path) -> impl Iterator<Item = PathBuf> {
    let mut todo = vec![dir.to_path_buf()];
    let mut out = Vec::new();
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                todo.push(e.path());
            }
            if !ft.is_symlink() {
                out.push(e.path());
            }
        }
    }
    out.into_iter()
}

const MODES: [&str; 4] = ["survival", "creative", "adventure", "spectator"];

fn describe(paths: &Paths, inst: &str, folder: &str, dir: &Path) -> World {
    let level = read_level(dir);
    let data = level.as_ref().and_then(|l| l.root.get("Data"));
    let get = |p: &[&str]| data.and_then(|d| d.path(p));
    let seed = get(&["WorldGenSettings", "seed"]).or_else(|| get(&["RandomSeed"])).and_then(Tag::int);
    let icon = std::fs::read(dir.join("icon.png"))
        .ok()
        .filter(|b| b.len() < 512 * 1024)
        .map(|b| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b)));
    World {
        folder: folder.to_string(),
        name: get(&["LevelName"]).and_then(Tag::text).filter(|n| !n.trim().is_empty()).unwrap_or_else(|| folder.to_string()),
        mode: get(&["GameType"]).and_then(Tag::int).and_then(|m| MODES.get(m as usize)).map(|m| m.to_string()),
        hardcore: get(&["hardcore"]).and_then(Tag::int).unwrap_or(0) != 0,
        cheats: get(&["allowCommands"]).and_then(Tag::int).unwrap_or(0) != 0,
        last_played: get(&["LastPlayed"]).and_then(Tag::int),
        seed: seed.map(|s| s.to_string()),
        version: get(&["Version", "Name"]).and_then(Tag::text),
        size: dir_size(dir),
        icon,
        datapacks: datapack_names(dir).len(),
        backups: list_backups(paths, inst, folder).len(),
    }
}

/// The instance's worlds, most recently played first.
pub fn list(paths: &Paths, inst: &str) -> Vec<World> {
    let mut out: Vec<World> = std::fs::read_dir(saves(paths, inst))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("level.dat").is_file())
        .map(|e| {
            let folder = e.file_name().to_string_lossy().into_owned();
            describe(paths, inst, &folder, &e.path())
        })
        .collect();
    out.sort_by(|a, b| b.last_played.cmp(&a.last_played).then_with(|| a.name.cmp(&b.name)));
    out
}

/// Change the name shown in the game (the folder stays the same).
pub fn rename(paths: &Paths, inst: &str, folder: &str, name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(Error::msg("Give the world a name (up to 100 characters)."));
    }
    let dir = world_dir(paths, inst, folder)?;
    let mut level = read_level(&dir).ok_or_else(|| Error::msg("This world's level.dat couldn't be read."))?;
    let data = level.root.get_mut("Data").ok_or_else(|| Error::msg("This world's level.dat couldn't be read."))?;
    match data.get_mut("LevelName") {
        Some(t) => *t = Tag::Str(name.as_bytes().to_vec()),
        None => {
            if let Tag::Compound(v) = data {
                v.push((b"LevelName".to_vec(), Tag::Str(name.as_bytes().to_vec())));
            }
        }
    }
    write_level(&dir, &level)
}

/// A folder name under `parent` that isn't taken: "name", "name (2)"...
fn free_name(parent: &Path, base: &str) -> String {
    let clean: String = base.chars().map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c }).collect();
    let clean = clean.trim().trim_end_matches('.').to_string();
    let clean = if clean.is_empty() { "World".to_string() } else { clean };
    (1..)
        .map(|i| if i == 1 { clean.clone() } else { format!("{clean} ({i})") })
        .find(|n| !parent.join(n).exists())
        .unwrap()
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let ft = e.file_type()?;
        let name = e.file_name();
        // The game's lock on an open world isn't part of the world.
        if name == "session.lock" || ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            copy_dir(&e.path(), &to.join(&name))?;
        } else {
            std::fs::copy(e.path(), to.join(&name))?;
        }
    }
    Ok(())
}

/// Copy a world. The copy is named "<name> (copy)" in the game. Returns its folder.
pub fn duplicate(paths: &Paths, inst: &str, folder: &str) -> Result<String> {
    let dir = world_dir(paths, inst, folder)?;
    let parent = saves(paths, inst);
    let new_folder = free_name(&parent, &format!("{folder} - Copy"));
    let to = parent.join(&new_folder);
    if let Err(e) = copy_dir(&dir, &to) {
        let _ = std::fs::remove_dir_all(&to);
        return Err(e);
    }
    let name = describe(paths, inst, folder, &dir).name;
    let _ = rename(paths, inst, &new_folder, &format!("{name} (copy)"));
    Ok(new_folder)
}

/// Send a world to the recycle bin. Its backups are kept.
pub fn delete(paths: &Paths, inst: &str, folder: &str) -> Result<()> {
    let dir = world_dir(paths, inst, folder)?;
    trash::delete(&dir).map_err(|e| Error::msg(format!("Couldn't move the world to the recycle bin: {e}")))
}

pub fn folder_path(paths: &Paths, inst: &str, folder: &str) -> Result<PathBuf> {
    world_dir(paths, inst, folder)
}

// ---------------------------------------------------------------- data packs

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Datapack {
    pub file_name: String,
    pub size: u64,
    pub folder: bool,
}

fn datapack_names(world: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(world.join("datapacks"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir() || e.file_name().to_string_lossy().to_lowercase().ends_with(".zip"))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    out.sort_by_key(|n| n.to_lowercase());
    out
}

pub fn datapacks(paths: &Paths, inst: &str, folder: &str) -> Result<Vec<Datapack>> {
    let dir = world_dir(paths, inst, folder)?.join("datapacks");
    Ok(datapack_names(&dir.parent().unwrap())
        .into_iter()
        .map(|n| {
            let p = dir.join(&n);
            Datapack { folder: p.is_dir(), size: if p.is_dir() { dir_size(&p) } else { p.metadata().map(|m| m.len()).unwrap_or(0) }, file_name: n }
        })
        .collect())
}

/// Copy a data pack (.zip) from anywhere into a world.
pub fn add_datapack(paths: &Paths, inst: &str, folder: &str, file: &Path) -> Result<String> {
    let dir = world_dir(paths, inst, folder)?.join("datapacks");
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if !name.to_lowercase().ends_with(".zip") || !file.is_file() {
        return Err(Error::msg("Data packs are .zip files."));
    }
    // A data pack has pack.mcmeta at its top level (or one folder down).
    let looks_right = std::fs::File::open(file)
        .ok()
        .and_then(|f| zip::ZipArchive::new(f).ok())
        .map(|z| z.file_names().any(|n| n == "pack.mcmeta" || n.matches('/').count() == 1 && n.ends_with("/pack.mcmeta")))
        .unwrap_or(false);
    if !looks_right {
        return Err(Error::msg(format!("{name} doesn't look like a data pack (it has no pack.mcmeta).")));
    }
    std::fs::create_dir_all(&dir)?;
    std::fs::copy(file, dir.join(&name))?;
    Ok(name)
}

pub fn remove_datapack(paths: &Paths, inst: &str, folder: &str, file_name: &str) -> Result<()> {
    if file_name.is_empty() || file_name.contains(['/', '\\']) || file_name.starts_with('.') {
        return Err(Error::msg("Invalid file name"));
    }
    let path = world_dir(paths, inst, folder)?.join("datapacks").join(file_name);
    if path.exists() {
        trash::delete(&path).map_err(|e| Error::msg(format!("Couldn't move it to the recycle bin: {e}")))?;
    }
    Ok(())
}

// ---------------------------------------------------------------- backups

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub file_name: String,
    /// ms since 1970.
    pub created: i64,
    pub size: u64,
    /// Made when the game closed (only the newest few are kept).
    pub auto: bool,
}

fn backup_dir(paths: &Paths, inst: &str, folder: &str) -> PathBuf {
    paths.instance(inst).join("backups").join("worlds").join(folder)
}

pub fn list_backups(paths: &Paths, inst: &str, folder: &str) -> Vec<Backup> {
    let mut out: Vec<Backup> = std::fs::read_dir(backup_dir(paths, inst, folder))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let file_name = e.file_name().to_string_lossy().into_owned();
            if !file_name.ends_with(".zip") {
                return None;
            }
            let meta = e.metadata().ok()?;
            let created = meta.modified().ok()?.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_millis() as i64;
            Some(Backup { auto: file_name.ends_with("-auto.zip"), file_name, created, size: meta.len() })
        })
        .collect();
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

fn zip_world(world: &Path, to: &Path) -> Result<()> {
    let tmp = to.with_extension("zip.part");
    let result = (|| -> Result<()> {
        let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(std::fs::File::create(&tmp)?));
        // Fast compression: world files (region data) are already compressed.
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).compression_level(Some(1));
        let mut files: Vec<PathBuf> = walkdir(world).filter(|p| p.is_file()).collect();
        files.sort();
        for p in files {
            let rel = p.strip_prefix(world).map_err(|_| Error::msg("Bad path"))?.to_string_lossy().replace('\\', "/");
            if rel == "session.lock" {
                continue;
            }
            // A file the open game is writing can't always be read; skip it
            // rather than fail the whole backup.
            let Ok(mut f) = std::fs::File::open(&p) else { continue };
            zip.start_file(rel, opts)?;
            std::io::copy(&mut f, &mut zip)?;
        }
        zip.finish()?.flush()?;
        Ok(())
    })();
    match result {
        Ok(()) => Ok(std::fs::rename(&tmp, to)?),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// Back up a world. `auto` backups beyond the newest `keep` are removed.
pub fn backup(paths: &Paths, inst: &str, folder: &str, auto: bool, keep: usize) -> Result<Backup> {
    let world = world_dir(paths, inst, folder)?;
    let dir = backup_dir(paths, inst, folder);
    std::fs::create_dir_all(&dir)?;
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    let mut file_name = format!("{stamp}{}.zip", if auto { "-auto" } else { "" });
    let mut n = 2;
    while dir.join(&file_name).exists() {
        file_name = format!("{stamp}_{n}{}.zip", if auto { "-auto" } else { "" });
        n += 1;
    }
    zip_world(&world, &dir.join(&file_name))?;
    if auto {
        for old in list_backups(paths, inst, folder).into_iter().filter(|b| b.auto).skip(keep.max(1)) {
            let _ = std::fs::remove_file(dir.join(old.file_name));
        }
    }
    list_backups(paths, inst, folder).into_iter().find(|b| b.file_name == file_name).ok_or_else(|| Error::msg("The backup went missing."))
}

fn backup_path(paths: &Paths, inst: &str, folder: &str, file_name: &str) -> Result<PathBuf> {
    if file_name.contains(['/', '\\']) || !file_name.ends_with(".zip") || folder.contains(['/', '\\']) || folder.starts_with('.') {
        return Err(Error::msg("That isn't one of this world's backups."));
    }
    let p = backup_dir(paths, inst, folder).join(file_name);
    if !p.is_file() {
        return Err(Error::msg("That backup isn't there any more."));
    }
    Ok(p)
}

/// Put a world back as it was in a backup. The world as it is now is
/// backed up first (so a restore can be undone), then replaced. A world that
/// was deleted since comes back too.
pub fn restore(paths: &Paths, inst: &str, folder: &str, file_name: &str) -> Result<()> {
    let zip_path = backup_path(paths, inst, folder, file_name)?;
    let target = saves(paths, inst).join(folder);
    if target.join("level.dat").is_file() {
        backup(paths, inst, folder, false, 0)?;
    }
    // Unpack beside it first, so a bad backup leaves the world alone.
    let staging = saves(paths, inst).join(format!(".{folder}.peak-restore"));
    let _ = std::fs::remove_dir_all(&staging);
    let unpacked = (|| -> Result<()> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&zip_path)?)?;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i)?;
            let Some(rel) = f.enclosed_name() else { continue };
            let out = staging.join(rel);
            if f.is_dir() {
                std::fs::create_dir_all(&out)?;
                continue;
            }
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::io::copy(&mut f, &mut std::fs::File::create(&out)?)?;
        }
        if !staging.join("level.dat").is_file() {
            return Err(Error::msg("That backup doesn't hold a world."));
        }
        Ok(())
    })();
    if let Err(e) = unpacked {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    if target.exists() {
        let old = saves(paths, inst).join(format!(".{folder}.peak-old"));
        let _ = std::fs::remove_dir_all(&old);
        std::fs::rename(&target, &old)?;
        if let Err(e) = std::fs::rename(&staging, &target) {
            let _ = std::fs::rename(&old, &target);
            return Err(e.into());
        }
        let _ = std::fs::remove_dir_all(&old);
    } else {
        std::fs::rename(&staging, &target)?;
    }
    Ok(())
}

pub fn delete_backup(paths: &Paths, inst: &str, folder: &str, file_name: &str) -> Result<()> {
    std::fs::remove_file(backup_path(paths, inst, folder, file_name)?)?;
    Ok(())
}

/// Worlds whose level.dat changed since `since` (played this session).
pub fn changed_since(paths: &Paths, inst: &str, since: SystemTime) -> Vec<String> {
    std::fs::read_dir(saves(paths, inst))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("level.dat").metadata().and_then(|m| m.modified()).map(|t| t > since).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(name: &str, seed: i64) -> Vec<u8> {
        let s = |v: &str| Tag::Str(v.as_bytes().to_vec());
        let data = Tag::Compound(vec![
            (b"LevelName".to_vec(), s(name)),
            (b"GameType".to_vec(), Tag::Int(1)),
            (b"hardcore".to_vec(), Tag::Byte(0)),
            (b"LastPlayed".to_vec(), Tag::Long(1_700_000_000_000)),
            (b"Version".to_vec(), Tag::Compound(vec![(b"Name".to_vec(), s("1.21.4"))])),
            (b"WorldGenSettings".to_vec(), Tag::Compound(vec![(b"seed".to_vec(), Tag::Long(seed))])),
            (b"Player".to_vec(), Tag::Compound(vec![(b"Pos".to_vec(), Tag::List(6, vec![Tag::Double(1.5), Tag::Double(64.0), Tag::Double(-3.0)]))])),
            (b"ints".to_vec(), Tag::Ints(vec![1, 2])),
            (b"empty".to_vec(), Tag::List(0, vec![])),
        ]);
        let nbt = Nbt { name: vec![], root: Tag::Compound(vec![(b"Data".to_vec(), data)]) };
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(&render(&nbt)).unwrap();
        gz.finish().unwrap()
    }

    fn setup(name: &str) -> (PathBuf, Paths) {
        let root = std::env::temp_dir().join(format!("peak-worlds-{name}-{}", uuid::Uuid::new_v4()));
        let paths = Paths::new(root.clone());
        let w = saves(&paths, "a").join("My World");
        std::fs::create_dir_all(w.join("region")).unwrap();
        std::fs::write(w.join("level.dat"), level("Home", -4_172_144_997_902_289_642)).unwrap();
        std::fs::write(w.join("region/r.0.0.mca"), vec![7u8; 5000]).unwrap();
        std::fs::write(w.join("session.lock"), b"x").unwrap();
        (root, paths)
    }

    #[test]
    fn reads_and_writes_level_dat() {
        let (root, paths) = setup("read");
        let worlds = list(&paths, "a");
        assert_eq!(worlds.len(), 1);
        let w = &worlds[0];
        assert_eq!((w.name.as_str(), w.mode.as_deref(), w.version.as_deref()), ("Home", Some("creative"), Some("1.21.4")));
        assert_eq!(w.seed.as_deref(), Some("-4172144997902289642"));

        let dir = saves(&paths, "a").join("My World");
        let before = read_level(&dir).unwrap();
        rename(&paths, "a", "My World", "Base").unwrap();
        let after = read_level(&dir).unwrap();
        assert_eq!(list(&paths, "a")[0].name, "Base");
        // Everything else is untouched.
        let strip = |n: &Nbt| {
            let mut r = n.root.clone();
            *r.get_mut("Data").unwrap().get_mut("LevelName").unwrap() = Tag::Str(vec![]);
            r
        };
        assert_eq!(strip(&before), strip(&after));
        assert!(dir.join("level.dat_old").exists());
        assert!(rename(&paths, "a", "../x", "y").is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn copies_worlds() {
        let (root, paths) = setup("copy");
        let new = duplicate(&paths, "a", "My World").unwrap();
        assert_eq!(new, "My World - Copy");
        let copy = saves(&paths, "a").join(&new);
        assert!(copy.join("region/r.0.0.mca").exists() && !copy.join("session.lock").exists());
        assert_eq!(list(&paths, "a").iter().find(|w| w.folder == new).unwrap().name, "Home (copy)");
        assert_eq!(duplicate(&paths, "a", "My World").unwrap(), "My World - Copy (2)");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn backs_up_and_restores() {
        let (root, paths) = setup("backup");
        let dir = saves(&paths, "a").join("My World");
        let b = backup(&paths, "a", "My World", false, 3).unwrap();
        assert!(!b.auto && b.size > 0);

        // Changed after the backup, then restored.
        std::fs::write(dir.join("region/r.0.0.mca"), b"griefed").unwrap();
        std::fs::write(dir.join("new.txt"), b"new").unwrap();
        restore(&paths, "a", "My World", &b.file_name).unwrap();
        assert_eq!(std::fs::read(dir.join("region/r.0.0.mca")).unwrap(), vec![7u8; 5000]);
        assert!(!dir.join("new.txt").exists());
        // The world as it was before the restore was kept.
        assert_eq!(list_backups(&paths, "a", "My World").len(), 2);

        // Only the newest automatic backups are kept; manual ones stay.
        for _ in 0..4 {
            backup(&paths, "a", "My World", true, 2).unwrap();
        }
        let all = list_backups(&paths, "a", "My World");
        assert_eq!((all.iter().filter(|b| b.auto).count(), all.iter().filter(|b| !b.auto).count()), (2, 2));

        // A deleted world comes back from its backup.
        std::fs::remove_dir_all(&dir).unwrap();
        restore(&paths, "a", "My World", &b.file_name).unwrap();
        assert!(dir.join("level.dat").exists());
        assert!(restore(&paths, "a", "My World", "../../x.zip").is_err());
        let _ = std::fs::remove_dir_all(root);
    }
}
