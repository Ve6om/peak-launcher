//! Export an instance as a modpack: Modrinth (.mrpack) or CurseForge (.zip).
//!
//! Mods, resource packs and shaders that exist on the target platform are
//! referenced by id/URL so the pack stays small. Anything else is bundled in
//! the pack's `overrides/` folder, and the report lists those files so the
//! author can check they are allowed to redistribute them.

use super::curseforge;
use super::instances::{self, Instance};
use super::modrinth::{self, Kind, Version};
use super::net;
use super::paths::Paths;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    /// "modrinth" | "curseforge"
    pub format: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub summary: String,
    /// Extra top-level files/folders from the game folder to bundle.
    #[serde(default)]
    pub include: Vec<String>,
    /// Where to write the pack.
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    pub path: String,
    /// Content referenced on the platform (downloaded by whoever installs the pack).
    pub linked: usize,
    /// Content files that had to be bundled because the platform doesn't have them.
    pub bundled: Vec<String>,
    /// Other bundled files (configs etc.).
    pub override_files: usize,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
    /// Suggested to include by default.
    pub suggested: bool,
}

/// Content folders are always handled (linked or bundled).
const CONTENT: &[(&str, Kind)] = &[("mods", Kind::Mod), ("resourcepacks", Kind::Resourcepack), ("shaderpacks", Kind::Shader)];
/// Never offered: generated, personal or launcher-only files.
const HIDDEN: &[&str] = &[
    "mods", "resourcepacks", "shaderpacks", "logs", "crash-reports", ".cache", "natives", "quickPlay", ".fabric",
    "debug", "downloads", "usercache.json", "usernamecache.json", "realms_persistence.json", "command_history.txt",
    ".mixin.out", "patchouli_data.json",
];
/// Ticked by default: configuration that makes the pack work as intended.
const SUGGESTED: &[&str] = &["config", "defaultconfigs", "kubejs", "scripts", "global_packs", "datapacks", "openloader", "paxi"];

/// Top-level entries of the instance's game folder the user can choose to bundle.
pub async fn entries(paths: &Paths, inst_id: &str) -> Result<Vec<Entry>> {
    let dir = instances::game_dir(paths, inst_id);
    let mut out = Vec::new();
    if let Ok(mut rd) = tokio::fs::read_dir(&dir).await {
        while let Some(e) = rd.next_entry().await? {
            let name = e.file_name().to_string_lossy().to_string();
            if HIDDEN.contains(&name.as_str()) {
                continue;
            }
            out.push(Entry { suggested: SUGGESTED.contains(&name.as_str()), is_dir: e.file_type().await?.is_dir(), name });
        }
    }
    out.sort_by(|a, b| b.suggested.cmp(&a.suggested).then(b.is_dir.cmp(&a.is_dir)).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(out)
}

/// An enabled content file in the instance.
struct ContentFile {
    rel: String, // e.g. "mods/sodium.jar"
    abs: PathBuf,
    sha1: String,
    size: u64,
    project_id: Option<String>,
    version_id: Option<String>,
}

async fn content_files(paths: &Paths, inst_id: &str) -> Result<Vec<ContentFile>> {
    let mut out = Vec::new();
    for (folder, kind) in CONTENT {
        for item in modrinth::list(paths, inst_id, *kind).await? {
            if !item.enabled {
                continue; // disabled content isn't part of the pack
            }
            out.push(ContentFile {
                rel: format!("{folder}/{}", item.file_name),
                abs: modrinth::content_dir(paths, inst_id, *kind).join(&item.file_name),
                sha1: item.sha1,
                size: item.size,
                project_id: item.project_id,
                version_id: item.version_id,
            });
        }
    }
    Ok(out)
}

fn safe_entry(name: &str) -> Result<()> {
    if name.is_empty() || name.contains(['/', '\\']) || name == ".." || name == "." {
        return Err(Error::msg(format!("Invalid entry '{name}'")));
    }
    Ok(())
}

/// Every file under the chosen top-level entries, as (relative path, absolute path).
fn override_files(game: &Path, include: &[String]) -> Result<Vec<(String, PathBuf)>> {
    fn walk(base: &Path, rel: &str, out: &mut Vec<(String, PathBuf)>) {
        let abs = base.join(rel);
        if abs.is_file() {
            out.push((rel.replace('\\', "/"), abs));
            return;
        }
        if let Ok(rd) = std::fs::read_dir(&abs) {
            for e in rd.flatten() {
                walk(base, &format!("{rel}/{}", e.file_name().to_string_lossy()), out);
            }
        }
    }
    let mut out = Vec::new();
    for name in include {
        safe_entry(name)?;
        if HIDDEN.contains(&name.as_str()) {
            continue;
        }
        walk(game, name, &mut out);
    }
    Ok(out)
}

/// The version string a loader uses inside pack files ("47.3.0" for Forge).
fn plain_loader_version(inst: &Instance) -> Option<String> {
    let v = inst.loader_version.clone()?;
    Some(match inst.loader.as_str() {
        "forge" => v.strip_prefix(&format!("{}-", inst.game_version)).unwrap_or(&v).to_string(),
        _ => v,
    })
}

fn write_zip(path: &Path, json_name: &str, json_body: &Value, overrides: &[(String, PathBuf)], prefix: &str) -> Result<u64> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("part");
    {
        let file = std::fs::File::create(&tmp)?;
        let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file(json_name, opts)?;
        zip.write_all(&serde_json::to_vec_pretty(json_body)?)?;
        for (rel, abs) in overrides {
            zip.start_file(format!("{prefix}/{rel}"), opts)?;
            let mut f = std::fs::File::open(abs)?;
            std::io::copy(&mut f, &mut zip)?;
        }
        zip.finish()?.flush()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(std::fs::metadata(path)?.len())
}

// ---------------------------------------------------------------- Modrinth (.mrpack)

async fn modrinth_lookup(hashes: &[String]) -> Result<HashMap<String, Version>> {
    if hashes.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(net::client()
        .post("https://api.modrinth.com/v2/version_files")
        .json(&json!({ "hashes": hashes, "algorithm": "sha1" }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

async fn export_modrinth(paths: &Paths, inst: &Instance, opts: &ExportOptions) -> Result<ExportReport> {
    let content = content_files(paths, &inst.id).await?;
    let found = modrinth_lookup(&content.iter().map(|c| c.sha1.clone()).collect::<Vec<_>>()).await?;

    let mut files = Vec::new();
    let mut bundled_content = Vec::new();
    for c in &content {
        let hit = found.get(&c.sha1).and_then(|v| v.files.iter().find(|f| f.hashes.get("sha1").map(|h| h.eq_ignore_ascii_case(&c.sha1)).unwrap_or(false)));
        match hit.and_then(|f| f.hashes.get("sha512").map(|s| (f, s))) {
            Some((f, sha512)) => files.push(json!({
                "path": c.rel,
                "hashes": { "sha1": c.sha1, "sha512": sha512 },
                "env": { "client": "required", "server": if c.rel.starts_with("mods/") { "required" } else { "unsupported" } },
                "downloads": [f.url],
                "fileSize": c.size,
            })),
            None => bundled_content.push((c.rel.clone(), c.abs.clone())),
        }
    }

    let mut deps = serde_json::Map::new();
    deps.insert("minecraft".into(), json!(inst.game_version));
    if let Some(v) = plain_loader_version(inst) {
        let key = match inst.loader.as_str() {
            "fabric" => "fabric-loader",
            "quilt" => "quilt-loader",
            "forge" => "forge",
            "neoforge" => "neoforge",
            _ => "",
        };
        if !key.is_empty() {
            deps.insert(key.into(), json!(v));
        }
    }
    let mut index = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": opts.version,
        "name": opts.name,
        "files": files,
        "dependencies": deps,
    });
    if !opts.summary.trim().is_empty() {
        index["summary"] = json!(opts.summary.trim());
    }

    let game = instances::game_dir(paths, &inst.id);
    let mut overrides = override_files(&game, &opts.include)?;
    let override_count = overrides.len();
    let bundled: Vec<String> = bundled_content.iter().map(|(r, _)| r.clone()).collect();
    overrides.extend(bundled_content);
    let linked = files.len();

    let out = PathBuf::from(&opts.path);
    let size = tokio::task::spawn_blocking(move || write_zip(&out, "modrinth.index.json", &index, &overrides, "overrides"))
        .await
        .map_err(|e| Error::msg(e.to_string()))??;
    Ok(ExportReport { path: opts.path.clone(), linked, bundled, override_files: override_count, size_bytes: size })
}

// ---------------------------------------------------------------- CurseForge (.zip)

/// CurseForge's file fingerprint: MurmurHash2 (seed 1) of the file with all
/// whitespace bytes (tab, LF, CR, space) removed.
pub fn cf_fingerprint(bytes: &[u8]) -> u32 {
    let data: Vec<u8> = bytes.iter().copied().filter(|b| !matches!(b, 9 | 10 | 13 | 32)).collect();
    murmur2(&data, 1)
}

fn murmur2(data: &[u8], seed: u32) -> u32 {
    const M: u32 = 0x5bd1_e995;
    let mut h = seed ^ data.len() as u32;
    let mut chunks = data.chunks_exact(4);
    for c in &mut chunks {
        let mut k = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        k = k.wrapping_mul(M);
        k ^= k >> 24;
        k = k.wrapping_mul(M);
        h = h.wrapping_mul(M) ^ k;
    }
    let rest = chunks.remainder();
    if !rest.is_empty() {
        if rest.len() >= 3 {
            h ^= (rest[2] as u32) << 16;
        }
        if rest.len() >= 2 {
            h ^= (rest[1] as u32) << 8;
        }
        h ^= rest[0] as u32;
        h = h.wrapping_mul(M);
    }
    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^ (h >> 15)
}

async fn export_curseforge(paths: &Paths, inst: &Instance, opts: &ExportOptions, cf: Option<&curseforge::Client>) -> Result<ExportReport> {
    let content = content_files(paths, &inst.id).await?;

    // file sha1 -> (projectID, fileID)
    let mut ids: HashMap<String, (u64, u64)> = HashMap::new();
    for c in &content {
        if let (Some(pid), Some(vid)) = (&c.project_id, &c.version_id) {
            if curseforge::is_cf(pid) {
                if let (Ok(p), Ok(f)) = (pid.trim_start_matches(curseforge::PREFIX).parse(), vid.parse()) {
                    ids.insert(c.sha1.clone(), (p, f));
                }
            }
        }
    }
    // With a key, recognise the rest by CurseForge fingerprint.
    if let Some(cf) = cf {
        let unknown: Vec<&ContentFile> = content.iter().filter(|c| !ids.contains_key(&c.sha1)).collect();
        if !unknown.is_empty() {
            let mut prints: HashMap<u32, String> = HashMap::new();
            for c in &unknown {
                let bytes = tokio::fs::read(&c.abs).await?;
                prints.insert(cf_fingerprint(&bytes), c.sha1.clone());
            }
            for (fp, (pid, fid)) in cf.fingerprint_matches(&prints.keys().copied().collect::<Vec<_>>()).await? {
                if let Some(sha) = prints.get(&fp) {
                    ids.insert(sha.clone(), (pid, fid));
                }
            }
        }
    }

    let mut files = Vec::new();
    let mut bundled_content = Vec::new();
    for c in &content {
        match ids.get(&c.sha1) {
            Some((p, f)) => files.push(json!({ "projectID": p, "fileID": f, "required": true })),
            None => bundled_content.push((c.rel.clone(), c.abs.clone())),
        }
    }

    let mut loaders = Vec::new();
    if let Some(v) = plain_loader_version(inst) {
        let id = match inst.loader.as_str() {
            "forge" => format!("forge-{v}"),
            "neoforge" => format!("neoforge-{v}"),
            "fabric" => format!("fabric-{v}"),
            "quilt" => format!("quilt-{v}"),
            _ => String::new(),
        };
        if !id.is_empty() {
            loaders.push(json!({ "id": id, "primary": true }));
        }
    }
    let manifest = json!({
        "minecraft": { "version": inst.game_version, "modLoaders": loaders },
        "manifestType": "minecraftModpack",
        "manifestVersion": 1,
        "name": opts.name,
        "version": opts.version,
        "author": opts.author,
        "files": files,
        "overrides": "overrides",
    });

    let game = instances::game_dir(paths, &inst.id);
    let mut overrides = override_files(&game, &opts.include)?;
    let override_count = overrides.len();
    let bundled: Vec<String> = bundled_content.iter().map(|(r, _)| r.clone()).collect();
    overrides.extend(bundled_content);
    let linked = files.len();

    let out = PathBuf::from(&opts.path);
    let size = tokio::task::spawn_blocking(move || write_zip(&out, "manifest.json", &manifest, &overrides, "overrides"))
        .await
        .map_err(|e| Error::msg(e.to_string()))??;
    Ok(ExportReport { path: opts.path.clone(), linked, bundled, override_files: override_count, size_bytes: size })
}

pub async fn export(paths: &Paths, inst_id: &str, opts: &ExportOptions, cf: Option<&curseforge::Client>) -> Result<ExportReport> {
    if opts.name.trim().is_empty() || opts.version.trim().is_empty() {
        return Err(Error::msg("Give the pack a name and a version."));
    }
    let inst = instances::get(paths, inst_id).await?;
    match opts.format.as_str() {
        "modrinth" => export_modrinth(paths, &inst, opts).await,
        "curseforge" => export_curseforge(paths, &inst, opts, cf).await,
        other => Err(Error::msg(format!("Unknown export format '{other}'"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_ignores_whitespace() {
        // Whitespace bytes are stripped before hashing.
        assert_eq!(cf_fingerprint(b"a b\tc\r\nd"), cf_fingerprint(b"abcd"));
        assert_ne!(cf_fingerprint(b"abcd"), cf_fingerprint(b"abce"));
        // SMHasher's verification value for MurmurHash2 (32-bit): hash keys
        // [0], [0,1], ... [0..254] with seed 256-i, then hash all the results.
        let key: Vec<u8> = (0..=255u8).collect();
        let mut hashes = Vec::with_capacity(256 * 4);
        for i in 0..256usize {
            hashes.extend_from_slice(&murmur2(&key[..i], 256 - i as u32).to_le_bytes());
        }
        assert_eq!(murmur2(&hashes, 0), 0x2786_4C1E);
    }

    /// Export -> re-import round trip against the real Modrinth API.
    /// cargo test -- --ignored --nocapture export_round_trip
    #[tokio::test]
    #[ignore]
    async fn export_round_trip() {
        use super::super::modpacks;
        let root = std::env::temp_dir().join(format!("peak-export-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = Paths::new(root.clone());
        let inst = instances::create(&paths, "Export Me", "1.21.4", "fabric", Some("0.16.9".into())).await.unwrap();
        let installed = modrinth::install(&paths, &inst.id, Kind::Mod, "mOgUt4GM", None).await.unwrap(); // Mod Menu
        let game = instances::game_dir(&paths, &inst.id);
        std::fs::create_dir_all(game.join("config")).unwrap();
        std::fs::write(game.join("config/modmenu.json"), b"{\"mods_button_style\":\"classic\"}").unwrap();
        std::fs::write(game.join("mods/my-private-mod.jar"), b"not on modrinth").unwrap();
        std::fs::create_dir_all(game.join("logs")).unwrap();
        std::fs::write(game.join("logs/latest.log"), b"log").unwrap();

        let suggested: Vec<String> = entries(&paths, &inst.id).await.unwrap().into_iter().filter(|e| e.suggested).map(|e| e.name).collect();
        assert_eq!(suggested, vec!["config".to_string()]);

        let out = root.join("Export Me-1.0.0.mrpack");
        let opts = ExportOptions {
            format: "modrinth".into(),
            name: "Export Me".into(),
            version: "1.0.0".into(),
            author: "Tester".into(),
            summary: "Round trip".into(),
            include: suggested.clone(),
            path: out.to_string_lossy().into(),
        };
        let report = export(&paths, &inst.id, &opts, None).await.unwrap();
        println!("modrinth: installed {installed:?} -> linked {} bundled {:?} overrides {} size {}", report.linked, report.bundled, report.override_files, report.size_bytes);
        assert_eq!(report.linked, installed.len());
        assert_eq!(report.bundled, vec!["mods/my-private-mod.jar".to_string()]);
        assert_eq!(report.override_files, 1);

        // Re-import it as a new instance.
        let (copy, index) = modpacks::prepare(&paths, &out, None).await.unwrap();
        assert_eq!((copy.game_version.as_str(), copy.loader.as_str(), copy.loader_version.as_deref()), ("1.21.4", "fabric", Some("0.16.9")));
        let progress: super::super::install::ProgressFn = std::sync::Arc::new(|_| {});
        modpacks::populate(&paths, &copy, &out, &index, &progress).await.unwrap();
        let copy_game = instances::game_dir(&paths, &copy.id);
        let mods = std::fs::read_dir(copy_game.join("mods")).unwrap().count();
        println!("re-imported: {mods} mods, config present {}", copy_game.join("config/modmenu.json").exists());
        assert_eq!(mods, installed.len() + 1);
        assert!(copy_game.join("config/modmenu.json").exists());
        assert!(!copy_game.join("logs").exists());

        // CurseForge export without a key: Modrinth-only files are bundled.
        let cf_out = root.join("Export Me-1.0.0.zip");
        let cf_opts = ExportOptions { format: "curseforge".into(), path: cf_out.to_string_lossy().into(), ..opts };
        let cf_report = export(&paths, &inst.id, &cf_opts, None).await.unwrap();
        println!("curseforge (no key): linked {} bundled {}", cf_report.linked, cf_report.bundled.len());
        let manifest = super::super::curseforge::is_curseforge_pack(&cf_out);
        assert!(manifest);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_unsafe_entries() {
        assert!(safe_entry("config").is_ok());
        assert!(safe_entry("../x").is_err());
        assert!(safe_entry("a/b").is_err());
    }
}
