//! Desktop shortcuts that start one instance straight away.
//!
//! A shortcut runs Peak with `--play <instance id>`. It's a command-line
//! argument rather than a peak:// link on purpose: a web page can open a
//! link, but it can't start a program with arguments, so a site can never
//! start the game by itself.

use super::instances::Instance;
use super::paths::Paths;
use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

pub const PLAY_ARG: &str = "--play";

/// An instance id is a short slug; anything else in a shortcut is ignored.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 100 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.') && !id.starts_with('.')
}

/// The instance a shortcut asked Peak to start, from its arguments.
pub fn play_arg(args: &[String]) -> Option<String> {
    let i = args.iter().position(|a| a == PLAY_ARG)?;
    args.get(i + 1).filter(|id| valid_id(id)).cloned()
}

/// A file name from an instance name.
fn file_name(name: &str) -> String {
    let clean: String = name.chars().map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { ' ' } else { c }).collect();
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    let clean = clean.trim_matches('.').to_string();
    if clean.is_empty() { "Minecraft".into() } else { clean.chars().take(80).collect() }
}

/// An .ico file holding one 256px PNG (Windows reads PNGs inside .ico).
fn ico(png: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(22 + png.len());
    out.extend([0, 0, 1, 0, 1, 0]); // icon, one image
    out.extend([0, 0, 0, 0]); // 256 x 256, no palette
    out.extend(1u16.to_le_bytes()); // planes
    out.extend(32u16.to_le_bytes()); // bits per pixel
    out.extend((png.len() as u32).to_le_bytes());
    out.extend(22u32.to_le_bytes()); // where the image starts
    out.extend(png);
    out
}

/// The instance's picture as a square 256px PNG, if it has one.
fn icon_png(picture: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(picture).ok()?;
    let side = img.width().min(img.height());
    let square = img.crop_imm((img.width() - side) / 2, (img.height() - side) / 2, side, side);
    let filter = if side < 64 { image::imageops::FilterType::Nearest } else { image::imageops::FilterType::Lanczos3 };
    let resized = square.resize_exact(256, 256, filter);
    let mut out = std::io::Cursor::new(Vec::new());
    resized.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

/// Make a shortcut on the desktop. `picture` is the instance's logo (any
/// format), `exe` is how to start Peak. Returns where the shortcut went.
pub fn create(paths: &Paths, inst: &Instance, picture: Option<&[u8]>, exe: &Path, desktop: &Path) -> Result<PathBuf> {
    if !valid_id(&inst.id) {
        return Err(Error::msg("This instance's id can't be used in a shortcut."));
    }
    std::fs::create_dir_all(desktop)?;
    let png = picture.and_then(icon_png);
    let dir = paths.instance(&inst.id);
    let name = file_name(&inst.name);
    platform::create(&dir, &inst.id, &name, png.as_deref(), exe, desktop)
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::os::windows::process::CommandExt;

    pub fn create(dir: &Path, id: &str, name: &str, png: Option<&[u8]>, exe: &Path, desktop: &Path) -> Result<PathBuf> {
        let icon = match png {
            Some(png) => {
                let p = dir.join("shortcut.ico");
                std::fs::write(&p, ico(png))?;
                format!("{},0", p.display())
            }
            None => format!("{},0", exe.display()),
        };
        let lnk = desktop.join(format!("{name}.lnk"));
        // Windows Script Host makes the .lnk. Every value goes in through an
        // environment variable, so nothing in a name can change the script.
        let script = "$s = (New-Object -ComObject WScript.Shell).CreateShortcut($env:PEAK_LNK); \
                      $s.TargetPath = $env:PEAK_EXE; $s.Arguments = $env:PEAK_ARGS; \
                      $s.IconLocation = $env:PEAK_ICON; $s.WorkingDirectory = $env:PEAK_DIR; \
                      $s.Description = $env:PEAK_DESC; $s.Save()";
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
            .env("PEAK_LNK", &lnk)
            .env("PEAK_EXE", exe)
            .env("PEAK_ARGS", format!("{PLAY_ARG} {id}"))
            .env("PEAK_ICON", icon)
            .env("PEAK_DIR", exe.parent().unwrap_or(exe))
            .env("PEAK_DESC", format!("Play {name} in Peak Launcher"))
            .creation_flags(0x0800_0000) // no console window
            .output()
            .map_err(|e| Error::msg(format!("Couldn't make the shortcut: {e}")))?;
        if !out.status.success() || !lnk.exists() {
            return Err(Error::msg(format!("Couldn't make the shortcut. {}", String::from_utf8_lossy(&out.stderr).trim())));
        }
        Ok(lnk)
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn quote(p: &Path) -> String {
        // Desktop-entry quoting: wrap in quotes, escape the few special characters.
        let s = p.display().to_string();
        let mut out = String::from("\"");
        for c in s.chars() {
            if matches!(c, '"' | '`' | '$' | '\\') {
                out.push('\\');
            }
            out.push(c);
        }
        out.push('"');
        out.replace('%', "%%")
    }

    pub fn create(dir: &Path, id: &str, name: &str, png: Option<&[u8]>, exe: &Path, desktop: &Path) -> Result<PathBuf> {
        // An AppImage runs from a temporary mount; start the AppImage itself.
        let exe = std::env::var_os("APPIMAGE").map(PathBuf::from).unwrap_or_else(|| exe.to_path_buf());
        let icon = match png {
            Some(png) => {
                let p = dir.join("shortcut.png");
                std::fs::write(&p, png)?;
                p.display().to_string()
            }
            None => "peak-launcher".to_string(),
        };
        let entry = format!(
            "[Desktop Entry]\nType=Application\nName={}\nComment=Play {} in Peak Launcher\nExec={} {PLAY_ARG} {id}\nIcon={icon}\nTerminal=false\nCategories=Game;\n",
            name.replace('\n', " "),
            name.replace('\n', " "),
            quote(&exe),
        );
        let file = format!("peak-{id}.desktop");
        // In the app menu too, where desktops without icons look.
        if let Some(apps) = dirs_apps() {
            let _ = std::fs::create_dir_all(&apps).and_then(|_| std::fs::write(apps.join(&file), &entry));
        }
        let path = desktop.join(&file);
        std::fs::write(&path, entry)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
        Ok(path)
    }

    fn dirs_apps() -> Option<PathBuf> {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .map(|d| d.join("applications"))
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A tiny app that opens Peak with the play argument.
    pub fn create(_dir: &Path, id: &str, name: &str, _png: Option<&[u8]>, _exe: &Path, desktop: &Path) -> Result<PathBuf> {
        let app = desktop.join(format!("{name}.app"));
        let macos = app.join("Contents/MacOS");
        std::fs::create_dir_all(&macos)?;
        let plist = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n<key>CFBundleExecutable</key><string>play</string>\n<key>CFBundleIdentifier</key><string>com.peaklauncher.shortcut.{id}</string>\n<key>CFBundleName</key><string>{}</string>\n<key>CFBundlePackageType</key><string>APPL</string>\n</dict></plist>\n",
            name.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
        );
        std::fs::write(app.join("Contents/Info.plist"), plist)?;
        let script = macos.join("play");
        std::fs::write(&script, format!("#!/bin/sh\nexec open -b com.peaklauncher.app --args {PLAY_ARG} {id}\n"))?;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))?;
        Ok(app)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_play_argument() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(play_arg(&args(&["peak.exe", "--play", "survival-island"])).as_deref(), Some("survival-island"));
        assert_eq!(play_arg(&args(&["peak.exe", "--play"])), None);
        assert_eq!(play_arg(&args(&["peak.exe", "--play", "../x"])), None);
        assert_eq!(play_arg(&args(&["peak.exe", "--play", "a b"])), None);
        assert_eq!(play_arg(&args(&["peak.exe", "peak://join?s=x"])), None);
    }

    #[test]
    fn safe_file_names_and_icons() {
        assert_eq!(file_name("All the Mods: 10 / Sky?"), "All the Mods 10 Sky");
        assert_eq!(file_name("..."), "Minecraft");
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbaImage::new(16, 16).write_to(&mut png, image::ImageFormat::Png).unwrap();
        let icon = icon_png(png.get_ref()).unwrap();
        assert_eq!(image::load_from_memory(&icon).unwrap().width(), 256);
        let i = ico(&icon);
        assert_eq!(&i[..4], &[0, 0, 1, 0]);
        assert_eq!(&i[22..26], &icon[..4]);
    }
}
