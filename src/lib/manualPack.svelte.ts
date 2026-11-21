// CurseForge packs whose author turned off launcher downloads: instead of
// giving up, open the file's page in the browser, watch the Downloads folder,
// and install the zip as soon as it's there (keeping the pack's details).
import { api } from './api';
import { refreshInstances, toast, errorText, go } from './state.svelte';
import type { Instance, ManualPack } from './types';

export const manualPack = $state<{
  current: { info: ManualPack; projectId: string; then?: (inst: Instance) => void } | null;
}>({ current: null });

/**
 * Install a modpack. If CurseForge won't let launchers download it, opens
 * the "download it yourself" dialog instead and returns null (the dialog
 * installs it and calls `then` once the file arrives).
 */
export async function installPack(
  opts: { projectId: string; versionId?: string | null },
  then?: (inst: Instance) => void
): Promise<Instance | null> {
  try {
    return await api.modpackInstall(opts);
  } catch (e) {
    if (opts.projectId.startsWith('cf:')) {
      const info = await api.modpackManualInfo(opts.projectId, opts.versionId ?? null).catch(() => null);
      if (info) {
        manualPack.current = { info, projectId: opts.projectId, then };
        return null;
      }
    }
    throw e;
  }
}

/** Install the file the player downloaded (from the dialog). */
export async function installDownloaded(path: string): Promise<boolean> {
  const cur = manualPack.current;
  if (!cur) return false;
  try {
    const inst = await api.modpackInstall({ projectId: cur.projectId, versionId: cur.info.fileId, filePath: path });
    manualPack.current = null;
    await refreshInstances();
    toast(`Installing ${inst.name}…`, 'info');
    go('instance', inst.id);
    cur.then?.(inst);
    return true;
  } catch (e) {
    toast(errorText(e), 'error');
    return false;
  }
}
