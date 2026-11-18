//! Automatic Java: downloads the exact runtime Mojang ships for each game
//! version, so users never have to install or pick Java themselves.

use super::net::{self, Download};
use super::paths::Paths;
use crate::error::{Error, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const RUNTIMES_URL: &str =
    "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

/// Used for old versions whose JSON doesn't declare a Java version (Java 8).
pub const LEGACY_COMPONENT: &str = "jre-legacy";

type RuntimeIndex = HashMap<String, HashMap<String, Vec<RuntimeEntry>>>;

#[derive(Debug, Deserialize)]
struct RuntimeEntry {
    manifest: ManifestRef,
}

#[derive(Debug, Deserialize)]
struct ManifestRef {
    sha1: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct RuntimeManifest {
    files: HashMap<String, RuntimeFile>,
}

#[derive(Debug, Deserialize)]
struct RuntimeFile {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    executable: bool,
    #[serde(default)]
    downloads: Option<RuntimeDownloads>,
    #[serde(default)]
    #[allow(dead_code)]
    target: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RuntimeDownloads {
    raw: RawDownload,
}

#[derive(Debug, Deserialize)]
struct RawDownload {
    sha1: String,
    size: u64,
    url: String,
}

fn platform_keys() -> Vec<&'static str> {
    let arch = std::env::consts::ARCH;
    match (std::env::consts::OS, arch) {
        ("windows", "aarch64") => vec!["windows-arm64", "windows-x64"],
        ("windows", "x86") => vec!["windows-x86"],
        ("windows", _) => vec!["windows-x64"],
        // Apple Silicon falls back to Intel builds under Rosetta for old Java.
        ("macos", "aarch64") => vec!["mac-os-arm64", "mac-os"],
        ("macos", _) => vec!["mac-os"],
        ("linux", "x86") => vec!["linux-i386"],
        _ => vec!["linux"],
    }
}

pub fn runtime_dir(paths: &Paths, component: &str) -> PathBuf {
    paths.runtimes().join(component)
}

pub fn java_executable(paths: &Paths, component: &str) -> PathBuf {
    let dir = runtime_dir(paths, component);
    if cfg!(target_os = "windows") {
        dir.join("bin").join("javaw.exe")
    } else if cfg!(target_os = "macos") {
        dir.join("jre.bundle/Contents/Home/bin/java")
    } else {
        dir.join("bin").join("java")
    }
}

/// Make sure the runtime `component` is installed and return its java binary.
/// With `verify` false an already-complete runtime is trusted without any
/// network access, which keeps launches instant and works offline. With
/// `verify` true (installs and repairs) every file is checked against
/// Mojang's checksums and damaged ones are downloaded again.
pub async fn ensure_runtime<F>(paths: &Paths, component: &str, verify: bool, on_progress: F) -> Result<PathBuf>
where
    F: Fn(net::Progress) + Send + Sync + 'static,
{
    let dir = runtime_dir(paths, component);
    let marker = dir.join(".peak-complete");
    let exe = java_executable(paths, component);
    if !verify && exe.exists() && marker.exists() {
        return Ok(exe);
    }

    let index: RuntimeIndex = net::get_json(RUNTIMES_URL).await?;
    let entry = platform_keys()
        .into_iter()
        .filter_map(|k| index.get(k)?.get(component)?.first())
        .next()
        .ok_or_else(|| Error::msg(format!("Java runtime '{component}' isn't available for this platform")))?;

    // Not trusted until every file has been checked again.
    let _ = tokio::fs::remove_file(&marker).await;

    let manifest: RuntimeManifest = net::get_json(&entry.manifest.url).await?;
    let mut files = Vec::new();
    for (rel, f) in &manifest.files {
        let path = dir.join(rel);
        match f.kind.as_str() {
            "directory" => tokio::fs::create_dir_all(&path).await?,
            "file" => {
                if let Some(d) = &f.downloads {
                    let mut dl = Download::new(d.raw.url.clone(), path, Some(d.raw.sha1.clone()), Some(d.raw.size));
                    dl.executable = f.executable;
                    files.push(dl);
                }
            }
            _ => {}
        }
    }
    net::download_all(files, net::concurrency(), on_progress).await?;

    #[cfg(unix)]
    for (rel, f) in &manifest.files {
        if f.kind == "link" {
            if let Some(target) = &f.target {
                let link = dir.join(rel);
                let _ = tokio::fs::remove_file(&link).await;
                let _ = tokio::fs::symlink(target, &link).await;
            }
        }
    }

    tokio::fs::write(&marker, &entry.manifest.sha1).await?;
    if !exe.exists() {
        return Err(Error::msg("Java was downloaded but the executable is missing"));
    }
    Ok(exe)
}

/// Console (non-`w`) variant of a java binary, for tools whose output we read.
pub fn console_java(exe: &Path) -> PathBuf {
    if cfg!(windows) && exe.file_name().map(|n| n.eq_ignore_ascii_case("javaw.exe")).unwrap_or(false) {
        exe.with_file_name("java.exe")
    } else {
        exe.to_path_buf()
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOption {
    pub component: String,
    pub major: u32,
    pub installed: bool,
}

/// Mojang-provided Java runtimes available for this platform.
pub async fn available_runtimes(paths: &Paths) -> Result<Vec<RuntimeOption>> {
    #[derive(Deserialize)]
    struct Entry {
        version: Ver,
    }
    #[derive(Deserialize)]
    struct Ver {
        name: String,
    }
    let index: HashMap<String, HashMap<String, Vec<Entry>>> = net::get_json(RUNTIMES_URL).await?;
    let mut out: Vec<RuntimeOption> = Vec::new();
    for key in platform_keys() {
        let Some(comps) = index.get(key) else { continue };
        for (component, entries) in comps {
            let Some(e) = entries.first() else { continue };
            if out.iter().any(|o| &o.component == component) || component.contains("minecraft-java-exe") {
                continue;
            }
            out.push(RuntimeOption {
                component: component.clone(),
                major: parse_major(&e.version.name),
                installed: java_executable(paths, component).exists(),
            });
        }
    }
    out.sort_by(|a, b| a.major.cmp(&b.major).then(a.component.cmp(&b.component)));
    Ok(out)
}

fn parse_major(v: &str) -> u32 {
    let v = v.trim_start_matches("1.");
    v.split(|c: char| !c.is_ascii_digit()).next().and_then(|s| s.parse().ok()).unwrap_or(0)
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedJava {
    pub path: String,
    pub version: String,
    pub major: u32,
}

/// Ask a java binary for its version.
pub async fn probe(exe: &Path) -> Option<DetectedJava> {
    let mut cmd = tokio::process::Command::new(console_java(exe));
    cmd.arg("-version");
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let out = tokio::time::timeout(std::time::Duration::from_secs(8), cmd.output()).await.ok()?.ok()?;
    let text = String::from_utf8_lossy(&out.stderr).to_string() + &String::from_utf8_lossy(&out.stdout);
    let version = text.split('"').nth(1)?.to_string();
    Some(DetectedJava { path: exe.to_string_lossy().into_owned(), major: parse_major(&version), version })
}

/// Find Java installations on this computer (common install folders, JAVA_HOME, PATH).
pub async fn detect(paths: &Paths) -> Vec<DetectedJava> {
    let bin = if cfg!(windows) { "javaw.exe" } else { "java" };
    let mut candidates: Vec<PathBuf> = Vec::new();
    let mut roots: Vec<PathBuf> = vec![paths.runtimes()];
    if cfg!(windows) {
        for base in [std::env::var("ProgramFiles").ok(), std::env::var("ProgramFiles(x86)").ok()].into_iter().flatten() {
            for vendor in ["Java", "Eclipse Adoptium", "Microsoft", "Zulu", "Amazon Corretto", "BellSoft", "Semeru", "AdoptOpenJDK"] {
                roots.push(PathBuf::from(&base).join(vendor));
            }
        }
    } else if cfg!(target_os = "macos") {
        roots.push(PathBuf::from("/Library/Java/JavaVirtualMachines"));
    } else {
        roots.push(PathBuf::from("/usr/lib/jvm"));
        roots.push(PathBuf::from("/usr/java"));
    }
    for root in roots {
        let Ok(rd) = std::fs::read_dir(&root) else { continue };
        for entry in rd.flatten() {
            let p = entry.path();
            for sub in ["bin", "Contents/Home/bin", "jre.bundle/Contents/Home/bin"] {
                let exe = p.join(sub).join(bin);
                if exe.exists() {
                    candidates.push(exe);
                }
            }
        }
    }
    if let Ok(home) = std::env::var("JAVA_HOME") {
        candidates.push(PathBuf::from(home).join("bin").join(bin));
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            candidates.push(dir.join(bin));
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for c in candidates {
        let Ok(canon) = std::fs::canonicalize(&c) else { continue };
        if !seen.insert(canon) {
            continue;
        }
        if let Some(found) = probe(&c).await {
            out.push(found);
        }
    }
    out.sort_by(|a, b| b.major.cmp(&a.major));
    out
}
