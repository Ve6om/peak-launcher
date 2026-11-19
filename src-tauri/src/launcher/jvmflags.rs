//! Java flag presets. None is the default; the others are well-known sets
//! that help with modded clients. A preset turns into flags at launch, for the
//! Java version actually being used, so a set that needs newer Java can't
//! stop an older Minecraft (on Java 8) from starting.
//!
//! Every set was checked to start cleanly on Java 8, 17, 21 and 25.

use serde::Serialize;
use std::path::Path;

/// Peak's own tuned G1 set (the launcher's default before presets existed).
const BALANCED: &[&str] = &[
    "-XX:+UnlockExperimentalVMOptions",
    "-XX:+UseG1GC",
    "-XX:G1NewSizePercent=20",
    "-XX:G1ReservePercent=20",
    "-XX:MaxGCPauseMillis=50",
    "-XX:G1HeapRegionSize=32M",
];

/// Aikar's flags (written for Paper servers, widely used for big modpacks),
/// without -XX:+AlwaysPreTouch, which slows a client's start-up.
const AIKAR: &[&str] = &[
    "-XX:+UseG1GC",
    "-XX:+ParallelRefProcEnabled",
    "-XX:MaxGCPauseMillis=200",
    "-XX:+UnlockExperimentalVMOptions",
    "-XX:+DisableExplicitGC",
    "-XX:G1NewSizePercent=30",
    "-XX:G1MaxNewSizePercent=40",
    "-XX:G1HeapRegionSize=8M",
    "-XX:G1ReservePercent=20",
    "-XX:G1HeapWastePercent=5",
    "-XX:G1MixedGCCountTarget=4",
    "-XX:InitiatingHeapOccupancyPercent=15",
    "-XX:G1MixedGCLiveThresholdPercent=90",
    "-XX:G1RSetUpdatingPauseTimePercent=5",
    "-XX:SurvivorRatio=32",
    "-XX:+PerfDisableSharedMem",
    "-XX:MaxTenuringThreshold=1",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// The flags as used on current Java, for showing in the UI.
    pub flags: String,
}

pub fn presets() -> Vec<Preset> {
    vec![
        Preset { id: "none", name: "None", description: "Java's own defaults. A good choice for most players and vanilla.", flags: String::new() },
        Preset {
            id: "balanced",
            name: "Smooth (G1)",
            description: "Short garbage collection pauses for steadier frame times. Works on every Java version.",
            flags: BALANCED.join(" "),
        },
        Preset {
            id: "aikar",
            name: "Aikar's flags",
            description: "A popular G1 set for large modpacks with lots of memory. Works on every Java version.",
            flags: AIKAR.join(" "),
        },
        Preset {
            id: "zgc",
            name: "ZGC (low latency)",
            description: "Very short pauses on newer Java (17+). Uses more memory; best with 8 GB or more. Older Minecraft on Java 8 falls back to Smooth (G1).",
            flags: flags_for("zgc", Some(25)).join(" "),
        },
    ]
}

/// The flags for a preset on a given Java version (None = unknown, pick what's safe everywhere).
pub fn flags_for(preset: &str, java_major: Option<u32>) -> Vec<String> {
    let set: Vec<&str> = match preset {
        "balanced" => BALANCED.to_vec(),
        "aikar" => AIKAR.to_vec(),
        "zgc" => match java_major {
            // ZGC exists from Java 15; generational ZGC is opt-in on 21-23 and the only kind from 24.
            Some(m) if (21..=23).contains(&m) => vec!["-XX:+UseZGC", "-XX:+ZGenerational"],
            Some(m) if m >= 15 => vec!["-XX:+UseZGC"],
            _ => BALANCED.to_vec(),
        },
        _ => Vec::new(),
    };
    set.into_iter().map(String::from).collect()
}

pub fn is_known(preset: &str) -> bool {
    matches!(preset, "none" | "balanced" | "aikar" | "zgc")
}

/// Java's feature version from its `release` file ("1.8.0_51" -> 8, "21.0.7" -> 21).
pub fn java_major(java_exe: &Path) -> Option<u32> {
    let home = java_exe.parent()?.parent()?;
    let text = std::fs::read_to_string(home.join("release")).ok()?;
    let version = text.lines().find_map(|l| l.strip_prefix("JAVA_VERSION="))?.trim().trim_matches('"').to_string();
    parse_major(&version)
}

fn parse_major(version: &str) -> Option<u32> {
    let mut parts = version.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 { parts.next()?.parse().ok() } else { Some(first) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_java_versions() {
        assert_eq!(parse_major("1.8.0_51"), Some(8));
        assert_eq!(parse_major("17.0.15"), Some(17));
        assert_eq!(parse_major("21"), Some(21));
        assert_eq!(parse_major("25.0.1+8-LTS"), Some(25));
        assert_eq!(parse_major("garbage"), None);
    }

    #[test]
    fn zgc_adapts_to_the_java_version() {
        assert_eq!(flags_for("zgc", Some(8)), BALANCED.iter().map(|s| s.to_string()).collect::<Vec<_>>(), "Java 8 has no ZGC");
        assert_eq!(flags_for("zgc", Some(17)), ["-XX:+UseZGC"]);
        assert_eq!(flags_for("zgc", Some(21)), ["-XX:+UseZGC", "-XX:+ZGenerational"]);
        assert_eq!(flags_for("zgc", Some(25)), ["-XX:+UseZGC"], "generational is the only ZGC from 24");
        assert_eq!(flags_for("zgc", None), BALANCED.iter().map(|s| s.to_string()).collect::<Vec<_>>(), "unknown Java: safe set");
    }

    #[test]
    fn none_and_unknown_presets_add_nothing() {
        assert!(flags_for("none", Some(21)).is_empty());
        assert!(flags_for("made-up", Some(21)).is_empty());
        assert!(!is_known("made-up"));
        assert_eq!(presets()[0].id, "none");
    }

    /// Start every preset on every Java runtime Peak has installed:
    /// cargo test --lib -- --ignored --nocapture jvm_presets_live
    #[test]
    #[ignore]
    fn jvm_presets_live() {
        let root = std::path::PathBuf::from(std::env::var("APPDATA").unwrap()).join("PeakLauncher").join("runtimes");
        for rt in std::fs::read_dir(&root).unwrap().flatten() {
            let exe = rt.path().join("bin").join(if cfg!(windows) { "java.exe" } else { "java" });
            let major = java_major(&exe);
            for p in ["none", "balanced", "aikar", "zgc"] {
                let out = std::process::Command::new(&exe).arg("-Xmx1G").args(flags_for(p, major)).arg("-version").output().unwrap();
                let err = String::from_utf8_lossy(&out.stderr);
                let warned = err.lines().any(|l| l.contains("warning") || l.contains("Ignoring"));
                println!("{:<24} Java {:?}  {p:<9} ok={} warned={warned}", rt.file_name().to_string_lossy(), major, out.status.success());
                assert!(out.status.success() && !warned, "{p} on {:?}: {err}", rt.path());
            }
        }
    }
}
