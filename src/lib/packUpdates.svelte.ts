// Modpack updates found at start-up, shared by the prompt and each
// instance's Modpack panel.
import { api } from './api';
import type { PackUpdate } from './types';

export const packUpdates = $state<{ list: PackUpdate[]; checked: boolean }>({ list: [], checked: false });

export async function checkPackUpdates() {
  try {
    packUpdates.list = await api.modpackUpdates();
  } catch {
    packUpdates.list = [];
  } finally {
    packUpdates.checked = true;
  }
}
