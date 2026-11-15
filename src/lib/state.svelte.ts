import { api, on, isTauri } from './api';
import type {
  AccountInfo,
  AppInfo,
  BlockedFile,
  BrowseKind,
  ContentKind,
  Instance,
  InstallProgress,
  LogLine,
  PlanItem,
  Settings,
  Source,
  View
} from './types';

interface Toast {
  id: number;
  kind: 'info' | 'success' | 'error';
  text: string;
}

export const app = $state({
  ready: false,
  view: 'home' as View,
  selectedId: null as string | null,
  instances: [] as Instance[],
  accounts: [] as AccountInfo[],
  settings: null as Settings | null,
  info: null as AppInfo | null,
  running: [] as string[],
  /** Instances whose console is open in its own window. */
  poppedOut: {} as Record<string, boolean>,
  progress: {} as Record<string, InstallProgress>,
  logs: {} as Record<string, LogLine[]>,
  toasts: [] as Toast[],
  creating: false,
  /** Import-from-other-launchers dialog open. */
  importing: false,
  /** Instance being exported as a modpack. */
  exporting: null as Instance | null,
  /** Instance whose manual-download dialog is open. */
  manualFor: null as string | null,
  /** "Games are still running" prompt shown when closing the launcher. */
  closePrompt: false,
  /** Background picked in Settings but not saved yet (live preview). */
  bgPreview: null as null | 'aurora' | 'contours' | 'both',
  /** Open right-click menu for an instance. */
  menu: null as { x: number; y: number; id: string } | null,
  /** Instance waiting for delete confirmation. */
  deleting: null as Instance | null,
  /** Tab to show when an instance page opens. */
  instanceTab: 'overview' as 'overview' | 'content' | 'settings' | 'console',
  /** Browse page target. */
  browse: { instanceId: null as string | null, kind: 'mod' as BrowseKind, source: 'modrinth' as Source },
  /** Modpack files whose authors block launcher downloads, per instance. */
  blocked: {} as Record<string, BlockedFile[]>
});

/** Open the Browse page for an instance and content type. */
export function browseFor(instanceId: string | null, kind: BrowseKind = 'mod') {
  app.browse = { ...app.browse, instanceId: instanceId ?? app.browse.instanceId, kind };
  app.view = 'browse';
}

/** Install a modpack from Modrinth or a .mrpack file as a new instance. */
// ---- closing the launcher ----

let allowClose = false;

/** Close the launcher, asking first if a game is still running. */
export async function requestClose() {
  if (app.running.length) {
    app.closePrompt = true;
    return;
  }
  await quitApp(false);
}

/** Quit, optionally stopping running games first. Games left running keep going. */
export async function quitApp(stopGames: boolean) {
  if (stopGames) {
    for (const id of [...app.running]) await api.stop(id).catch(() => {});
  }
  allowClose = true;
  app.closePrompt = false;
  if (isTauri) {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().destroy();
  }
}

/** Catch closes from the OS too (Alt+F4, taskbar) while games are running. */
async function guardWindowClose() {
  if (!isTauri) return;
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  await getCurrentWindow().onCloseRequested((e) => {
    if (!allowClose && app.running.length) {
      e.preventDefault();
      app.closePrompt = true;
    }
  });
}

/** Content the user is confirming (shown when a mod needs dependencies). */
export const pending = $state({
  plan: null as null | { instanceId: string; items: PlanItem[]; resolve: (installed: boolean) => void }
});

/**
 * Install a mod / resource pack / shader into an instance. If it needs other
 * projects that aren't installed yet, the user confirms them first.
 * Resolves true when something was installed.
 */
export async function requestInstall(instanceId: string, kind: ContentKind, projectId: string, versionId: string | null = null) {
  let items: PlanItem[];
  try {
    items = await api.contentPlan(instanceId, kind, projectId, versionId);
  } catch (e) {
    toast(errorText(e), 'error');
    return false;
  }
  if (items.length > 1) {
    return new Promise<boolean>((resolve) => {
      pending.plan = { instanceId, items, resolve };
    });
  }
  return installItems(instanceId, items);
}

export async function installItems(instanceId: string, items: PlanItem[]) {
  if (!items.length) return false;
  try {
    const names = await api.contentInstallItems(instanceId, items);
    toast(names.length > 1 ? `Installed ${names[0]} and ${names.length - 1} more` : `Installed ${names[0]}`, 'success');
    return true;
  } catch (e) {
    toast(errorText(e), 'error');
    return false;
  }
}

export async function installModpack(opts: { projectId?: string; versionId?: string | null; filePath?: string }) {
  try {
    const inst = await api.modpackInstall(opts);
    await refreshInstances();
    toast(`Installing ${inst.name}…`, 'info');
    go('instance', inst.id);
    return inst;
  } catch (e) {
    toast(errorText(e), 'error');
    return null;
  }
}

/** Pick a .mrpack file and install it. */
export async function importModpack() {
  const path = await api.pickModpackFile();
  if (path) await installModpack({ filePath: path });
}

export function compact(n: number) {
  return Intl.NumberFormat('en', { notation: 'compact', maximumFractionDigits: 1 }).format(n);
}

export const activeAccount = () => app.accounts.find((a) => a.active) ?? null;
export const selected = () => app.instances.find((i) => i.id === app.selectedId) ?? null;
export const isRunning = (id: string) => app.running.includes(id);
export const isBusy = (id: string) => !!app.progress[id] && app.progress[id].stage !== 'Done';

let toastId = 0;
export function toast(text: string, kind: Toast['kind'] = 'info') {
  const id = ++toastId;
  app.toasts.push({ id, kind, text });
  setTimeout(() => {
    app.toasts = app.toasts.filter((t) => t.id !== id);
  }, kind === 'error' ? 7000 : 3500);
}

export function errorText(e: unknown) {
  return typeof e === 'string' ? e : e instanceof Error ? e.message : 'Something went wrong';
}

export function go(view: View, id?: string, tab: typeof app.instanceTab = 'overview') {
  app.instanceTab = tab;
  app.view = view;
  if (id) app.selectedId = id;
}

export function openInstanceMenu(e: MouseEvent, inst: Instance) {
  e.preventDefault();
  e.stopPropagation();
  app.menu = { x: e.clientX, y: e.clientY, id: inst.id };
}

export async function duplicateInstance(inst: Instance) {
  try {
    const copy = await api.duplicateInstance(inst.id);
    await refreshInstances();
    toast(`Created ${copy.name}`, 'success');
  } catch (e) {
    toast(errorText(e), 'error');
  }
}

export async function repairInstance(inst: Instance) {
  try {
    await api.install(inst.id);
    toast(`${inst.name}: all game files verified`, 'success');
  } catch (e) {
    toast(errorText(e), 'error');
  }
}

export async function deleteInstance(inst: Instance) {
  try {
    await api.deleteInstance(inst.id);
    app.deleting = null;
    if (app.selectedId === inst.id && app.view === 'instance') go('library');
    await refreshInstances();
    toast(`Deleted ${inst.name}`);
  } catch (e) {
    toast(errorText(e), 'error');
  }
}

export async function refreshInstances() {
  app.instances = await api.instances();
}

/** Accent colours: [accent, lighter tint, text-on-accent]. Named after the mountains. */
export const ACCENTS: Record<string, [string, string, string]> = {
  glacier: ['#5cc8f0', '#a9e4fa', '#06141b'],
  alpenglow: ['#ff8a4c', '#ffc59e', '#1d0c03'],
  moss: ['#86d17a', '#c9eebf', '#0b1708'],
  dusk: ['#a596f7', '#d6cfff', '#120c2b'],
  ember: ['#ff6464', '#ffb3b3', '#2a0707']
};

/** Accent names used by earlier versions. */
const OLD_ACCENTS: Record<string, string> = { violet: 'dusk', rose: 'ember', emerald: 'moss', ocean: 'glacier', sunset: 'alpenglow' };

export function accentName(name: string) {
  return ACCENTS[name] ? name : (OLD_ACCENTS[name] ?? 'glacier');
}

export function applyAccent(name: string) {
  const [a, b, on] = ACCENTS[accentName(name)];
  const root = document.documentElement.style;
  root.setProperty('--accent', a);
  root.setProperty('--accent-2', b);
  root.setProperty('--on-accent', on);
}

export async function init() {
  const [instances, accounts, settings, info, running] = await Promise.all([
    api.instances(),
    api.accounts(),
    api.settings(),
    api.info(),
    api.running()
  ]);
  Object.assign(app, { instances, accounts, settings, info, running });
  applyAccent(settings.accent);

  await on<{ instanceId: string; name: string; files: BlockedFile[] }>('install-blocked', (e) => {
    app.blocked[e.instanceId] = e.files;
    // Ask the user to fetch them right away, like ATLauncher does.
    app.manualFor = e.instanceId;
  });
  await on<{ instanceId: string; name: string; message: string }>('install-error', (e) => {
    toast(`${e.name}: ${e.message}`, 'error');
    refreshInstances();
  });
  await on<InstallProgress>('install-progress', (p) => {
    app.progress[p.instanceId] = p;
    if (p.stage === 'Done') {
      setTimeout(() => delete app.progress[p.instanceId], 1200);
      refreshInstances();
    }
  });
  await on<LogLine & { instanceId: string }>('game-log', (l) => {
    const list = (app.logs[l.instanceId] ??= []);
    list.push({ line: l.line, stream: l.stream });
    if (list.length > 5000) list.splice(0, list.length - 5000);
  });
  await on<{ instanceId: string; open: boolean }>('console-window', (e) => {
    if (e.open) app.poppedOut[e.instanceId] = true;
    else delete app.poppedOut[e.instanceId];
  });
  // "Quit Peak" from the tray icon: same as closing the window.
  await on('tray-quit', () => requestClose());
  await on<{ instanceId: string }>('game-log-reset', (e) => {
    app.logs[e.instanceId] = [];
  });
  await on<{ instanceId: string; state: 'running' | 'stopped'; exitCode: number | null }>('game-state', (s) => {
    if (s.state === 'running') {
      if (!app.running.includes(s.instanceId)) app.running.push(s.instanceId);
    } else {
      app.running = app.running.filter((r) => r !== s.instanceId);
      if (s.exitCode && s.exitCode !== 0) {
        toast(`Game exited with code ${s.exitCode}. Check the console for details.`, 'error');
        // Show what went wrong (the pop-out window, if open, already does).
        if (!app.poppedOut[s.instanceId]) go('instance', s.instanceId, 'console');
      }
      refreshInstances();
    }
  });
  app.ready = true;
  guardWindowClose().catch(() => {});

  // Modpacks installed before Peak saved their artwork: fetch it once, quietly.
  for (const inst of app.instances.filter((i) => i.pack?.projectId && ((!i.pack.iconUrl && !i.pack.bannerUrl) || !i.pack.description))) {
    api
      .refreshPackArt(inst.id)
      .then((updated) => {
        app.instances = app.instances.map((i) => (i.id === updated.id ? updated : i));
      })
      .catch(() => {});
  }
}

export async function play(inst: Instance) {
  if (isRunning(inst.id)) return;
  if (!activeAccount()) {
    toast('Add an account to start playing', 'info');
    go('accounts');
    return;
  }
  app.logs[inst.id] = [];
  try {
    await api.launch(inst.id);
  } catch (e) {
    delete app.progress[inst.id];
    toast(errorText(e), 'error');
  }
}

export async function stop(inst: Instance) {
  await api.stop(inst.id);
}

// ---- formatting helpers ----

export function playtime(secs: number) {
  if (secs < 60) return secs ? `${secs}s` : 'Never played';
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return h ? `${h}h ${m}m` : `${m}m`;
}

export function ago(iso: string | null) {
  if (!iso) return 'Never';
  const s = (Date.now() - new Date(iso).getTime()) / 1000;
  if (s < 60) return 'Just now';
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  if (s < 86400 * 30) return `${Math.floor(s / 86400)}d ago`;
  return new Date(iso).toLocaleDateString();
}

/** Stable hue pair derived from a string, used for generated instance art. */
export function hues(seed: string): [number, number] {
  let h = 2166136261;
  for (const c of seed) h = Math.imul(h ^ c.charCodeAt(0), 16777619);
  const a = Math.abs(h) % 360;
  return [a, (a + 40 + (Math.abs(h >> 8) % 80)) % 360];
}
