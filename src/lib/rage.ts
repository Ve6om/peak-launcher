// The rage button: after a launch fails or the game crashes, shake Peak's
// window violently for five seconds. Purely for the feelings.
import { isTauri } from './api';

const RAGE_MS = 5000;
let raging = false;

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Shake the window (or, if it's maximized or can't move, the page inside it). */
export async function rage() {
  if (raging) return;
  raging = true;
  try {
    if (!(isTauri && (await shakeWindow()))) await shakePage();
  } finally {
    raging = false;
  }
}

// Move the real window around its spot, then put it back exactly.
async function shakeWindow(): Promise<boolean> {
  try {
    const { getCurrentWindow, PhysicalPosition } = await import('@tauri-apps/api/window');
    const win = getCurrentWindow();
    if ((await win.isMaximized()) || (await win.isFullscreen())) return false;
    const home = await win.outerPosition();
    const end = Date.now() + RAGE_MS;
    try {
      while (Date.now() < end) {
        // Hardest at the start, calming down towards the end.
        const left = (end - Date.now()) / RAGE_MS;
        const amp = Math.round(8 + 34 * left);
        const dx = Math.round((Math.random() * 2 - 1) * amp);
        const dy = Math.round((Math.random() * 2 - 1) * amp);
        await win.setPosition(new PhysicalPosition(home.x + dx, home.y + dy));
        await sleep(16);
      }
    } finally {
      await win.setPosition(new PhysicalPosition(home.x, home.y));
    }
    return true;
  } catch {
    return false;
  }
}

// Fallback: shake everything inside the window with CSS.
async function shakePage() {
  const root = document.documentElement;
  root.classList.add('raging');
  await sleep(RAGE_MS);
  root.classList.remove('raging');
}
