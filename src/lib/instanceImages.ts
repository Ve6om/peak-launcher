// An instance's logo and banner: the player's own pictures if they picked
// some, else the modpack's artwork. Own pictures live in the instance folder
// and are shown through Tauri's asset protocol.
import { isTauri } from './api';
import { app } from './state.svelte';
import type { Instance } from './types';

let toUrl: ((path: string) => string) | null = null;
if (isTauri) {
  import('@tauri-apps/api/core').then((m) => (toUrl = m.convertFileSrc)).catch(() => {});
}

function ownPicture(inst: Instance, file: string | null | undefined): string | null {
  const dir = app.info?.dataDir;
  if (!file || !dir || !toUrl) return null;
  // The file name carries a timestamp, so a new picture gets a new address.
  return toUrl(`${dir.replace(/[\\/]+$/, '')}/instances/${inst.id}/${file}`);
}

/** The square logo: the player's own, else the modpack's icon. */
export function instanceIcon(inst: Instance): string | null {
  return ownPicture(inst, inst.customIcon) ?? inst.pack?.iconUrl ?? null;
}

/** The wide banner: the player's own, else the modpack's cover art. */
export function instanceBanner(inst: Instance): string | null {
  return ownPicture(inst, inst.customBanner) ?? inst.pack?.bannerUrl ?? null;
}
