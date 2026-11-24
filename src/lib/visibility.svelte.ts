// Whether Peak's window can be seen and is in use. Moving effects (the
// background glow, the rainbow accent) pause when it isn't, so Peak sits at
// ~0% CPU in the background or while you play.
import { isTauri } from './api';

export const view = $state({
  /** Not minimized and not hidden to the tray. */
  shown: true,
  /** The window has focus. */
  focused: true
});

let started = false;

export function watchVisibility() {
  if (started || typeof document === 'undefined') return;
  started = true;
  const update = () => {
    view.shown = !document.hidden;
    view.focused = document.hasFocus();
  };
  update();
  document.addEventListener('visibilitychange', update);
  window.addEventListener('focus', update);
  window.addEventListener('blur', update);
  if (isTauri) watchWindow();
}

// Some systems (Linux on Wayland especially) don't tell the page it was
// minimized, so ask the window itself too.
async function watchWindow() {
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    const win = getCurrentWindow();
    const check = async () => {
      try {
        const [min, visible] = await Promise.all([win.isMinimized(), win.isVisible()]);
        view.shown = !min && visible && !document.hidden;
      } catch {
        // Keep what the page said.
      }
    };
    await win.onResized(check);
    await win.onFocusChanged(({ payload }) => {
      view.focused = payload;
      check();
    });
    check();
  } catch {
    // Not available: the page events are enough.
  }
}
