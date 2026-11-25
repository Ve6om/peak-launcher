// CurseForge packs whose author turned off launcher downloads: instead of
// giving up, open the file's page in the browser, watch the Downloads folder,
// and install the zip as soon as it's there (keeping the pack's details).
import { api } from './api';
import { refreshInstances, toast, errorText, go } from './state.svelte';
import type { Instance, ManualPack } from './types';

export const manualPack = $state<{
  current: {
    info: ManualPack;
    projectId: string;
    then?: (inst: Instance) => void;
    /** Changing this instance's pack version, rather than installing a new one. */
    changeFor?: string;
  } | null;
}>({ current: null });

/**
 * Change an instance's modpack version. If CurseForge won't let launchers
 * download that version, opens the "download it yourself" dialog instead
 * and returns null (the dialog finishes the change once the file arrives).
 */
export async function changePackVersion(inst: Instance, versionId: string, then?: (inst: Instance) => void): Promise<Instance | null> {
  try {
    return await api.modpackChangeVersion(inst.id, versionId);
  } catch (e) {
    const projectId = inst.pack?.projectId;
    if (projectId?.startsWith('cf:')) {
      const info = await api.modpackManualInfo(projectId, versionId).catch(() => null);
      if (info) {
        manualPack.current = { info, projectId, then, changeFor: inst.id };
        return null;
      }
    }
    throw e;
  }
}

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

let busy = false;

/**
 * Install the file the player downloaded (from the dialog). `found` means
 * Peak spotted it in Downloads, so it's deleted once the pack is installed.
 */
export async function installDownloaded(path: string, found = false): Promise<boolean> {
  const cur = manualPack.current;
  if (!cur || busy) return false;
  busy = true;
  try {
    const inst = cur.changeFor
      ? await api.modpackChangeVersion(cur.changeFor, cur.info.fileId, path, found)
      : await api.modpackInstall({ projectId: cur.projectId, versionId: cur.info.fileId, filePath: path, removeFile: found });
    manualPack.current = null;
    await refreshInstances();
    toast(cur.changeFor ? `Switching ${inst.name} to ${cur.info.version}…` : `Installing ${inst.name}…`, 'info');
    go('instance', inst.id);
    cur.then?.(inst);
    return true;
  } catch (e) {
    toast(errorText(e), 'error');
    return false;
  } finally {
    busy = false;
  }
}
