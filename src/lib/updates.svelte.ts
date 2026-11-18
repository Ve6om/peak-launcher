// In-app updates. Releases are published on GitHub with a short description;
// Peak checks for them, shows the version and patch notes, and installs the
// update when the user agrees.
import { isTauri } from './api';

export interface AvailableUpdate {
  version: string;
  currentVersion: string;
  date: string | null;
  notes: string;
}

export const updates = $state({
  available: null as AvailableUpdate | null,
  /** Dialog open. */
  open: false,
  checking: false,
  installing: false,
  /** 0..1 while downloading, null when the size isn't known. */
  progress: null as number | null,
  error: null as string | null,
  /** Version the user chose "Later" for, so we don't nag every check. */
  snoozed: null as string | null
});

// The updater's handle isn't serialisable, so it stays outside the state.
let handle: { downloadAndInstall: (cb: (e: any) => void) => Promise<void> } | null = null;

const SIX_HOURS = 6 * 60 * 60 * 1000;

/** Look for an update. With `interactive`, always open the dialog (or say we're current). */
export async function checkForUpdate(interactive = false): Promise<'available' | 'current' | 'error'> {
  updates.checking = true;
  updates.error = null;
  try {
    const found = await findUpdate();
    if (!found) {
      updates.available = null;
      return 'current';
    }
    updates.available = found;
    if (interactive || updates.snoozed !== found.version) updates.open = true;
    return 'available';
  } catch (e) {
    updates.error = friendly(e instanceof Error ? e.message : String(e));
    return 'error';
  } finally {
    updates.checking = false;
  }
}

async function findUpdate(): Promise<AvailableUpdate | null> {
  if (!isTauri) {
    // Browser preview: add #demo-update to the URL to see the dialog.
    if (typeof location !== 'undefined' && location.hash === '#demo-update') {
      handle = {
        downloadAndInstall: async (cb) => {
          cb({ event: 'Started', data: { contentLength: 1000 } });
          for (let i = 0; i < 10; i++) {
            await new Promise((r) => setTimeout(r, 120));
            cb({ event: 'Progress', data: { chunkLength: 100 } });
          }
          cb({ event: 'Finished' });
        }
      };
      return {
        version: '0.2.0',
        currentVersion: '0.1.0',
        date: new Date().toISOString(),
        notes:
          '### New\n- Technic modpacks on the Modpacks page\n- Pack artwork on cards and banners\n\n### Fixed\n- Manual downloads now move files out of Downloads automatically'
      };
    }
    return null;
  }
  const { check } = await import('@tauri-apps/plugin-updater');
  const update = await check();
  if (!update) return null;
  handle = update;
  return {
    version: update.version,
    currentVersion: update.currentVersion,
    date: update.date ?? null,
    notes: update.body?.trim() || 'Bug fixes and improvements.'
  };
}

/** Plain-language versions of the updater's errors. */
function friendly(message: string): string {
  if (/platforms/i.test(message)) return 'the latest release is still being published. Try again in a few minutes.';
  if (/(404|not found)/i.test(message)) return "no release information was found. Try again later.";
  if (/(network|dns|connect|timed? ?out|offline)/i.test(message)) return "couldn't reach the update server. Check your connection.";
  return message;
}

/** Download, install and restart. */
export async function installUpdate() {
  if (!handle) return;
  updates.installing = true;
  updates.error = null;
  updates.progress = 0;
  let total = 0;
  let done = 0;
  try {
    await handle.downloadAndInstall((e) => {
      if (e.event === 'Started') {
        total = e.data?.contentLength ?? 0;
        updates.progress = total ? 0 : null;
      } else if (e.event === 'Progress') {
        done += e.data?.chunkLength ?? 0;
        if (total) updates.progress = Math.min(1, done / total);
      } else if (e.event === 'Finished') {
        updates.progress = 1;
      }
    });
    if (isTauri) {
      const { relaunch } = await import('@tauri-apps/plugin-process');
      await relaunch();
    } else {
      updates.installing = false;
      updates.open = false;
    }
  } catch (e) {
    updates.installing = false;
    updates.error = e instanceof Error ? e.message : String(e);
  }
}

export function snooze() {
  updates.snoozed = updates.available?.version ?? null;
  updates.open = false;
}

/** Check shortly after start-up, then every few hours, quietly. */
export function startUpdateChecks() {
  setTimeout(() => checkForUpdate(false), 3000);
  setInterval(() => checkForUpdate(false), SIX_HOURS);
}
