//! "Add Peak to your applications menu" for the Linux AppImage.
//!
//! An AppImage is just a file, so the desktop doesn't list it until something
//! writes a menu entry for it. This writes one (and an icon) into the user's
//! own folders, following the freedesktop.org rules, pointing at wherever the
//! AppImage lives. Updates replace the AppImage in place, so the entry keeps
//! working; if the file is moved, Peak notices and offers to fix the entry.
//! The .deb and .rpm packages add their own entry; this one has a different
//! name, so the two never clash.

use crate::error::{Error, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

const ENTRY_FILE: &str = "peak-launcher-appimage.desktop";
const ICON_NAME: &str = "peak-launcher-appimage";
/// Peak's 256 px icon, built into the app.
const ICON_PNG: &[u8] = include_bytes!("../../icons/128x128@2x.png");

/// The running AppImage, if Peak was started as one (Linux only).
pub fn current_appimage() -> Option<PathBuf> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let path = PathBuf::from(std::env::var_os("APPIMAGE")?);
    path.is_file().then_some(path)
}

/// $XDG_DATA_HOME, or ~/.local/share.
fn data_home() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).filter(|d| d.is_absolute()) {
        return Some(d);
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
}

fn entry_path(data: &Path) -> PathBuf {
    data.join("applications").join(ENTRY_FILE)
}

fn icon_path(data: &Path) -> PathBuf {
    data.join("icons").join("hicolor").join("256x256").join("apps").join(format!("{ICON_NAME}.png"))
}

/// Quote a path for the Exec line (Desktop Entry spec: double quotes, with
/// `"`, `` ` ``, `$` and `\` escaped, and `%` doubled so it isn't a field code).
fn exec_quote(path: &str) -> String {
    let mut out = String::from("\"");
    for c in path.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '%' => out.push_str("%%"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A value for other keys: no line breaks (they'd start a new key).
fn plain(value: &str) -> String {
    value.replace(['\n', '\r'], " ")
}

fn desktop_entry(appimage: &Path) -> String {
    let path = appimage.to_string_lossy();
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Peak Launcher\n\
         Comment=Minecraft launcher for modpacks, mods and servers\n\
         Exec={} %U\n\
         TryExec={}\n\
         Icon={ICON_NAME}\n\
         Terminal=false\n\
         Categories=Game;\n\
         Keywords=minecraft;launcher;modpack;mods;\n\
         X-AppImage-Path={}\n",
        exec_quote(&path),
        plain(&path),
        plain(&path)
    )
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuStatus {
    /// Running as an AppImage (the only case where this applies).
    pub is_appimage: bool,
    /// A menu entry exists...
    pub installed: bool,
    /// ...and points at this AppImage (false after the file was moved).
    pub up_to_date: bool,
}

fn status_in(data: Option<&Path>, appimage: Option<&Path>) -> MenuStatus {
    let Some(appimage) = appimage else {
        return MenuStatus { is_appimage: false, installed: false, up_to_date: false };
    };
    let Some(data) = data else {
        return MenuStatus { is_appimage: true, installed: false, up_to_date: false };
    };
    let text = std::fs::read_to_string(entry_path(data)).ok();
    MenuStatus {
        is_appimage: true,
        installed: text.is_some(),
        up_to_date: text.is_some_and(|t| t == desktop_entry(appimage)),
    }
}

fn install_in(data: &Path, appimage: &Path) -> Result<()> {
    let entry = entry_path(data);
    let icon = icon_path(data);
    std::fs::create_dir_all(entry.parent().unwrap())?;
    std::fs::create_dir_all(icon.parent().unwrap())?;
    std::fs::write(&icon, ICON_PNG)?;
    std::fs::write(&entry, desktop_entry(appimage))?;
    Ok(())
}

fn remove_in(data: &Path) -> Result<()> {
    for path in [entry_path(data), icon_path(data)] {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

/// Ask the desktop to pick up the change straight away. Both tools are
/// optional; most desktops notice the new file on their own anyway.
fn refresh_menus(data: &Path) {
    let _ = std::process::Command::new("update-desktop-database").arg(data.join("applications")).status();
    let _ = std::process::Command::new("gtk-update-icon-cache").args(["-q", "-t"]).arg(data.join("icons").join("hicolor")).status();
}

pub fn status() -> MenuStatus {
    status_in(data_home().as_deref(), current_appimage().as_deref())
}

pub fn install() -> Result<MenuStatus> {
    let appimage = current_appimage().ok_or_else(|| Error::msg("This only applies when Peak runs as an AppImage."))?;
    let data = data_home().ok_or_else(|| Error::msg("Couldn't find your home folder."))?;
    install_in(&data, &appimage)?;
    refresh_menus(&data);
    Ok(status())
}

pub fn remove() -> Result<MenuStatus> {
    let data = data_home().ok_or_else(|| Error::msg("Couldn't find your home folder."))?;
    remove_in(&data)?;
    refresh_menus(&data);
    Ok(status())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("peak-appimage-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn exec_paths_are_quoted_safely() {
        assert_eq!(exec_quote("/home/a/Peak.AppImage"), "\"/home/a/Peak.AppImage\"");
        assert_eq!(exec_quote("/home/a/My Apps/Peak.AppImage"), "\"/home/a/My Apps/Peak.AppImage\"");
        assert_eq!(exec_quote("/x/$HOME`id`\"q\"\\100%"), "\"/x/\\$HOME\\`id\\`\\\"q\\\"\\\\100%%\"");
    }

    #[test]
    fn entry_points_at_the_appimage() {
        let e = desktop_entry(Path::new("/home/a/Apps/Peak Launcher.AppImage"));
        assert!(e.starts_with("[Desktop Entry]\n"));
        assert!(e.contains("Exec=\"/home/a/Apps/Peak Launcher.AppImage\" %U\n"));
        assert!(e.contains("Icon=peak-launcher-appimage\n"));
        assert!(e.contains("Categories=Game;\n"));
        // A line break in the path can't sneak in extra keys.
        let odd = desktop_entry(Path::new("/tmp/a\nExec=evil"));
        assert!(!odd.lines().any(|l| l == "Exec=evil"));
    }

    #[test]
    fn install_status_move_and_remove() {
        let data = temp("flow");
        let appimage = data.join("Peak.AppImage");
        std::fs::write(&appimage, b"").unwrap();

        let s = status_in(Some(&data), Some(&appimage));
        assert!(s.is_appimage && !s.installed && !s.up_to_date);

        install_in(&data, &appimage).unwrap();
        assert!(entry_path(&data).is_file() && icon_path(&data).is_file());
        assert!(std::fs::read(icon_path(&data)).unwrap().starts_with(b"\x89PNG"));
        let s = status_in(Some(&data), Some(&appimage));
        assert!(s.installed && s.up_to_date);

        // Moved somewhere else: the entry is there but out of date.
        let moved = data.join("elsewhere").join("Peak.AppImage");
        let s = status_in(Some(&data), Some(&moved));
        assert!(s.installed && !s.up_to_date);

        remove_in(&data).unwrap();
        assert!(!entry_path(&data).exists() && !icon_path(&data).exists());
        remove_in(&data).unwrap(); // removing twice is fine
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn not_an_appimage_means_nothing_to_do() {
        let s = status_in(Some(Path::new("/tmp")), None);
        assert!(!s.is_appimage && !s.installed);
    }
}
