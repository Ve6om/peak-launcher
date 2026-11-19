// Linux AppImage: the applications-menu entry, shared by the start-up offer
// and the switch in Settings.
import { api } from './api';
import type { MenuStatus } from './types';

export const menu = $state<{ status: MenuStatus | null; busy: boolean }>({ status: null, busy: false });

export async function loadMenuStatus() {
  try {
    menu.status = await api.appimageMenuStatus();
  } catch {
    menu.status = null;
  }
}

/** Add or update the entry (true when it worked). */
export async function addToMenu(): Promise<boolean> {
  menu.busy = true;
  try {
    menu.status = await api.appimageMenuInstall();
    return true;
  } finally {
    menu.busy = false;
  }
}

export async function removeFromMenu() {
  menu.busy = true;
  try {
    menu.status = await api.appimageMenuRemove();
  } finally {
    menu.busy = false;
  }
}
