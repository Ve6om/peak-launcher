//! Other Java builds a player can choose instead of Mojang's: Eclipse Temurin
//! and GraalVM. Nothing is downloaded until a game that uses one is launched,
//! and only the one Java version that game needs.

use super::net::{self, Download};
use super::paths::Paths;
use crate::error::{Error, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const VENDORS: &[&str] = &["temurin", "graalvm"];

pub fn is_vendor(v: &str) -> bool {
    VENDORS.contains(&v)
}

/// The Java version of `vendor` to run a game that asks for `needed`: the
/// first one it publishes at or above it. None when it has nothing suitable
/// (GraalVM doesn't exist for Java 8).
pub fn major_for(vendor: &str, needed: u32) -> Option<u32> {
    let offered: &[u32] = match vendor {
        "temurin" => &[8, 11, 17, 21, 25],
        "graalvm" => &[17, 21, 25],
        _ => &[],
    };
    // Java 8 games won't run on anything newer, so never move them up.
    offered.iter().copied().find(|&m| m >= needed && (needed > 8 || m == 8))
}

fn dir(paths: &Paths, vendor: &str, major: u32) -> PathBuf {
    paths.runtimes().join(format!("{vendor}-{major}"))
}

const MARKER: &str = ".peak-java";

/// The java binary, if this build is already downloaded.
pub fn installed(paths: &Paths, vendor: &str, major: u32) -> Option<PathBuf> {
    let d = dir(paths, vendor, major);
    let rel = std::fs::read_to_string(d.join(MARKER)).ok()?;
    let exe = d.join(rel.trim());
    exe.exists().then_some(exe)
}

struct Archive {
    url: String,
    sha256: String,
    zip: bool,
}

fn os_arch() -> (&'static str, &'static str) {
    let os = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x64" };
    (os, arch)
}

async fn find_archive(vendor: &str, major: u32) -> Result<Archive> {
    let (os, arch) = os_arch();
    let zip = cfg!(windows);
    match vendor {
        "temurin" => {
            #[derive(Deserialize)]
            struct Asset {
                binary: Binary,
            }
            #[derive(Deserialize)]
            struct Binary {
                package: Package,
            }
            #[derive(Deserialize)]
            struct Package {
                link: String,
                checksum: String,
            }
            let url = format!(
                "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture={arch}&image_type=jre&os={os}&vendor=eclipse"
            );
            let assets: Vec<Asset> = net::get_json(&url).await?;
            let p = assets
                .into_iter()
                .next()
                .ok_or_else(|| Error::msg(format!("Temurin doesn't make Java {major} for this computer")))?
                .binary
                .package;
            Ok(Archive { zip: p.link.ends_with(".zip"), url: p.link, sha256: p.checksum })
        }
        "graalvm" => {
            let os = if os == "mac" { "macos" } else { os };
            let ext = if zip { "zip" } else { "tar.gz" };
            // GraalVM for Java 17 is the last community release; 21 and later
            // come from Oracle's free GraalVM downloads.
            let url = if major == 17 {
                format!("https://github.com/graalvm/graalvm-ce-builds/releases/download/jdk-17.0.9/graalvm-community-jdk-17.0.9_{os}-{arch}_bin.{ext}")
            } else {
                format!("https://download.oracle.com/graalvm/{major}/latest/graalvm-jdk-{major}_{os}-{arch}_bin.{ext}")
            };
            let res = net::client().get(format!("{url}.sha256")).send().await?;
            if !res.status().is_success() {
                return Err(Error::msg(format!("GraalVM doesn't make Java {major} for this computer")));
            }
            let text = res.text().await?;
            let sha256 = text.split_whitespace().next().unwrap_or_default().to_lowercase();
            if sha256.len() != 64 {
                return Err(Error::msg("GraalVM's download checksum couldn't be read"));
            }
            Ok(Archive { url, sha256, zip })
        }
        _ => Err(Error::msg(format!("Unknown Java build '{vendor}'"))),
    }
}

/// Download (if needed) `vendor`'s Java `major` and return its java binary.
pub async fn ensure<F>(paths: &Paths, vendor: &str, major: u32, on_progress: F) -> Result<PathBuf>
where
    F: Fn(net::Progress) + Send + Sync + 'static,
{
    if let Some(exe) = installed(paths, vendor, major) {
        return Ok(exe);
    }
    let archive = find_archive(vendor, major).await?;
    let target = dir(paths, vendor, major);
    // Unpacked straight into its folder; the marker written last is what makes
    // it count, so a half-finished one is never used.
    let file = paths.runtimes().join(format!(".{vendor}-{major}.{}", if archive.zip { "zip" } else { "tar.gz" }));
    let _ = tokio::fs::remove_dir_all(&target).await;
    tokio::fs::create_dir_all(&target).await?;

    net::download_all(vec![Download::new(archive.url.clone(), file.clone(), None, None)], 1, on_progress)
        .await
        .map_err(|e| Error::msg(format!("Couldn't download {vendor} Java {major}: {e}")))?;
    let result = async {
        if sha256_file(&file).await? != archive.sha256.to_lowercase() {
            return Err(Error::msg("The Java download was damaged (checksum mismatch). Try again."));
        }
        let (file2, target2, zip) = (file.clone(), target.clone(), archive.zip);
        tokio::task::spawn_blocking(move || extract(&file2, &target2, zip))
            .await
            .map_err(|e| Error::msg(e.to_string()))?
            .map_err(|e| Error::msg(format!("Couldn't unpack {vendor} Java {major}: {e}")))?;
        let exe = find_java(&target).ok_or_else(|| Error::msg("The Java download didn't contain java"))?;
        let rel = exe.strip_prefix(&target).map_err(|e| Error::msg(e.to_string()))?.to_path_buf();
        tokio::fs::write(target.join(MARKER), rel.to_string_lossy().replace('\\', "/")).await?;
        Ok(exe)
    }
    .await;
    let _ = tokio::fs::remove_file(&file).await;
    if result.is_err() {
        let _ = tokio::fs::remove_dir_all(&target).await;
    }
    result
}

async fn sha256_file(path: &Path) -> Result<String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<String> {
        let mut f = std::fs::File::open(&path)?;
        let mut h = Sha256::new();
        std::io::copy(&mut f, &mut h)?;
        Ok(format!("{:x}", h.finalize()))
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))?
}

fn extract(file: &Path, into: &Path, zip: bool) -> Result<()> {
    let f = std::fs::File::open(file)?;
    if zip {
        let mut z = zip::ZipArchive::new(f).map_err(|e| Error::msg(e.to_string()))?;
        z.extract(into).map_err(|e| Error::msg(e.to_string()))?;
    } else {
        let mut t = tar::Archive::new(flate2::read::GzDecoder::new(f));
        t.set_preserve_permissions(true);
        t.unpack(into)?;
    }
    Ok(())
}

/// The java binary inside an unpacked build (its folder layout differs by
/// vendor and platform; on macOS it's inside Contents/Home).
fn find_java(root: &Path) -> Option<PathBuf> {
    let name = if cfg!(windows) { "javaw.exe" } else { "java" };
    let mut stack = vec![(root.to_path_buf(), 0)];
    while let Some((d, depth)) = stack.pop() {
        let candidate = d.join("bin").join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        if depth < 4 {
            for e in std::fs::read_dir(&d).ok()?.flatten() {
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    stack.push((e.path(), depth + 1));
                }
            }
        }
    }
    None
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledRuntime {
    /// Folder name under runtimes/ (what to pass back to delete it).
    pub id: String,
    /// "Mojang", "Temurin" or "GraalVM".
    pub vendor: String,
    pub label: String,
    pub size_bytes: u64,
}

/// Every Java Peak has downloaded, with its size on disk.
pub async fn list_installed(paths: &Paths) -> Vec<InstalledRuntime> {
    let root = paths.runtimes();
    tokio::task::spawn_blocking(move || {
        let mut out = Vec::new();
        for e in std::fs::read_dir(&root).into_iter().flatten().flatten() {
            let id = e.file_name().to_string_lossy().into_owned();
            if id.starts_with('.') || !e.path().is_dir() {
                continue;
            }
            let (vendor, label) = describe(&id);
            out.push(InstalledRuntime { size_bytes: dir_size(&e.path()), id, vendor, label });
        }
        out.sort_by(|a, b| a.vendor.cmp(&b.vendor).then(a.label.cmp(&b.label)));
        out
    })
    .await
    .unwrap_or_default()
}

fn describe(id: &str) -> (String, String) {
    if let Some((vendor, major)) = id.split_once('-').filter(|(v, _)| is_vendor(v)) {
        let name = if vendor == "temurin" { "Temurin" } else { "GraalVM" };
        return (name.into(), format!("{name} · Java {major}"));
    }
    // Mojang's runtimes are named after Greek letters; show the Java version too.
    let major = match id {
        "jre-legacy" => "8",
        "java-runtime-alpha" => "16",
        "java-runtime-beta" | "java-runtime-gamma" | "java-runtime-gamma-snapshot" => "17",
        "java-runtime-delta" => "21",
        "java-runtime-epsilon" => "25",
        _ => "",
    };
    let label = if major.is_empty() { id.to_string() } else { format!("Java {major} ({id})") };
    ("Mojang".into(), label)
}

fn dir_size(p: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![p.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(t) if t.is_file() => total += e.metadata().map(|m| m.len()).unwrap_or(0),
                _ => {}
            }
        }
    }
    total
}

/// Delete one downloaded Java. Peak downloads it again if a game needs it.
pub async fn remove(paths: &Paths, id: &str) -> Result<()> {
    if id.is_empty() || id.starts_with('.') || id.contains(['/', '\\']) || id.contains("..") {
        return Err(Error::msg("That isn't one of Peak's Java folders"));
    }
    let d = paths.runtimes().join(id);
    if d.is_dir() {
        tokio::fs::remove_dir_all(&d).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_nearest_published_java() {
        assert_eq!(major_for("temurin", 8), Some(8));
        assert_eq!(major_for("temurin", 16), Some(17));
        assert_eq!(major_for("temurin", 21), Some(21));
        assert_eq!(major_for("graalvm", 17), Some(17));
        assert_eq!(major_for("graalvm", 8), None, "no GraalVM for Java 8 games");
        assert_eq!(major_for("graalvm", 25), Some(25));
        assert_eq!(major_for("graalvm", 26), None);
        assert_eq!(major_for("other", 17), None);
    }

    /// Downloads and unpacks a real build (about 50 MB for Temurin's JRE):
    /// cargo test --lib -- --ignored --nocapture downloads_and_unpacks
    #[tokio::test]
    #[ignore]
    async fn downloads_and_unpacks() {
        let vendor = std::env::var("PEAK_JAVA_VENDOR").unwrap_or_else(|_| "temurin".into());
        let major: u32 = std::env::var("PEAK_JAVA_MAJOR").ok().and_then(|m| m.parse().ok()).unwrap_or(21);
        let root = std::env::temp_dir().join(format!("peak-distro-test-{vendor}-{major}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("runtimes")).unwrap();
        let paths = Paths::new(root.clone());
        let exe = ensure(&paths, &vendor, major, |_| {}).await.unwrap();
        println!("{}", exe.display());
        assert!(exe.exists());
        assert_eq!(installed(&paths, &vendor, major), Some(exe.clone()));
        assert_eq!(super::super::jvmflags::java_major(&exe), Some(major));
        let listed = list_installed(&paths).await;
        assert_eq!(listed.len(), 1, "{listed:?}");
        println!("{} MB", listed[0].size_bytes / 1_000_000);
        remove(&paths, &listed[0].id).await.unwrap();
        assert!(installed(&paths, &vendor, major).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn names_runtimes_for_people() {
        assert_eq!(describe("graalvm-21").1, "GraalVM · Java 21");
        assert_eq!(describe("java-runtime-delta").1, "Java 21 (java-runtime-delta)");
        assert_eq!(describe("temurin-8").0, "Temurin");
    }
}
