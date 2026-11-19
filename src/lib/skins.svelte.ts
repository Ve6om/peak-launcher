// Account skins, fetched once per account and shared by every place that
// shows a head or the skin preview.
import { api } from './api';
import type { Skin } from './types';

/** Skin per account id: undefined while loading, null when there's none. */
export const skins = $state<Record<string, Skin | null | undefined>>({});

const loading = new Set<string>();

/** Fetch an account's skin unless it's already known (or `refresh` asks again). */
export async function loadSkin(accountId: string, refresh = false) {
  if (loading.has(accountId) || (!refresh && skins[accountId] !== undefined)) return;
  loading.add(accountId);
  try {
    skins[accountId] = await api.accountSkin(accountId, refresh);
  } catch {
    // Offline or rate limited: show the plain avatar and try again next time.
    if (skins[accountId] === undefined) delete skins[accountId];
  } finally {
    loading.delete(accountId);
  }
}
