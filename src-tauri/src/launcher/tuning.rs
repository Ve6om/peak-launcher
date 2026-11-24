//! The "Optimize" button: memory, flags and Java suggested for one instance
//! from what's in it (how many mods, shaders) and what this computer has.
//! Modrinth and CurseForge don't publish a recommended amount of memory for
//! packs, so it's worked out here instead.

use super::instances::{self, Instance};
use super::paths::Paths;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub memory_mb: u32,
    pub jvm_preset: String,
    /// Suggest GraalVM for this instance (anything on Java 17+).
    pub graalvm: bool,
    pub mods: usize,
    pub shaders: bool,
    pub java_major: u32,
    pub total_memory_mb: u64,
    /// One line per choice, saying why.
    pub reasons: Vec<String>,
}

/// Leave the computer this much at least, whatever it has.
const RESERVE_MB: u64 = 4096;
/// More than this mostly makes the game's clean-up pauses longer.
const MAX_MB: u32 = 12288;

pub fn suggest(mods: usize, shaders: bool, total_mb: u64, java_major: u32) -> Suggestion {
    let mut reasons = Vec::new();
    let (mut want, why): (u32, String) = match mods {
        0 if java_major <= 8 => (2048, "Vanilla on older Minecraft is happy with 2 GB.".into()),
        0 => (3072, "Vanilla needs about 3 GB.".into()),
        1..=49 => (4096, format!("{mods} mods: a light modpack, 4 GB is plenty.")),
        50..=149 => (6144, format!("{mods} mods: a medium modpack, 6 GB fits well.")),
        150..=299 => (8192, format!("{mods} mods: a big modpack, 8 GB.")),
        _ => (10240, format!("{mods} mods: a very big modpack, 10 GB.")),
    };
    reasons.push(why);
    if shaders && mods > 0 {
        want += 1024;
        reasons.push("Shaders are installed, so 1 GB more.".into());
    }
    // What this computer can spare: at least 4 GB (or a quarter) stays free.
    let spare = total_mb.saturating_sub(RESERVE_MB.max(total_mb / 4));
    let cap = (spare.min(MAX_MB as u64) as u32 / 512 * 512).max(2048);
    let memory_mb = if want > cap {
        reasons.push(format!(
            "Your computer has {} GB, so it's held to {} GB to leave room for everything else.",
            gb(total_mb),
            gb(cap as u64)
        ));
        cap
    } else {
        want
    };

    let (jvm_preset, why) = if java_major >= 21 && memory_mb >= 8192 && total_mb >= 24 * 1024 {
        ("zgc", "ZGC: Java 21+ with lots of memory, for the shortest stutters.")
    } else if mods >= 150 {
        ("aikar", "Aikar's flags: made for big modpacks.")
    } else {
        ("balanced", "Smooth (G1): steadier frame times, safe on any Java.")
    };
    reasons.push(why.into());

    let graalvm = java_major >= 17;
    if graalvm {
        reasons.push("GraalVM: often runs Minecraft a little faster. If the game acts strangely, switch back to Mojang's Java.".into());
    }

    Suggestion { memory_mb, jvm_preset: jvm_preset.into(), graalvm, mods, shaders, java_major, total_memory_mb: total_mb, reasons }
}

fn gb(mb: u64) -> String {
    let v = mb as f64 / 1024.0;
    if (v - v.round()).abs() < 0.05 { format!("{}", v.round()) } else { format!("{v:.1}") }
}

/// Look at the instance and this computer, then suggest.
pub async fn suggest_for(paths: &Paths, inst: &Instance) -> Suggestion {
    let game = instances::game_dir(paths, &inst.id);
    let mods = count_files(&game.join("mods"), |n| n.ends_with(".jar"));
    let shaders = count_files(&game.join("shaderpacks"), |n| n.ends_with(".zip") || !n.contains('.')) > 0;
    let java_major = java_needed(paths, inst);
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    suggest(mods, shaders, sys.total_memory() / 1024 / 1024, java_major)
}

fn count_files(dir: &Path, keep: impl Fn(&str) -> bool) -> usize {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| keep(&e.file_name().to_string_lossy().to_lowercase()))
        .count()
}

/// The Java version the game asks for: from its installed version file, or
/// worked out from the Minecraft version if it isn't installed yet.
fn java_needed(paths: &Paths, inst: &Instance) -> u32 {
    let from_file = std::fs::read_to_string(paths.version_json(&inst.game_version))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("javaVersion")?.get("majorVersion")?.as_u64());
    if let Some(m) = from_file {
        return m as u32;
    }
    let parts: Vec<u32> = inst.game_version.split('.').filter_map(|p| p.parse().ok()).collect();
    match parts.as_slice() {
        // Year-numbered versions (26.1 and on).
        [major, ..] if *major >= 26 => 25,
        [1, minor, patch, ..] if *minor == 20 && *patch >= 5 => 21,
        [1, minor, ..] if *minor >= 21 => 21,
        [1, minor, ..] if *minor >= 18 => 17,
        [1, 17, ..] => 16,
        _ => 8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_memory_with_mods() {
        assert_eq!(suggest(0, false, 32768, 21).memory_mb, 3072);
        assert_eq!(suggest(0, false, 32768, 8).memory_mb, 2048);
        assert_eq!(suggest(30, false, 32768, 21).memory_mb, 4096);
        assert_eq!(suggest(120, false, 32768, 21).memory_mb, 6144);
        assert_eq!(suggest(250, true, 32768, 17).memory_mb, 9216);
        assert_eq!(suggest(450, false, 32768, 21).memory_mb, 10240);
    }

    #[test]
    fn leaves_room_on_small_computers() {
        // 8 GB computer: 4 GB stays free.
        let s = suggest(300, false, 8192, 17);
        assert_eq!(s.memory_mb, 4096);
        assert!(s.reasons.iter().any(|r| r.contains("8 GB")));
        // Never below 2 GB, even on a tiny machine.
        assert_eq!(suggest(300, false, 4096, 17).memory_mb, 2048);
    }

    #[test]
    fn picks_flags_and_java() {
        assert_eq!(suggest(0, false, 16384, 21).jvm_preset, "balanced");
        assert_eq!(suggest(200, false, 16384, 17).jvm_preset, "aikar");
        assert_eq!(suggest(200, false, 32768, 21).jvm_preset, "zgc");
        assert!(suggest(200, false, 32768, 21).graalvm);
        assert!(!suggest(200, false, 32768, 8).graalvm, "no GraalVM for Java 8 games");
        assert!(suggest(0, false, 16384, 21).graalvm, "vanilla on modern Java too");
    }
}
