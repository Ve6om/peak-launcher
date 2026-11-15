//! Which installed mods need which. Read from the metadata inside each mod
//! file (the same data the mod loader checks), so it covers mods from
//! modpacks and ones added by hand, not only those installed through Peak.

use crate::error::{Error, Result};
use crate::launcher::modrinth::{content_dir, Kind};
use crate::launcher::paths::Paths;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::io::{Cursor, Read, Seek};
use std::path::Path;

/// Ids every loader provides itself; never a mod in the mods folder.
const BUILT_IN: &[&str] = &[
    "minecraft", "java", "fabricloader", "fabric-loader", "quilt_loader", "forge", "neoforge", "fml", "javafml", "mcp",
    "lowcodefml", "mixinextras",
];

/// How deep to follow mods bundled inside other mods.
const MAX_NESTING: u8 = 2;

#[derive(Debug, Default, Clone)]
pub struct ModInfo {
    /// Display name from the metadata, if any.
    pub name: Option<String>,
    /// Mod ids this file provides (its own, aliases and bundled mods).
    pub provides: HashSet<String>,
    /// Mod ids it can't run without.
    pub requires: HashSet<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedMod {
    pub file_name: String,
    pub name: String,
    /// Why it's affected, e.g. "Needs Fabric API" or "Needed by Create".
    pub reason: String,
}

fn norm(id: &str) -> String {
    id.trim().to_lowercase()
}

fn read_entry<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, name: &str) -> Option<Vec<u8>> {
    let mut f = zip.by_name(name).ok()?;
    let mut buf = Vec::with_capacity(f.size() as usize);
    f.read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// Strings from a JSON value that may be a string, a list of strings, or
/// objects with an "id" field (Quilt allows all three).
fn ids_of(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => vec![s.clone()],
        Value::Array(a) => a.iter().flat_map(ids_of).collect(),
        Value::Object(o) => o.get("id").and_then(Value::as_str).map(|s| vec![s.to_string()]).unwrap_or_default(),
        _ => vec![],
    }
}

/// Read a mod's metadata from an open jar.
fn read_jar<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, info: &mut ModInfo, depth: u8) {
    let mut nested: Vec<String> = Vec::new();

    if let Some(v) = read_entry(zip, "fabric.mod.json").and_then(|b| serde_json::from_slice::<Value>(&lenient(&b)).ok()) {
        if let Some(id) = v.get("id").and_then(Value::as_str) {
            info.provides.insert(norm(id));
        }
        for p in v.get("provides").map(ids_of).unwrap_or_default() {
            info.provides.insert(norm(&p));
        }
        if depth == 0 {
            info.name = info.name.take().or_else(|| v.get("name").and_then(Value::as_str).map(String::from));
            if let Some(Value::Object(deps)) = v.get("depends") {
                info.requires.extend(deps.keys().map(|k| norm(k)));
            }
        }
        if let Some(Value::Array(jars)) = v.get("jars") {
            nested.extend(jars.iter().filter_map(|j| j.get("file").and_then(Value::as_str).map(String::from)));
        }
    }

    if let Some(v) = read_entry(zip, "quilt.mod.json").and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
        if let Some(ql) = v.get("quilt_loader") {
            if let Some(id) = ql.get("id").and_then(Value::as_str) {
                info.provides.insert(norm(id));
            }
            for p in ql.get("provides").map(ids_of).unwrap_or_default() {
                info.provides.insert(norm(&p));
            }
            if depth == 0 {
                info.name = info
                    .name
                    .take()
                    .or_else(|| ql.pointer("/metadata/name").and_then(Value::as_str).map(String::from));
                if let Some(Value::Array(deps)) = ql.get("depends") {
                    for d in deps {
                        let optional = d.get("optional").and_then(Value::as_bool).unwrap_or(false);
                        if !optional {
                            info.requires.extend(ids_of(d).iter().map(|s| norm(s)));
                        }
                    }
                }
            }
            if let Some(jars) = ql.get("jars") {
                nested.extend(ids_of(jars));
            }
        }
    }

    for toml_name in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
        let Some(text) = read_entry(zip, toml_name).and_then(|b| String::from_utf8(b).ok()) else { continue };
        let Ok(doc) = text.parse::<toml::Table>() else { continue };
        let mods = doc.get("mods").and_then(|m| m.as_array()).cloned().unwrap_or_default();
        for m in &mods {
            if let Some(id) = m.get("modId").and_then(|v| v.as_str()) {
                info.provides.insert(norm(id));
            }
        }
        if depth == 0 {
            info.name = info.name.take().or_else(|| {
                mods.first().and_then(|m| m.get("displayName")).and_then(|v| v.as_str()).map(String::from)
            });
            if let Some(deps) = doc.get("dependencies").and_then(|d| d.as_table()) {
                for list in deps.values().filter_map(|l| l.as_array()) {
                    for d in list {
                        // Forge: mandatory = true. NeoForge: type = "required" (the default).
                        let required = match (d.get("type").and_then(|v| v.as_str()), d.get("mandatory").and_then(|v| v.as_bool())) {
                            (Some(t), _) => t.eq_ignore_ascii_case("required"),
                            (None, Some(m)) => m,
                            (None, None) => toml_name.contains("neoforge"),
                        };
                        if required {
                            if let Some(id) = d.get("modId").and_then(|v| v.as_str()) {
                                info.requires.insert(norm(id));
                            }
                        }
                    }
                }
            }
        }
        break;
    }

    // Forge/NeoForge "Jar-in-Jar" bundles.
    if let Some(v) = read_entry(zip, "META-INF/jarjar/metadata.json").and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
        if let Some(Value::Array(jars)) = v.get("jars") {
            nested.extend(jars.iter().filter_map(|j| j.get("path").and_then(Value::as_str).map(String::from)));
        }
    }

    // Old Forge (1.12 and earlier).
    if let Some(v) = read_entry(zip, "mcmod.info").and_then(|b| serde_json::from_slice::<Value>(&lenient(&b)).ok()) {
        let list = match &v {
            Value::Array(a) => a.clone(),
            Value::Object(o) => o.get("modList").and_then(Value::as_array).cloned().unwrap_or_default(),
            _ => vec![],
        };
        for m in &list {
            if let Some(id) = m.get("modid").and_then(Value::as_str) {
                info.provides.insert(norm(id));
            }
            if depth == 0 {
                if let Some(Value::Array(req)) = m.get("requiredMods") {
                    for r in req.iter().filter_map(Value::as_str) {
                        // "modid@[1.0,)"
                        info.requires.insert(norm(r.split('@').next().unwrap_or(r)));
                    }
                }
            }
        }
        if depth == 0 {
            info.name = info.name.take().or_else(|| list.first().and_then(|m| m.get("name")).and_then(Value::as_str).map(String::from));
        }
    }

    if depth < MAX_NESTING {
        for path in nested {
            if let Some(bytes) = read_entry(zip, path.trim_start_matches('/')) {
                if let Ok(mut inner) = zip::ZipArchive::new(Cursor::new(bytes)) {
                    read_jar(&mut inner, info, depth + 1);
                }
            }
        }
    }
}

/// Some mods ship JSON with raw newlines inside strings; turn those into
/// spaces so the file still parses.
fn lenient(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut in_str = false;
    let mut escaped = false;
    for &b in bytes {
        if in_str {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_str = false;
            } else if b == b'\n' || b == b'\r' || b == b'\t' {
                out.push(b' ');
                continue;
            }
        } else if b == b'"' {
            in_str = true;
        }
        out.push(b);
    }
    out
}

pub fn read_mod(path: &Path) -> ModInfo {
    let mut info = ModInfo::default();
    if let Ok(file) = std::fs::File::open(path) {
        if let Ok(mut zip) = zip::ZipArchive::new(std::io::BufReader::new(file)) {
            read_jar(&mut zip, &mut info, 0);
        }
    }
    // Fabric API's id changed from "fabric" to "fabric-api"; mods use either.
    if info.provides.contains("fabric-api") || info.provides.contains("fabric") {
        info.provides.insert("fabric".into());
        info.provides.insert("fabric-api".into());
    }
    for b in BUILT_IN {
        info.requires.remove(*b);
    }
    let own = info.provides.clone();
    info.requires.retain(|r| !own.contains(r));
    info
}

struct Entry {
    file_name: String,
    enabled: bool,
    name: String,
    info: ModInfo,
}

fn scan(dir: &Path) -> Vec<Entry> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let file_name = e.file_name().to_string_lossy().into_owned();
        let base = file_name.strip_suffix(".disabled").unwrap_or(&file_name);
        if !base.ends_with(".jar") || !e.path().is_file() {
            continue;
        }
        let info = read_mod(&e.path());
        let name = info.name.clone().unwrap_or_else(|| base.trim_end_matches(".jar").to_string());
        out.push(Entry { enabled: !file_name.ends_with(".disabled"), file_name, name, info });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// The mods that would stop working if `file_name` were disabled or removed,
/// directly or through another mod. Only enabled mods are listed, and a
/// dependency still provided by another enabled file doesn't count.
fn dependents_in(entries: &[Entry], file_name: &str) -> Vec<LinkedMod> {
    let Some(target) = entries.iter().position(|e| e.file_name == file_name) else { return vec![] };
    let mut gone: HashSet<usize> = HashSet::from([target]);
    let mut out = Vec::new();
    // Ids some file in the folder provides; others (missing already, or built in) are ignored.
    let known: HashSet<&String> = entries.iter().flat_map(|e| e.info.provides.iter()).collect();
    loop {
        let mut added = false;
        for (i, e) in entries.iter().enumerate() {
            if !e.enabled || gone.contains(&i) {
                continue;
            }
            let missing = e.info.requires.iter().filter(|r| known.contains(r)).find(|r| {
                !entries.iter().enumerate().any(|(j, p)| p.enabled && !gone.contains(&j) && p.info.provides.contains(*r))
            });
            if let Some(r) = missing {
                let by = entries
                    .iter()
                    .enumerate()
                    .find(|(j, p)| gone.contains(j) && p.info.provides.contains(r))
                    .map(|(_, p)| p.name.clone())
                    .unwrap_or_else(|| r.clone());
                out.push(LinkedMod { file_name: e.file_name.clone(), name: e.name.clone(), reason: format!("Needs {by}") });
                gone.insert(i);
                added = true;
            }
        }
        if !added {
            return out;
        }
    }
}

/// Disabled mods that `file_name` needs, directly or through them, which
/// should be turned back on with it.
fn requirements_in(entries: &[Entry], file_name: &str) -> Vec<LinkedMod> {
    let Some(target) = entries.iter().position(|e| e.file_name == file_name) else { return vec![] };
    let mut on: HashSet<usize> = HashSet::from([target]);
    let mut queue = vec![target];
    let mut out = Vec::new();
    while let Some(i) = queue.pop() {
        let needer = &entries[i];
        for r in &needer.info.requires {
            let available = entries.iter().enumerate().any(|(j, p)| (p.enabled || on.contains(&j)) && p.info.provides.contains(r));
            if available {
                continue;
            }
            if let Some((j, p)) = entries.iter().enumerate().find(|(j, p)| !p.enabled && !on.contains(j) && p.info.provides.contains(r)) {
                out.push(LinkedMod { file_name: p.file_name.clone(), name: p.name.clone(), reason: format!("Needed by {}", needer.name) });
                on.insert(j);
                queue.push(j);
            }
        }
    }
    out
}

/// Mods affected by changing one mod. `action` is "disable", "remove" or "enable".
pub async fn linked(paths: &Paths, inst: &str, file_name: &str, action: &str) -> Result<Vec<LinkedMod>> {
    let dir = content_dir(paths, inst, Kind::Mod);
    let file_name = file_name.to_string();
    let action = action.to_string();
    tokio::task::spawn_blocking(move || {
        let entries = scan(&dir);
        match action.as_str() {
            "disable" | "remove" => Ok(dependents_in(&entries, &file_name)),
            "enable" => Ok(requirements_in(&entries, &file_name)),
            _ => Err(Error::msg("Unknown action")),
        }
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn jar(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            for (name, data) in files {
                z.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
                z.write_all(data).unwrap();
            }
            z.finish().unwrap();
        }
        buf.into_inner()
    }

    fn entry(name: &str, enabled: bool, provides: &[&str], requires: &[&str]) -> Entry {
        Entry {
            file_name: format!("{name}.jar{}", if enabled { "" } else { ".disabled" }),
            enabled,
            name: name.into(),
            info: ModInfo {
                name: Some(name.into()),
                provides: provides.iter().map(|s| s.to_string()).collect(),
                requires: requires.iter().map(|s| s.to_string()).collect(),
            },
        }
    }

    #[test]
    fn reads_fabric_with_nested_jars() {
        let inner = jar(&[("fabric.mod.json", br#"{"id":"fabric-lifecycle-events-v1"}"#)]);
        let outer = jar(&[
            (
                "fabric.mod.json",
                br#"{"id":"fabric-api","name":"Fabric API","depends":{"fabricloader":">=0.15","minecraft":"1.21.1"},"jars":[{"file":"META-INF/jars/life.jar"}]}"#,
            ),
            ("META-INF/jars/life.jar", &inner),
        ]);
        let dir = std::env::temp_dir().join(format!("peak-moddeps-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("fabric-api.jar");
        std::fs::write(&p, outer).unwrap();
        let info = read_mod(&p);
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(info.name.as_deref(), Some("Fabric API"));
        assert!(info.provides.contains("fabric-lifecycle-events-v1"));
        assert!(info.provides.contains("fabric"));
        assert!(info.requires.is_empty(), "loader and game aren't mods: {:?}", info.requires);
    }

    #[test]
    fn reads_forge_and_neoforge_toml() {
        let forge = br#"
modLoader="javafml"
[[mods]]
modId="create"
displayName="Create"
[[dependencies.create]]
modId="forge"
mandatory=true
[[dependencies.create]]
modId="flywheel"
mandatory=true
[[dependencies.create]]
modId="jei"
mandatory=false
"#;
        let mut info = ModInfo::default();
        read_jar(&mut zip::ZipArchive::new(Cursor::new(jar(&[("META-INF/mods.toml", forge)]))).unwrap(), &mut info, 0);
        assert!(info.provides.contains("create"));
        assert!(info.requires.contains("flywheel"));
        assert!(!info.requires.contains("jei"));

        let neo = br#"
[[mods]]
modId="sophisticatedbackpacks"
[[dependencies.sophisticatedbackpacks]]
modId="sophisticatedcore"
type="required"
[[dependencies.sophisticatedbackpacks]]
modId="curios"
type="optional"
"#;
        let mut info = ModInfo::default();
        read_jar(&mut zip::ZipArchive::new(Cursor::new(jar(&[("META-INF/neoforge.mods.toml", neo)]))).unwrap(), &mut info, 0);
        assert!(info.requires.contains("sophisticatedcore"));
        assert!(!info.requires.contains("curios"));
    }

    #[test]
    fn reads_old_mcmod_info() {
        let text = b"[{\"modid\":\"thaumcraft\",\"name\":\"Thaumcraft\",\"description\":\"line one\nline two\",\"requiredMods\":[\"baubles@[1.5,)\",\"Forge\"]}]";
        let mut info = ModInfo::default();
        read_jar(&mut zip::ZipArchive::new(Cursor::new(jar(&[("mcmod.info", text)]))).unwrap(), &mut info, 0);
        assert!(info.provides.contains("thaumcraft"));
        assert!(info.requires.contains("baubles"));
    }

    #[test]
    fn finds_dependents_through_chains() {
        let mods = vec![
            entry("Fabric API", true, &["fabric-api"], &[]),
            entry("Create", true, &["create"], &["fabric-api", "flywheel"]),
            entry("Flywheel", true, &["flywheel"], &["fabric-api"]),
            entry("Create Addon", true, &["createaddon"], &["create"]),
            entry("Sodium", true, &["sodium"], &[]),
            entry("Old Addon", false, &["oldaddon"], &["create"]),
        ];
        let d = dependents_in(&mods, "Flywheel.jar");
        let names: Vec<_> = d.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Create", "Create Addon"]);
        assert_eq!(d[0].reason, "Needs Flywheel");
        assert_eq!(d[1].reason, "Needs Create");
        assert!(dependents_in(&mods, "Sodium.jar").is_empty());
    }

    #[test]
    fn duplicate_provider_keeps_dependents_running() {
        let mods = vec![
            entry("Lib A", true, &["lib"], &[]),
            entry("Lib B", true, &["lib"], &[]),
            entry("User", true, &["user"], &["lib"]),
        ];
        assert!(dependents_in(&mods, "Lib A.jar").is_empty());
    }

    #[test]
    fn enabling_brings_back_disabled_requirements() {
        let mods = vec![
            entry("Core", false, &["core"], &[]),
            entry("Lib", false, &["lib"], &["core"]),
            entry("Mod", false, &["mod"], &["lib", "missing-thing"]),
        ];
        let r = requirements_in(&mods, "Mod.jar.disabled");
        let names: Vec<_> = r.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Lib", "Core"]);
        assert_eq!(r[1].reason, "Needed by Lib");
    }
}

#[cfg(test)]
mod live {
    /// Print what each mod in a real folder needs and what depends on it:
    /// PEAK_TEST_MODS=<mods folder> cargo test --lib -- --ignored --nocapture moddeps_scan
    #[test]
    #[ignore]
    fn moddeps_scan() {
        let dir = std::path::PathBuf::from(std::env::var("PEAK_TEST_MODS").expect("PEAK_TEST_MODS"));
        let entries = super::scan(&dir);
        let mut with_deps = 0;
        for e in &entries {
            let d = super::dependents_in(&entries, &e.file_name);
            if !d.is_empty() {
                with_deps += 1;
                let list: Vec<_> = d.iter().map(|m| format!("{} ({})", m.name, m.reason)).collect();
                println!("{} -> {}", e.name, list.join(", "));
            }
        }
        let unnamed = entries.iter().filter(|e| e.info.provides.is_empty()).count();
        println!("{} mods, {} with dependents, {} without readable metadata", entries.len(), with_deps, unnamed);
    }
}
