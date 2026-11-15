//! What the launcher window does while a game runs: get out of the way once
//! the game's window is up, sit in the tray if hidden, and come back when the
//! game closes or crashes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

const TRAY: &str = "peak";

/// Give up waiting for the game's window after this long and act anyway.
const WINDOW_TIMEOUT: Duration = Duration::from_secs(120);

/// Log lines printed around when the game window opens, used where the
/// window itself can't be seen (other platforms, wrapper commands).
pub fn looks_like_window_opened(line: &str) -> bool {
    line.contains("LWJGL")
        || line.contains("Backend library:")
        || line.contains("ImmediateWindowProvider")
        || line.contains("EARLYDISPLAY")
        || line.contains("Sound engine started")
}

/// Create the tray icon (hidden until Peak hides itself).
pub fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Peak", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Peak", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &PredefinedMenuItem::separator(app)?, &quit])?;
    let mut builder = TrayIconBuilder::with_id(TRAY)
        .tooltip("Peak Launcher")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "show" => show_main(app),
            "quit" => {
                // Let the window ask about running games before quitting.
                show_main(app);
                let _ = app.emit("tray-quit", ());
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let tray = builder.build(app)?;
    tray.set_visible(false)?;
    Ok(())
}

fn set_tray(app: &AppHandle, visible: bool) {
    if let Some(t) = app.tray_by_id(TRAY) {
        let _ = t.set_visible(visible);
    }
}

/// Bring the launcher back to the front.
pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
    set_tray(app, false);
}

/// Wait for the game's window, then minimize, hide or close the launcher.
pub fn after_launch(app: AppHandle, id: String, mode: String, pid: Option<u32>, window_seen: Arc<AtomicBool>) {
    if !matches!(mode.as_str(), "minimize" | "hide" | "close") {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let start = Instant::now();
        loop {
            tokio::time::sleep(Duration::from_millis(400)).await;
            let still_running = app.state::<crate::AppState>().running.lock().unwrap().contains_key(&id);
            if !still_running {
                return;
            }
            let has_window = pid.map(game_window_open).unwrap_or(false);
            if has_window || window_seen.load(Ordering::Relaxed) || start.elapsed() > WINDOW_TIMEOUT {
                break;
            }
        }
        match mode.as_str() {
            "minimize" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.minimize();
                }
            }
            "hide" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
                set_tray(&app, true);
            }
            // The game keeps running on its own.
            "close" => app.exit(0),
            _ => {}
        }
    });
}

/// After a game exits: bring the launcher back if it stepped aside for it.
pub fn after_exit(app: &AppHandle, mode: &str) {
    if matches!(mode, "minimize" | "hide") {
        show_main(app);
    }
}

/// Whether the process has a visible top-level window (Windows only).
#[cfg(windows)]
fn game_window_open(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible};

    struct Search {
        pid: u32,
        found: bool,
    }
    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: lparam is the &mut Search passed to EnumWindows below, which
        // outlives the enumeration; hwnd comes from EnumWindows.
        unsafe {
            let s = &mut *(lparam as *mut Search);
            let mut owner = 0u32;
            GetWindowThreadProcessId(hwnd, &mut owner);
            // Titled and visible: skips the hidden helper windows Java creates.
            if owner == s.pid && IsWindowVisible(hwnd) != 0 && GetWindowTextLengthW(hwnd) > 0 {
                s.found = true;
                return 0;
            }
        }
        1
    }
    let mut s = Search { pid, found: false };
    unsafe {
        EnumWindows(Some(visit), &mut s as *mut Search as LPARAM);
    }
    s.found
}

#[cfg(not(windows))]
fn game_window_open(_pid: u32) -> bool {
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn spots_window_log_lines() {
        assert!(super::looks_like_window_opened("[Render thread/INFO]: Backend library: LWJGL version 3.3.3"));
        assert!(super::looks_like_window_opened("[Client thread/INFO] [minecraft/Minecraft]: LWJGL Version: 2.9.4"));
        assert!(!super::looks_like_window_opened("[main/INFO]: Loading tweak class name"));
    }
}

#[cfg(all(test, windows))]
mod live {
    /// Opens Character Map and checks its window is found by process id:
    /// cargo test --lib -- --ignored --nocapture finds_real_window
    #[test]
    #[ignore]
    fn finds_real_window() {
        let mut child = std::process::Command::new("charmap.exe").spawn().unwrap();
        let pid = child.id();
        let mut found = false;
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(250));
            if super::game_window_open(pid) {
                found = true;
                break;
            }
        }
        let _ = child.kill();
        assert!(found, "no window found for pid {pid}");
        assert!(!super::game_window_open(std::process::id()), "test process has no window");
    }
}
