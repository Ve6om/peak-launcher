//! The crash helper: when a game closes with an error, read its crash report
//! and log and say what went wrong in plain words, with a fix Peak can apply
//! (install a missing mod, give it more memory, use the right Java, turn a
//! mod off or pick another version of it).
//!
//! Only well-known messages from Minecraft, Fabric, Quilt, Forge and NeoForge
//! are recognised. Anything else gets a short excerpt and the report to open.

use super::instances;
use super::moddeps::{self, Installed};
use super::paths::Paths;
use regex::Regex;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Diagnosis {
    /// "missing-mod" | "wrong-version" | "java" | "memory" | "duplicate" | "mod-crash" | "unknown"
    pub kind: &'static str,
    pub title: String,
    pub detail: String,
    pub fix: Option<Fix>,
    /// The crash report this came from, to open.
    pub report: Option<String>,
    /// A few lines from the report or log, for "unknown".
    pub excerpt: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum Fix {
    /// Find this mod id on Modrinth / CurseForge and install it.
    #[serde(rename_all = "camelCase")]
    InstallMod { mod_id: String, name: String },
    /// Give the instance this much memory.
    #[serde(rename_all = "camelCase")]
    Memory { mb: u32 },
    /// Use automatic Java (the game needs Java `major`).
    #[serde(rename_all = "camelCase")]
    Java { major: u32 },
    /// Pick another version of this installed mod.
    #[serde(rename_all = "camelCase")]
    ChangeVersion { file_name: String, name: String },
    /// Turn this mod off.
    #[serde(rename_all = "camelCase")]
    DisableMod { file_name: String, name: String },
}

static RX: LazyLock<Rx> = LazyLock::new(Rx::new);

struct Rx {
    class_version: Regex,
    fabric_java: Regex,
    fabric_missing: Regex,
    fabric_wrong: Regex,
    fabric_replace: Regex,
    fabric_remove: Regex,
    forge_dep: Regex,
    neoforge_missing: Regex,
    duplicate: Regex,
    suspected: Regex,
    mixin: Regex,
    mod_file: Regex,
}

impl Rx {
    fn new() -> Self {
        let r = |s: &str| Regex::new(s).unwrap();
        Rx {
            // "...compiled by a more recent version of the Java Runtime (class file version 65.0)..."
            class_version: r(r"class file version (\d+)\.\d+"),
            // Fabric: "requires version 21 or later of 'OpenJDK 64-Bit Server VM' (java), but only the wrong version is present: 17!"
            fabric_java: r(r"requires (?:version )?(\d+)(?:\.\S*)? or later of [^\n]*\(java\)"),
            // Fabric: "Mod 'Sodium Extra' (sodium-extra) 0.5.4 requires any version of 'Sodium' (sodium), which is missing!"
            //         "Mod 'X' (x) 1.0 requires any version of fabric-api, which is missing!"
            fabric_missing: r(r"Mod '([^']+)' \(([^)]+)\) \S+ requires [^\n]*? of (?:mod )?(?:'([^']+)' \(([^)]+)\)|([\w.\-]+)), which is missing"),
            // Fabric: "...requires version 0.6 or later of 'Sodium' (sodium), but only the wrong version is present: 0.5.8!"
            fabric_wrong: r(r"Mod '([^']+)' \(([^)]+)\) \S+ requires [^\n]*? of (?:mod )?(?:'([^']+)' \(([^)]+)\)|([\w.\-]+)), but only the wrong version is present"),
            // Fabric's own suggested solution: "Replace mod 'Nvidium' (nvidium) 0.4.4 with any version that is compatible with: ..."
            fabric_replace: r(r"Replace mod '([^']+)' \(([^)]+)\)"),
            // "Remove mod 'X' (x) 1.0."
            fabric_remove: r(r"Remove mod '([^']+)' \(([^)]+)\)"),
            // Forge / older NeoForge: "Mod ID: 'kotlinforforge', Requested by: 'x', Expected range: '[4.0,)', Actual version: '[MISSING]'"
            forge_dep: r(r"Mod ID: '([^']+)', Requested by: '([^']+)', Expected range: '[^']*', Actual version: '([^']+)'"),
            // NeoForge: "Mod Create requires Flywheel 1.0.0 or above\nCurrently, flywheel is not installed"
            neoforge_missing: r(r"Mod (.+?) requires (.+?) \S+ or above\s+Currently, (\S+) is not installed"),
            // Fabric / Forge duplicates.
            duplicate: r(r"(?i)duplicate(?:, conflicting)? mods? (?:found|id)|found duplicate mods|mod id '([^']+)'[^\n]*from mod files"),
            // Forge / NeoForge crash reports: "Suspected Mods: Create (create), Version: 0.5.1" or a list under "Suspected Mod(s):".
            suspected: r(r"Suspected Mods?(?:\(s\))?:\s*\n?\s*([^\n(]+?) \(([\w.\-]+)\)"),
            // Mixin failures name the mod: "Mixin apply for mod sodium failed" / "from mod sodium->".
            mixin: r(r"(?:Mixin apply for mod|from mod) ([\w.\-]+)"),
            // Forge mod-loading errors: "Mod File: /path/mods/thing-1.2.jar"
            mod_file: r(r"Mod File: [^\n]*?[/\\]([^/\\\n]+\.jar)"),
        }
    }
}

/// The newest crash report written after `since`, if any.
fn newest_report(game: &Path, since: SystemTime) -> Option<PathBuf> {
    std::fs::read_dir(game.join("crash-reports"))
        .ok()?
        .flatten()
        .filter_map(|e| {
            let t = e.metadata().ok()?.modified().ok()?;
            (t >= since && e.file_name().to_string_lossy().ends_with(".txt")).then(|| (t, e.path()))
        })
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

/// The end of the latest log (where the error is), at most ~400 KB.
fn log_tail(game: &Path) -> String {
    let Ok(bytes) = std::fs::read(game.join("logs").join("latest.log")) else { return String::new() };
    let start = bytes.len().saturating_sub(400_000);
    String::from_utf8_lossy(&bytes[start..]).into_owned()
}

fn find_mod<'a>(mods: &'a [Installed], id: &str) -> Option<&'a Installed> {
    let id = moddeps::mod_id(id);
    mods.iter().find(|m| m.provides.contains(&id))
}

/// Read what went wrong. `text` is the crash report (if any) followed by the
/// end of the log; `mods` are the instance's mod files; `memory_mb` what the
/// game had.
pub fn diagnose(text: &str, mods: &[Installed], memory_mb: u32, report: Option<String>) -> Diagnosis {
    let rx = &*RX;
    let make = |kind, title: String, detail: String, fix| Diagnosis { kind, title, detail, fix, report: report.clone(), excerpt: vec![] };
    let name_of = |id: &str, fallback: Option<&str>| {
        find_mod(mods, id).map(|m| m.name.clone()).or(fallback.map(String::from)).unwrap_or_else(|| id.to_string())
    };

    // Out of memory.
    if text.contains("java.lang.OutOfMemoryError") {
        let mb = (memory_mb + 2048).min(16384);
        return make(
            "memory",
            "The game ran out of memory".into(),
            format!("It had {} GB. Giving it {} GB usually fixes this, as long as your computer has enough to spare.", gb(memory_mb), gb(mb)),
            Some(Fix::Memory { mb }),
        );
    }

    // Wrong Java.
    let needed = rx
        .fabric_java
        .captures(text)
        .and_then(|c| c[1].parse::<u32>().ok())
        .or_else(|| {
            let hint = text.contains("UnsupportedClassVersionError") || text.contains("more recent version of the Java Runtime");
            hint.then(|| rx.class_version.captures_iter(text).filter_map(|c| c[1].parse::<u32>().ok()).max()).flatten().map(|v| v - 44)
        });
    if let Some(major) = needed {
        return make(
            "java",
            format!("This needs Java {major}"),
            format!("The game or one of its mods was made for Java {major}, but a different Java was used. Peak can pick the right one automatically."),
            Some(Fix::Java { major }),
        );
    }

    // Missing dependencies (Fabric / Quilt).
    if let Some(c) = rx.fabric_missing.captures(text) {
        let (who, dep_name, dep_id) = (c[1].to_string(), c.get(3).map(|m| m.as_str()), c.get(4).or(c.get(5)).map(|m| m.as_str()).unwrap_or(""));
        let name = name_of(dep_id, dep_name);
        let more = rx.fabric_missing.captures_iter(text).count().saturating_sub(1);
        return make(
            "missing-mod",
            format!("{name} is missing"),
            format!("{who} needs {name}, which isn't installed{}.", if more > 0 { format!(" ({more} more missing mod{} listed in the log)", if more == 1 { "" } else { "s" }) } else { String::new() }),
            Some(Fix::InstallMod { mod_id: dep_id.to_string(), name }),
        );
    }
    if let Some(c) = rx.fabric_wrong.captures(text) {
        let (who, who_id, dep_name, dep_id) = (c[1].to_string(), c[2].to_string(), c.get(3).map(|m| m.as_str()), c.get(4).or(c.get(5)).map(|m| m.as_str()).unwrap_or(""));
        let name = name_of(dep_id, dep_name);
        // Fabric says which mod to replace when it can tell (usually the one
        // that needs an older or newer version, not the one everything uses).
        let replace = rx.fabric_replace.captures(text).map(|r| r[2].to_string());
        let target = replace.as_deref().and_then(|id| find_mod(mods, id)).or_else(|| find_mod(mods, dep_id)).or_else(|| find_mod(mods, &who_id));
        let fix = target.map(|m| Fix::ChangeVersion { file_name: m.file_name.clone(), name: m.name.clone() });
        let detail = match target {
            Some(m) if replace.is_some() => format!(
                "{who} needs a different version of {name} than the one installed. Picking another version of {} that works with your {name} should fix it.",
                m.name
            ),
            _ => format!("{who} needs a different version of {name} than the one installed."),
        };
        return make("wrong-version", format!("{who} and {name} don't match"), detail, fix);
    }
    if let Some(c) = rx.fabric_remove.captures(text) {
        if let Some(m) = find_mod(mods, &c[2]) {
            return make(
                "mod-crash",
                format!("{} doesn't work with this setup", m.name),
                format!("Fabric suggests removing {}. Turn it off to play without it.", m.name),
                Some(Fix::DisableMod { file_name: m.file_name.clone(), name: m.name.clone() }),
            );
        }
    }

    // Missing dependencies (Forge / NeoForge).
    if let Some(c) = rx.forge_dep.captures(text) {
        let (dep_id, who, actual) = (c[1].to_string(), name_of(&c[2], None), c[3].to_string());
        let name = name_of(&dep_id, None);
        if actual.contains("MISSING") {
            return make("missing-mod", format!("{name} is missing"), format!("{who} needs {name}, which isn't installed."), Some(Fix::InstallMod { mod_id: dep_id, name }));
        }
        let fix = find_mod(mods, &dep_id).map(|m| Fix::ChangeVersion { file_name: m.file_name.clone(), name: m.name.clone() });
        return make("wrong-version", format!("{name} is the wrong version"), format!("{who} needs a different version of {name} than {actual}."), fix);
    }
    if let Some(c) = rx.neoforge_missing.captures(text) {
        let (who, dep_name, dep_id) = (c[1].trim().to_string(), c[2].trim().to_string(), c[3].to_string());
        return make("missing-mod", format!("{dep_name} is missing"), format!("{who} needs {dep_name}, which isn't installed."), Some(Fix::InstallMod { mod_id: dep_id, name: dep_name }));
    }

    // The same mod twice.
    if rx.duplicate.is_match(text) {
        let id = rx.duplicate.captures(text).and_then(|c| c.get(1).map(|m| m.as_str().to_string()));
        let copies: Vec<&Installed> = id.as_deref().map(|i| mods.iter().filter(|m| m.enabled && m.provides.contains(&moddeps::mod_id(i))).collect()).unwrap_or_default();
        let fix = (copies.len() > 1).then(|| Fix::DisableMod { file_name: copies[copies.len() - 1].file_name.clone(), name: copies[copies.len() - 1].file_name.clone() });
        let what = copies.first().map(|m| m.name.clone()).or(id).unwrap_or_else(|| "A mod".into());
        return make("duplicate", format!("{what} is installed twice"), "Two files provide the same mod. Turn one of them off.".into(), fix);
    }

    // A mod that crashed.
    let culprit = rx
        .suspected
        .captures(text)
        .map(|c| c[2].to_string())
        .or_else(|| rx.mod_file.captures(text).and_then(|c| mods.iter().find(|m| m.file_name.trim_end_matches(".disabled") == &c[1]).and_then(|m| m.provides.iter().next().cloned())))
        .or_else(|| rx.mixin.captures(text).map(|c| c[1].to_string()))
        .filter(|id| !matches!(id.as_str(), "minecraft" | "forge" | "neoforge" | "fabricloader" | "java"));
    if let Some(id) = culprit {
        if let Some(m) = find_mod(mods, &id) {
            return make(
                "mod-crash",
                format!("{} crashed the game", m.name),
                format!("The crash came from {}. Another version of it may work, or turn it off to play without it.", m.name),
                Some(Fix::ChangeVersion { file_name: m.file_name.clone(), name: m.name.clone() }),
            );
        }
    }

    // Something else: show the most telling lines.
    let excerpt: Vec<String> = text
        .lines()
        .filter(|l| {
            let l = l.trim_start();
            l.starts_with("Description:") || l.starts_with("Caused by:") || l.starts_with("Exception in") || l.contains("Exception:") || l.contains("Error:")
        })
        .map(|l| l.trim().chars().take(240).collect())
        .take(4)
        .collect();
    Diagnosis {
        kind: "unknown",
        title: "The game crashed".into(),
        detail: "Peak couldn't tell exactly why. The lines below and the crash report usually name the mod involved.".into(),
        fix: None,
        report,
        excerpt,
    }
}

fn gb(mb: u32) -> String {
    let v = mb as f64 / 1024.0;
    if (v - v.round()).abs() < 0.05 { format!("{}", v.round()) } else { format!("{v:.1}") }
}

/// Look at a game that just closed with an error.
pub async fn for_instance(paths: &Paths, inst_id: &str, since: SystemTime, global_memory_mb: u32, console: String) -> crate::error::Result<Diagnosis> {
    let inst = instances::get(paths, inst_id).await?;
    let game = instances::game_dir(paths, inst_id);
    let memory = inst.memory_mb.unwrap_or(global_memory_mb);
    tokio::task::spawn_blocking(move || {
        let report = newest_report(&game, since);
        let mut text = report.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
        text.push('\n');
        text.push_str(&log_tail(&game));
        // What the game printed (an error can show before the log file has it).
        text.push('\n');
        text.push_str(&console);
        let mods = moddeps::installed_mods(&game.join("mods"));
        diagnose(&text, &mods, memory, report.map(|p| p.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|e| crate::error::Error::msg(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn m(file: &str, name: &str, ids: &[&str]) -> Installed {
        Installed {
            file_name: file.into(),
            name: name.into(),
            enabled: true,
            provides: ids.iter().map(|s| s.to_string()).collect(),
            requires: HashSet::new(),
        }
    }

    #[test]
    fn fabric_missing_dependency() {
        let log = "Incompatible mods found!\n - Mod 'Sodium Extra' (sodium-extra) 0.5.4+mc1.21 requires any version of 'Sodium' (sodium), which is missing!\n";
        let d = diagnose(log, &[], 4096, None);
        assert_eq!(d.kind, "missing-mod");
        assert_eq!(d.title, "Sodium is missing");
        assert_eq!(d.fix, Some(Fix::InstallMod { mod_id: "sodium".into(), name: "Sodium".into() }));
        let d = diagnose(" - Mod 'Mod Menu' (modmenu) 11.0 requires any version of fabric-api, which is missing!", &[], 4096, None);
        assert_eq!(d.fix, Some(Fix::InstallMod { mod_id: "fabric-api".into(), name: "fabric-api".into() }));
    }

    #[test]
    fn fabric_wrong_version_points_at_the_installed_file() {
        let log = " - Mod 'Sodium Extra' (sodium-extra) 0.6 requires version 0.6 or later of 'Sodium' (sodium), but only the wrong version is present: 0.5.8!";
        let d = diagnose(log, &[m("sodium-fabric-0.5.8.jar", "Sodium", &["sodium"])], 4096, None);
        assert_eq!(d.kind, "wrong-version");
        assert_eq!(d.fix, Some(Fix::ChangeVersion { file_name: "sodium-fabric-0.5.8.jar".into(), name: "Sodium".into() }));
    }

    #[test]
    fn fabric_mod_wording_and_its_suggested_fix() {
        // Exactly as Fabric printed it for Nvidium and Sodium on 26.3.
        let log = "Incompatible mods found!\nA potential solution has been determined, this may resolve your problem:\n\t - Replace mod 'Nvidium' (nvidium) 0.4.4-beta8-26.3 with any version that is compatible with:\n\t\t - sodium 0.9.4-beta.2+mc26.3\nMore details:\n\t - Mod 'Nvidium' (nvidium) 0.4.4-beta8-26.3 requires version 0.9.2 or version 0.9.3 of mod 'Sodium' (sodium), but only the wrong version is present: 0.9.4-beta.2+mc26.3!";
        let mods = [m("nvidium-0.4.4-beta8-26.3.jar", "Nvidium", &["nvidium"]), m("sodium-fabric-0.9.4-beta.2+mc26.3.jar", "Sodium", &["sodium"])];
        let d = diagnose(log, &mods, 4096, None);
        assert_eq!(d.kind, "wrong-version");
        assert_eq!(d.title, "Nvidium and Sodium don't match");
        assert_eq!(d.fix, Some(Fix::ChangeVersion { file_name: "nvidium-0.4.4-beta8-26.3.jar".into(), name: "Nvidium".into() }));
        let missing = " - Mod 'X' (x) 1.0 requires any version of mod 'Cloth Config API' (cloth-config), which is missing!";
        assert_eq!(diagnose(missing, &[], 4096, None).fix, Some(Fix::InstallMod { mod_id: "cloth-config".into(), name: "Cloth Config API".into() }));
    }

    #[test]
    fn forge_missing_dependency() {
        let log = "Missing or unsupported mandatory dependencies:\n\tMod ID: 'kotlinforforge', Requested by: 'bigreactors', Expected range: '[4.0,)', Actual version: '[MISSING]'";
        let d = diagnose(log, &[m("ExtremeReactors.jar", "Extreme Reactors", &["bigreactors"])], 4096, None);
        assert_eq!(d.title, "kotlinforforge is missing");
        assert!(d.detail.starts_with("Extreme Reactors needs"));
        assert_eq!(d.fix, Some(Fix::InstallMod { mod_id: "kotlinforforge".into(), name: "kotlinforforge".into() }));
    }

    #[test]
    fn wrong_java_and_memory() {
        let log = "java.lang.UnsupportedClassVersionError: net/minecraft/client/main/Main has been compiled by a more recent version of the Java Runtime (class file version 65.0), this version of the Java Runtime only recognizes class file versions up to 61.0";
        assert_eq!(diagnose(log, &[], 4096, None).fix, Some(Fix::Java { major: 21 }));
        let fabric = " - Mod 'Minecraft' (minecraft) 1.21.1 requires version 21 or later of 'OpenJDK 64-Bit Server VM' (java), but only the wrong version is present: 17!";
        assert_eq!(diagnose(fabric, &[], 4096, None).fix, Some(Fix::Java { major: 21 }));
        let d = diagnose("Exception in thread \"main\" java.lang.OutOfMemoryError: Java heap space", &[], 4096, None);
        assert_eq!(d.fix, Some(Fix::Memory { mb: 6144 }));
    }

    #[test]
    fn names_the_mod_that_crashed() {
        let report = "---- Minecraft Crash Report ----\nDescription: Ticking entity\n\nSuspected Mods: Create (create), Version: 0.5.1\n";
        let d = diagnose(report, &[m("create-1.20.1-0.5.1.jar", "Create", &["create"])], 4096, Some("r.txt".into()));
        assert_eq!(d.kind, "mod-crash");
        assert_eq!(d.fix, Some(Fix::ChangeVersion { file_name: "create-1.20.1-0.5.1.jar".into(), name: "Create".into() }));
        assert_eq!(d.report.as_deref(), Some("r.txt"));
    }

    #[test]
    fn unknown_crashes_show_the_telling_lines() {
        let d = diagnose("blah\nDescription: Rendering overlay\njava.lang.NullPointerException: oops\n\tat x.y\n", &[], 4096, None);
        assert_eq!(d.kind, "unknown");
        assert_eq!(d.excerpt, vec!["Description: Rendering overlay".to_string(), "java.lang.NullPointerException: oops".to_string()]);
    }
}
