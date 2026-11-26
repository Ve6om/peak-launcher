import { api, on, isTauri } from './api';
import { view } from './visibility.svelte';
import type { PeakLink } from './discover.svelte';
import type {
  CrashDiagnosis,
  ModSearch,
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
  /** Offer the rage button (a launch failed or the game crashed). */
  rage?: boolean;
}

/** When each running game started (ms), to find its crash report. */
const gameStarted: Record<string, number> = {};
/** Games the crash helper has already spoken up about this run. */
const crashShown: Record<string, boolean> = {};
const crashTimers: Record<string, ReturnType<typeof setTimeout>> = {};
/** A load error was printed during this run (for a broken-mod search test). */
const loadErrorSeen: Record<string, boolean> = {};

// Errors a mod loader prints while it keeps its own error window open (Fabric's
// "Incompatible mods found!"), so the game hasn't closed yet.
const LOAD_ERROR = /Incompatible mods found|FormattedException|Mod resolution failed|Missing or unsupported mandatory dependencies|Mod loading has failed|Failed to load mod|Duplicate, conflicting mods/;

/** Ask the crash helper about a game, once per run. */
function showCrashHelp(instanceId: string, fallback?: string) {
  if (crashShown[instanceId]) return;
  crashShown[instanceId] = true;
  const since = (gameStarted[instanceId] ?? Date.now()) - 5000;
  api
    .crashDiagnose(instanceId, since)
    .then((diagnosis) => (app.crash = { instanceId, diagnosis }))
    .catch(() => fallback && toast(fallback, 'error', { rage: true }));
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
  /** "Which Java?" asked on first play; answer with true to go on playing. */
  javaPrompt: null as null | ((go: boolean) => void),
  /** Instance whose Optimize dialog is open (from its overview or right-click menu). */
  optimizing: null as Instance | null,
  /** A game that just crashed, with what the crash helper found. */
  crash: null as { instanceId: string; diagnosis: CrashDiagnosis } | null,
  /** Broken-mod searches going on, by instance id. */
  modSearches: {} as Record<string, ModSearch>,
  /** Instance whose broken-mod search dialog is open, and the answer Peak
   *  expects for the test just played (from how the game closed). */
  modSearchFor: null as { instanceId: string; guess: 'crashed' | 'worked' | null } | null,
  /** Instance whose "change modpack version" dialog is open. */
  packVersionFor: null as Instance | null,
  /** Instance being exported as a server pack. */
  serverPack: null as Instance | null,
  /** Instance whose manual-download dialog is open. */
  manualFor: null as string | null,
  /** "Games are still running" prompt shown when closing the launcher. */
  closePrompt: false,
  /** Background picked in Settings but not saved yet (live preview). */
  bgPreview: null as null | 'aurora' | 'contours' | 'both',
  /** Lighter effects switched in Settings but not saved yet. */
  lightPreview: null as null | boolean,
  /** Moving glow switched in Settings but not saved yet. */
  glowPreview: null as null | boolean,
  /** Open right-click menu for an instance. */
  menu: null as { x: number; y: number; id: string } | null,
  /** Instance waiting for delete confirmation. */
  deleting: null as Instance | null,
  /** Tab to show when an instance page opens. */
  instanceTab: 'overview' as 'overview' | 'content' | 'servers' | 'settings' | 'console',
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
    if (names.length) toast(names.length > 1 ? `Installed ${names[0]} and ${names.length - 1} more` : `Installed ${names[0]}`, 'success');
    // Files launchers aren't allowed to download: the browser download window.
    const byHand = items.filter((i) => i.manual);
    if (byHand.length) {
      toast(`${byHand.map((i) => i.title).join(', ')} ${byHand.length === 1 ? 'has' : 'have'} to be downloaded in your browser. Peak moves ${byHand.length === 1 ? 'it' : 'them'} into place.`, 'info');
      app.manualFor = instanceId;
    }
    return names.length > 0 || byHand.length > 0;
  } catch (e) {
    toast(errorText(e), 'error');
    return false;
  }
}

export async function installModpack(opts: { projectId?: string; versionId?: string | null; filePath?: string }) {
  try {
    // CurseForge packs that launchers can't download open the "download it
    // yourself" dialog, which installs the pack once the file arrives.
    const { installPack } = await import('./manualPack.svelte');
    const inst = opts.projectId && !opts.filePath ? await installPack({ projectId: opts.projectId, versionId: opts.versionId }) : await api.modpackInstall(opts);
    if (!inst) return null;
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
export function toast(text: string, kind: Toast['kind'] = 'info', opts: { rage?: boolean } = {}) {
  const id = ++toastId;
  app.toasts.push({ id, kind, text, rage: opts.rage });
  setTimeout(() => {
    app.toasts = app.toasts.filter((t) => t.id !== id);
  }, opts.rage ? 12000 : kind === 'error' ? 7000 : 3500);
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
  tarn: ['#3fd6b4', '#a8f0de', '#04170f'],
  moss: ['#86d17a', '#c9eebf', '#0b1708'],
  larch: ['#f2c14e', '#fae3a8', '#1d1503'],
  alpenglow: ['#ff8a4c', '#ffc59e', '#1d0c03'],
  ember: ['#ff6464', '#ffb3b3', '#2a0707'],
  heather: ['#f27ac0', '#fbc4e3', '#2a0718'],
  dusk: ['#a596f7', '#d6cfff', '#120c2b'],
  granite: ['#b4bfcc', '#dde3ea', '#11151a']
};

/** Accent names used by earlier versions. */
const OLD_ACCENTS: Record<string, string> = { violet: 'dusk', rose: 'ember', emerald: 'moss', ocean: 'glacier', sunset: 'alpenglow' };

export const isHexColor = (v: string) => /^#[0-9a-f]{6}$/i.test(v);

/** A preset name, a custom colour ("#rrggbb") or "rainbow". */
export function accentName(name: string) {
  if (ACCENTS[name] || name === 'rainbow' || isHexColor(name)) return isHexColor(name) ? name.toLowerCase() : name;
  return OLD_ACCENTS[name] ?? 'glacier';
}

export function hexToRgb(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}
export function rgbToHex([r, g, b]: [number, number, number]) {
  return '#' + [r, g, b].map((v) => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, '0')).join('');
}
const mix = (a: [number, number, number], b: [number, number, number], t: number): [number, number, number] =>
  [0, 1, 2].map((i) => a[i] + (b[i] - a[i]) * t) as [number, number, number];

/** Tint and button-text colours for any accent, so it always stays readable. */
export function accentColors(hex: string): [string, string, string] {
  const rgb = hexToRgb(hex);
  const lum = (c: number) => {
    c /= 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  const light = 0.2126 * lum(rgb[0]) + 0.7152 * lum(rgb[1]) + 0.0722 * lum(rgb[2]) > 0.18;
  return [hex, rgbToHex(mix(rgb, [255, 255, 255], 0.5)), light ? rgbToHex(mix(rgb, [0, 0, 0], 0.9)) : '#ffffff'];
}

function setAccentVars([a, b, on]: [string, string, string]) {
  const root = document.documentElement.style;
  root.setProperty('--accent', a);
  root.setProperty('--accent-2', b);
  root.setProperty('--on-accent', on);
}

export function applyAccent(name: string) {
  const picked = accentName(name);
  stopRainbow();
  if (picked === 'rainbow') startRainbow();
  else setAccentVars(isHexColor(picked) ? accentColors(picked) : ACCENTS[picked]);
}

// Rainbow: walks round the colour wheel (one lap every 8 seconds), in small
// steps so it looks smooth. Waits while Peak is minimized or hidden.
const RAINBOW_LAP_MS = 8000;
const RAINBOW_STEP_MS = 50;
let rainbowTimer: ReturnType<typeof setInterval> | null = null;
let hue = 200;

function startRainbow() {
  const step = () => {
    if (!view.shown) return;
    hue = (hue + (360 * RAINBOW_STEP_MS) / RAINBOW_LAP_MS) % 360;
    setAccentVars([`hsl(${hue.toFixed(1)} 85% 66%)`, `hsl(${hue.toFixed(1)} 90% 84%)`, `hsl(${hue.toFixed(1)} 60% 8%)`]);
  };
  step();
  rainbowTimer = setInterval(step, RAINBOW_STEP_MS);
}

function stopRainbow() {
  if (rainbowTimer) clearInterval(rainbowTimer);
  rainbowTimer = null;
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
  api.modSearchAll().then((m) => (app.modSearches = m)).catch(() => {});

  // peak:// links from the website's "Play in Peak" buttons.
  const { openLink } = await import('./discover.svelte');
  await on<PeakLink>('open-link', (link) => {
    api.takePendingLink().catch(() => {});
    openLink(link);
  });
  api
    .takePendingLink()
    .then((link) => link && openLink(link))
    .catch(() => {});

  await on<{ instanceId: string; name: string; files: BlockedFile[] }>('install-blocked', (e) => {
    app.blocked[e.instanceId] = e.files;
    // Don't join a server with mods still missing.
    import('./discover.svelte').then((m) => m.dropPendingJoin(e.instanceId, 'blocked')).catch(() => {});
    // Ask the user to fetch them right away, like ATLauncher does.
    app.manualFor = e.instanceId;
  });
  await on<{ instanceId: string; name: string; message: string }>('install-error', (e) => {
    toast(`${e.name}: ${e.message}`, 'error');
    import('./discover.svelte').then((m) => m.dropPendingJoin(e.instanceId, 'failed')).catch(() => {});
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
    // A load error while the game is still open (e.g. behind Fabric's error
    // window): explain it now, once the whole message has been printed.
    if (app.modSearches[l.instanceId]) {
      // Mid-search: the search asks about it when the game closes.
      if (LOAD_ERROR.test(l.line)) loadErrorSeen[l.instanceId] = true;
    } else if (!crashShown[l.instanceId] && LOAD_ERROR.test(l.line)) {
      clearTimeout(crashTimers[l.instanceId]);
      crashTimers[l.instanceId] = setTimeout(() => showCrashHelp(l.instanceId), 1500);
    }
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
      gameStarted[s.instanceId] = Date.now();
      delete crashShown[s.instanceId];
      delete loadErrorSeen[s.instanceId];
    } else {
      app.running = app.running.filter((r) => r !== s.instanceId);
      import('./presence.svelte').then((m) => delete m.playingOn[s.instanceId]).catch(() => {});
      import('./invite.svelte').then((m) => delete m.playingAt[s.instanceId]).catch(() => {});
      if (app.modSearches[s.instanceId]) {
        // A test of a broken-mod search: ask how it went instead.
        const crashed = (s.exitCode !== null && s.exitCode !== 0) || loadErrorSeen[s.instanceId];
        app.modSearchFor = { instanceId: s.instanceId, guess: crashed ? 'crashed' : 'worked' };
      } else if (s.exitCode && s.exitCode !== 0) {
        // Show what went wrong (the pop-out window, if open, already does).
        if (!app.poppedOut[s.instanceId]) go('instance', s.instanceId, 'console');
        // The crash helper explains it and offers a fix; the plain message if it can't.
        clearTimeout(crashTimers[s.instanceId]);
        showCrashHelp(s.instanceId, `Game exited with code ${s.exitCode}. Check the console for details.`);
      }
      refreshInstances();
    }
  });
  app.ready = true;
  guardWindowClose().catch(() => {});
  import('./presence.svelte').then((m) => m.startPresence()).catch(() => {});
  // Check modpacks for updates quietly, a moment after start-up.
  setTimeout(() => import('./packUpdates.svelte').then((m) => m.checkPackUpdates()).catch(() => {}), 4000);

  // Modpacks installed before Peak saved their artwork: fetch it once, quietly.
  for (const inst of app.instances.filter((i) => i.pack?.projectId && (!i.pack.iconUrl || !i.pack.description))) {
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
  // A modpack with a newer version: ask now (not at start-up). Not while
  // searching for a broken mod, which an update would throw off.
  const { askBeforePlay } = await import('./packUpdates.svelte');
  if (!app.modSearches[inst.id] && !(await askBeforePlay(inst))) return;
  if (!(await askJavaOnce(inst))) return;
  app.logs[inst.id] = [];
  try {
    await api.launch(inst.id);
  } catch (e) {
    delete app.progress[inst.id];
    toast(errorText(e), 'error', { rage: true });
  }
}

/** The first time someone plays, let them pick a Java build (unless this
 *  instance or their settings already name a particular Java). */
function askJavaOnce(inst: Instance): Promise<boolean> {
  const s = app.settings;
  if (!s || s.javaVendorChosen || !s.useBundledJava || inst.javaPath || inst.javaRuntime) return Promise.resolve(true);
  return new Promise((resolve) => (app.javaPrompt = resolve));
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
