// Thin wrapper over Tauri commands. When the UI runs in a normal browser
// (e.g. `npm run dev` without Tauri) it falls back to an in-memory mock so the
// interface can be designed and tested without the Rust backend.
import type {
  SideFilter,
  AccountInfo,
  AppInfo,
  ContentKind,
  ContentVersion,
  InstalledItem,
  PlanItem,
  Project,
  SearchResult,
  Source,
  UpdateInfo,
  DetectedJava,
  ExportEntry,
  ExportOptions,
  ExportReport,
  ImportCandidate,
  ManualFile,
  DeviceCode,
  LoaderId,
  LoaderVersion,
  RuntimeOption,
  Instance,
  InstallProgress,
  LinkedMod,
  LoginStart,
  ModSide,
  ServerPackReport,
  SavedServer,
  ServerStatus,
  Skin,
  Wardrobe,
  PastSkin,
  PackUpdate,
  Listing,
  ManualPack,
  InstanceServer,
  Player,
  MenuStatus,
  LogLine,
  Settings,
  VersionManifest
} from './types';

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

type Handler<T> = (payload: T) => void;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(cmd, args);
  }
  return mock.call(cmd, args ?? {}) as Promise<T>;
}

export async function on<T>(event: string, handler: Handler<T>): Promise<() => void> {
  if (isTauri) {
    const { listen } = await import('@tauri-apps/api/event');
    return listen<T>(event, (e) => handler(e.payload));
  }
  return mock.on(event, handler as Handler<unknown>);
}

export const api = {
  versions: () => call<VersionManifest>('get_versions'),
  instances: () => call<Instance[]>('list_instances'),
  loaderVersions: (loader: LoaderId, gameVersion: string) => call<LoaderVersion[]>('loader_versions', { loader, gameVersion }),
  createInstance: (name: string, gameVersion: string, loader: LoaderId, loaderVersion: string | null) =>
    call<Instance>('create_instance', { name, gameVersion, loader, loaderVersion }),
  worlds: (id: string) => call<string[]>('list_worlds', { id }),
  contentSearch: (source: Source, instanceId: string, kind: ContentKind, query: string, sort: string, offset: number, side: SideFilter = 'any') =>
    call<SearchResult>('content_search', { source, instanceId, kind, query, sort, offset, side }),
  contentProject: (id: string) => call<Project>('content_project', { id }),
  contentVersions: (instanceId: string, kind: ContentKind, projectId: string) =>
    call<ContentVersion[]>('content_versions', { instanceId, kind, projectId }),
  contentInstall: (instanceId: string, kind: ContentKind, projectId: string, versionId: string | null = null) =>
    call<string[]>('content_install', { instanceId, kind, projectId, versionId }),
  contentPlan: (instanceId: string, kind: ContentKind, projectId: string, versionId: string | null = null) =>
    call<PlanItem[]>('content_plan', { instanceId, kind, projectId, versionId }),
  contentInstallItems: (instanceId: string, items: PlanItem[]) => call<string[]>('content_install_items', { instanceId, items }),
  contentList: (instanceId: string, kind: ContentKind) => call<InstalledItem[]>('content_list', { instanceId, kind }),
  contentSetEnabled: (instanceId: string, kind: ContentKind, fileName: string, enabled: boolean) =>
    call<void>('content_set_enabled', { instanceId, kind, fileName, enabled }),
  contentLinked: (instanceId: string, fileName: string, action: 'disable' | 'remove' | 'enable') =>
    call<LinkedMod[]>('content_linked', { instanceId, fileName, action }),
  contentRemove: (instanceId: string, kind: ContentKind, fileName: string) =>
    call<void>('content_remove', { instanceId, kind, fileName }),
  contentUpdates: (instanceId: string, kind: ContentKind) => call<UpdateInfo[]>('content_updates', { instanceId, kind }),
  openContentFolder: (instanceId: string, kind: ContentKind) => call<void>('open_content_folder', { instanceId, kind }),
  modpackSearch: (source: Source, query: string, sort: string, offset: number) =>
    call<SearchResult>('modpack_search', { source, query, sort, offset }),
  modpackVersions: (projectId: string) => call<ContentVersion[]>('modpack_versions', { projectId }),
  modpackUpdates: () => call<PackUpdate[]>('modpack_updates'),
  /** If a CurseForge pack must be downloaded by hand: what and where (else null). */
  modpackManualInfo: (projectId: string, versionId: string | null) => call<ManualPack | null>('modpack_manual_info', { projectId, versionId }),
  /** The path of a file in Downloads once it's all there, else null. */
  findDownload: (fileName: string, size: number) => call<string | null>('find_download', { fileName, size }),
  modpackSkipUpdate: (id: string, versionId: string) => call<Instance>('modpack_skip_update', { id, versionId }),
  modpackMuteUpdates: (id: string, muted: boolean) => call<Instance>('modpack_mute_updates', { id, muted }),
  /** Switch a modpack instance to another version of its pack (installs in the background). */
  modpackChangeVersion: (id: string, versionId: string) => call<Instance>('modpack_change_version', { id, versionId }),
  resetSettings: () => call<Settings>('reset_settings'),
  presenceShow: (card: import('./presence.svelte').PresenceCard) => call<void>('presence_show', { card }),
  presenceClear: () => call<void>('presence_clear'),
  modpackInstall: (opts: { projectId?: string; versionId?: string | null; filePath?: string; removeFile?: boolean }) =>
    call<Instance>('modpack_install', {
      projectId: opts.projectId ?? null,
      versionId: opts.versionId ?? null,
      filePath: opts.filePath ?? null,
      removeFile: opts.removeFile ?? null
    }),
  exportEntries: (instanceId: string) => call<ExportEntry[]>('export_entries', { instanceId }),
  exportPack: (instanceId: string, options: ExportOptions) => call<ExportReport>('export_pack', { instanceId, options }),
  serverPackAnalyze: (instanceId: string) => call<ModSide[]>('serverpack_analyze', { instanceId }),
  serverPackExport: (instanceId: string, options: { path: string; clientFiles: string[]; keepClient: boolean }) =>
    call<ServerPackReport>('serverpack_export', { instanceId, options }),
  /** Ask where to save a file. Returns null if cancelled. */
  pickSavePath: async (defaultName: string, ext: string, label: string): Promise<string | null> => {
    if (!isTauri) return `C:/Users/you/Downloads/${defaultName}`;
    const { save } = await import('@tauri-apps/plugin-dialog');
    return (await save({ defaultPath: defaultName, filters: [{ name: label, extensions: [ext] }] })) ?? null;
  },
  /** Show a file in the system file manager. */
  revealFile: async (path: string) => {
    if (!isTauri) return;
    const { revealItemInDir } = await import('@tauri-apps/plugin-opener');
    return revealItemInDir(path);
  },
  refreshPackArt: (id: string) => call<Instance>('refresh_pack_art', { id }),
  manualList: (instanceId: string) => call<ManualFile[]>('manual_list', { instanceId }),
  manualScan: (instanceId: string) => call<ManualFile[]>('manual_scan', { instanceId }),
  manualDismiss: (instanceId: string) => call<void>('manual_dismiss', { instanceId }),
  openDownloadsFolder: () => call<void>('open_downloads_folder'),
  importDetect: () => call<ImportCandidate[]>('import_detect'),
  importInspect: (path: string) => call<ImportCandidate>('import_inspect', { path }),
  importInstances: (candidates: ImportCandidate[]) => call<Instance[]>('import_instances', { candidates }),
  /** Ask the user for a folder. Returns null if cancelled. */
  pickFolder: async (title: string): Promise<string | null> => {
    if (!isTauri) return 'C:/Games/MultiMC/instances/Better Survival';
    const { open } = await import('@tauri-apps/plugin-dialog');
    const picked = await open({ directory: true, multiple: false, title });
    return typeof picked === 'string' ? picked : null;
  },
  /** Ask the user for a Java executable. Returns null if cancelled. */
  pickJava: async (os: string | undefined): Promise<string | null> => {
    if (!isTauri) return 'C:/Program Files/Eclipse Adoptium/jdk-21/bin/javaw.exe';
    const { open } = await import('@tauri-apps/plugin-dialog');
    const picked = await open({
      multiple: false,
      directory: false,
      title: 'Choose a Java executable',
      // Windows Java is java.exe / javaw.exe; on macOS and Linux it has no extension.
      filters: os === 'windows' ? [{ name: 'Java (java.exe, javaw.exe)', extensions: ['exe'] }] : undefined
    });
    return typeof picked === 'string' ? picked : null;
  },
  /** Ask the user for a .mrpack file. Returns null if cancelled. */
  pickModpackFile: async (): Promise<string | null> => {
    if (!isTauri) return 'C:/Users/you/Downloads/Example Pack 1.0.mrpack';
    const { open } = await import('@tauri-apps/plugin-dialog');
    const picked = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'Modpack (Modrinth .mrpack or CurseForge .zip)', extensions: ['mrpack', 'zip'] }]
    });
    return typeof picked === 'string' ? picked : null;
  },
  javaRuntimes: () => call<RuntimeOption[]>('java_runtimes'),
  detectJava: () => call<DetectedJava[]>('detect_java'),
  testJava: (path: string) => call<DetectedJava>('test_java', { path }),
  updateInstance: (instance: Instance) => call<Instance>('update_instance', { instance }),
  duplicateInstance: (id: string) => call<Instance>('duplicate_instance', { id }),
  deleteInstance: (id: string) => call<void>('delete_instance', { id }),
  openFolder: (id: string) => call<void>('open_instance_folder', { id }),
  install: (id: string) => call<Instance>('install_instance', { id }),
  launch: (id: string) => call<void>('launch_instance', { id }),
  serversList: () => call<SavedServer[]>('servers_list'),
  serverSave: (server: SavedServer) => call<SavedServer[]>('server_save', { server }),
  serverDelete: (id: string) => call<SavedServer[]>('server_delete', { id }),
  serversReorder: (ids: string[]) => call<SavedServer[]>('servers_reorder', { ids }),
  serverPing: (address: string) => call<ServerStatus>('server_ping', { address }),
  serversFromInstance: (instanceId: string) => call<[string, string][]>('servers_from_instance', { instanceId }),
  serverJoin: (serverId: string, instanceId: string) => call<void>('server_join', { serverId, instanceId }),
  discoverList: () => call<Listing[]>('discover_list'),
  /** A peak:// server link Peak was opened with, if any (only returned once). */
  takePendingLink: () => call<string | null>('take_pending_link'),
  /** Launch an instance straight into a server from the Discover list. */
  discoverJoin: (instanceId: string, address: string, name?: string) => call<void>('discover_join', { instanceId, address, name }),
  instanceServers: (id: string) => call<InstanceServer[]>('instance_servers', { id }),
  instanceServerAdd: (id: string, name: string, address: string) => call<InstanceServer[]>('instance_server_add', { id, name, address }),
  instanceServerRemove: (id: string, address: string) => call<InstanceServer[]>('instance_server_remove', { id, address }),
  instanceServersReorder: (id: string, addresses: string[]) => call<InstanceServer[]>('instance_servers_reorder', { id, addresses }),
  instanceServerJoin: (id: string, address: string) => call<void>('instance_server_join', { id, address }),
  stop: (id: string) => call<void>('stop_instance', { id }),
  running: () => call<string[]>('running_instances'),
  gameLogs: (id: string) => call<LogLine[]>('game_logs', { id }),
  openConsoleWindow: (id: string) => call<void>('open_console_window', { id }),
  closeConsoleWindow: (id: string) => call<void>('close_console_window', { id }),
  accounts: () => call<AccountInfo[]>('list_accounts'),
  msLoginStart: () => call<LoginStart>('ms_login_start'),
  msLoginFinish: (start: LoginStart) => call<AccountInfo[]>('ms_login_finish', { start }),
  msLoginCancel: () => call<void>('ms_login_cancel'),
  accountSkin: (id: string, refresh = false) => call<Skin | null>('account_skin', { id, refresh }),
  accountCape: (id: string) => call<string | null>('account_cape', { id }),
  skinWardrobe: (id: string) => call<Wardrobe>('skin_wardrobe', { id }),
  appimageMenuStatus: () => call<MenuStatus>('appimage_menu_status'),
  appimageMenuInstall: () => call<MenuStatus>('appimage_menu_install'),
  appimageMenuRemove: () => call<MenuStatus>('appimage_menu_remove'),
  skinReadFile: (path: string) => call<string>('skin_read_file', { path }),
  skinUpload: (id: string, path: string, model: 'classic' | 'slim') => call<Wardrobe>('skin_upload', { id, path, model }),
  skinSetModel: (id: string, model: 'classic' | 'slim') => call<Wardrobe>('skin_set_model', { id, model }),
  skinReset: (id: string) => call<Wardrobe>('skin_reset', { id }),
  skinSetCape: (id: string, capeId: string | null) => call<Wardrobe>('skin_set_cape', { id, capeId }),
  skinHistory: (id: string) => call<PastSkin[]>('skin_history', { id }),
  skinUseSaved: (id: string, skinId: string, model: 'classic' | 'slim') => call<Wardrobe>('skin_use_saved', { id, skinId, model }),
  skinHistoryRemove: (id: string, skinId: string) => call<void>('skin_history_remove', { id, skinId }),
  playerLookup: (name: string) => call<Player>('player_lookup', { name }),
  /** Set an instance's own logo or banner (null puts the pack's artwork back). */
  instanceSetPicture: (id: string, kind: 'icon' | 'banner', path: string | null) => call<Instance>('instance_set_picture', { id, kind, path }),
  /** Ask the user for a picture. Returns null if cancelled. */
  pickPicture: async (title: string): Promise<string | null> => {
    if (!isTauri) return 'C:/Users/you/Pictures/banner.png';
    const { open } = await import('@tauri-apps/plugin-dialog');
    const picked = await open({ multiple: false, directory: false, title, filters: [{ name: 'Pictures', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] }] });
    return typeof picked === 'string' ? picked : null;
  },
  /** Ask the user for a skin PNG. Returns null if cancelled. */
  pickSkinFile: async (): Promise<string | null> => {
    if (!isTauri) return 'C:/Users/you/Downloads/my-skin.png';
    const { open } = await import('@tauri-apps/plugin-dialog');
    const picked = await open({ multiple: false, directory: false, title: 'Choose a skin', filters: [{ name: 'Minecraft skin (PNG)', extensions: ['png'] }] });
    return typeof picked === 'string' ? picked : null;
  },
  addOffline: (username: string) => call<AccountInfo[]>('add_offline_account', { username }),
  setActiveAccount: (id: string) => call<AccountInfo[]>('set_active_account', { id }),
  removeAccount: (id: string) => call<AccountInfo[]>('remove_account', { id }),
  info: () => call<AppInfo>('app_info'),
  settings: () => call<Settings>('get_settings'),
  saveSettings: (settings: Settings) => call<Settings>('save_settings', { settings }),
  autostartEnabled: () => call<boolean>('autostart_enabled'),
  setAutostart: (on: boolean) => call<void>('set_autostart', { on }),
  openUrl: async (url: string) => {
    if (isTauri) {
      const { openUrl } = await import('@tauri-apps/plugin-opener');
      return openUrl(url);
    }
    window.open(url, '_blank');
  }
};

// ---------------------------------------------------------------- mock ----

const mock = (() => {
  const listeners = new Map<string, Set<Handler<unknown>>>();
  const emit = (event: string, payload: unknown) => listeners.get(event)?.forEach((h) => h(payload));
  const now = Date.now();
  const iso = (msAgo: number) => new Date(now - msAgo).toISOString();

  let instances: Instance[] = [
    mk('survival-sp', 'Survival Island', '1.21.4', iso(36e5), 86400 * 3.2, 'fabric', '0.16.9'),
    mk('creative-builds', 'Creative Builds', '1.21.1', iso(864e5 * 2), 86400 * 1.1, 'neoforge', '21.1.77'),
    mk('skyblock', 'Skyblock Hardcore', '1.20.1', iso(864e5 * 9), 3600 * 14, 'forge', '1.20.1-47.3.0'),
    mk('nostalgia', 'Beta Nostalgia', 'b1.7.3', null, 0)
  ];
  let accounts: AccountInfo[] = [{ id: 'a1', username: 'PeakPlayer', kind: 'microsoft', active: true }];
  const DEFAULT_ARGS =
    '-XX:+UnlockExperimentalVMOptions -XX:+UseG1GC -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M';
  let mockAutostart = false;
  let settings: Settings = {
    accent: 'glacier',
    background: 'aurora',
    memoryMb: 4096,
    metaspaceMb: 256,
    jvmArgs: '',
    jvmPreset: 'none',
    useBundledJava: true,
    javaPath: '',
    windowWidth: 1280,
    windowHeight: 720,
    fullscreen: false,
    afterLaunch: 'open',
    openConsole: true,
    autoUpdateCheck: true,
    appimageMenuDismissed: false,
    modpackUpdateCheck: true,
    discordPresence: true,
    enableCommands: false,
    preLaunchCommand: '',
    postExitCommand: '',
    wrapperCommand: '',
    gamemode: false,
    useSystemGlfw: false,
    useSystemOpenal: false,
    concurrentDownloads: 24,
    connectionTimeout: 30,
    proxyEnabled: false,
    proxyType: 'http',
    proxyHost: '',
    proxyPort: 8080,
    curseforgeApiKey: ''
  };
  const defaultSettings: Settings = JSON.parse(JSON.stringify(settings));
  // Preview: one instance is a modpack a version behind.
  instances[2].pack = {
    name: 'Skyblock Hardcore',
    versionNumber: '12.0.0',
    projectId: 'mock-pack',
    versionId: 'mv2',
    iconUrl: null,
    bannerUrl: '/hero.jpg'
  };
  const running = new Set<string>();

  function mk(
    id: string,
    name: string,
    v: string,
    lastPlayed: string | null,
    playtime: number,
    loader: Instance['loader'] = 'vanilla',
    loaderVersion: string | null = null
  ): Instance {
    return {
      id,
      name,
      gameVersion: v,
      loader,
      loaderVersion,
      createdAt: iso(864e5 * 30),
      lastPlayed,
      playtimeSecs: Math.round(playtime),
      installed: lastPlayed !== null,
      versionId: null,
      notes: '',
      memoryMb: null,
      metaspaceMb: null,
      jvmArgs: null,
      jvmPreset: null,
      javaPath: null,
      javaRuntime: null,
      accountId: null,
      quickPlay: { mode: 'none', value: '' },
      windowWidth: null,
      windowHeight: null,
      fullscreen: null,
      enableCommands: null,
      preLaunchCommand: null,
      postExitCommand: null,
      wrapperCommand: null,
      useSystemGlfw: null,
      useSystemOpenal: null,
      pack: null,
      importedFrom: null
    };
  }

  const versions: VersionManifest = {
    latest: { release: '1.21.4', snapshot: '25w02a' },
    versions: [
      ['25w02a', 'snapshot'],
      ['1.21.4', 'release'],
      ['1.21.3', 'release'],
      ['1.21.1', 'release'],
      ['1.21', 'release'],
      ['1.20.6', 'release'],
      ['1.20.4', 'release'],
      ['1.20.1', 'release'],
      ['1.19.4', 'release'],
      ['1.18.2', 'release'],
      ['1.16.5', 'release'],
      ['1.12.2', 'release'],
      ['1.8.9', 'release'],
      ['1.7.10', 'release'],
      ['b1.7.3', 'old_beta'],
      ['a1.2.6', 'old_alpha']
    ].map(([id, type]) => ({ id, type, url: '', releaseTime: '' })) as VersionManifest['versions']
  };

  const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

  const MOCK_MODS: [string, string, string][] = [
    ['Sodium', 'The fastest and most compatible rendering optimization mod for Minecraft.', 'jellysquid3'],
    ['Fabric API', 'Lightweight and modular API providing common hooks and intercompatibility measures.', 'modmuss50'],
    ['Iris Shaders', 'A modern shader pack loader intended to be compatible with OptiFine shaders.', 'coderbot'],
    ['Lithium', 'No-compromises game logic optimization mod.', 'jellysquid3'],
    ['Mod Menu', 'Adds a mod menu to view the list of mods you have installed.', 'Prospector'],
    ['Entity Culling', 'Uses async path-tracing to hide blocks and entities that are not visible.', 'tr7zw'],
    ['ImmediatelyFast', 'Speeds up immediate mode rendering in Minecraft.', 'RaphiMC'],
    ['Jade', 'Shows information about what you are looking at.', 'Snownee'],
    ['AppleSkin', 'Food and hunger related HUD improvements.', 'squeek502']
  ];
  const MOCK_SHADERS: [string, string, string][] = [
    ['Complementary Shaders - Reimagined', 'Gorgeous and performant shaders that keep the vanilla look.', 'EminGT'],
    ['BSL Shaders', 'A bright and colorful shader pack with lots of customization.', 'capttatsu'],
    ['Solas Shader', 'A fantasy-styled shader with volumetric lighting.', 'Septonious']
  ];
  const MOCK_PACKS: [string, string, string][] = [
    ['Fresh Animations', 'Adds new animations to vanilla mobs.', 'FreshLX'],
    ['Faithful 32x', 'The classic Minecraft look, in twice the resolution.', 'Faithful Team']
  ];
  const MOCK_MODPACKS: [string, string, string][] = [
    ['Fabulously Optimized', 'Improved performance and graphics with vanilla look and feel.', 'robotkoer'],
    ['Cobblemon Official Modpack', 'The official modpack for Cobblemon, a Pokemon-inspired adventure.', 'Cobblemon'],
    ['Simply Optimized', 'Simple, performance-focused modpack.', 'HyperSoop'],
    ['Better MC', 'Turns vanilla Minecraft into a better experience while staying true to its spirit.', 'SharkieFPS']
  ];
  const MOCK_FTB: [string, string, string][] = [
    ['FTB StoneBlock 4', 'The stone remembers. Underground survival with Vaults, Echoes and the World Engine.', 'FTB Team'],
    ['FTB Evolution', 'A kitchen-sink pack that grows with you across tech and magic.', 'FTB Team'],
    ['FTB Skies 2', 'Skyblock with a story, quests and floating islands.', 'FTB Team']
  ];
  const ALL_MOCK = [...MOCK_MODS, ...MOCK_SHADERS, ...MOCK_PACKS, ...MOCK_MODPACKS];
  const ftbMock = (id: string) => MOCK_FTB[130 - Number(String(id).replace('ftb:', ''))];
  const pid = (title: string) => 'p-' + title.toLowerCase().replace(/[^a-z]/g, '');
  /** Mock ids: Modrinth "p-name", CurseForge "cf:p-name". */
  const findMock = (id: string) => ALL_MOCK.find(([t]) => pid(t) === String(id).replace(/^cf:/, ''));
  const NEED_KEY = 'CurseForge needs an API key. Add yours in Settings > Integrations.';
  /** In the preview, CurseForge needs a key in Settings just like the real app. */
  const sourced = <T extends { project_id: string }>(source: string, hits: T[]) => {
    if (source !== 'curseforge') return hits;
    if (!settings.curseforgeApiKey.trim()) throw NEED_KEY;
    return hits.map((h) => ({ ...h, project_id: 'cf:' + h.project_id }));
  };
  // A few mods on the first instance so the Content tab has something to show.
  const popouts: Record<string, Window | null> = {};

  /** A simple made-up 64x64 skin for the preview (not a Minecraft asset). */
  function sampleSkin(shirt = '#4b8fd6'): string {
    const c = document.createElement('canvas');
    c.width = c.height = 64;
    const g = c.getContext('2d')!;
    const box = (x: number, y: number, w: number, h: number, color: string) => ((g.fillStyle = color), g.fillRect(x, y, w, h));
    // head (all six faces), skin tone with hair on top/back
    box(0, 0, 32, 16, '#c9956b');
    box(8, 0, 8, 8, '#3b2a20');
    box(0, 8, 32, 3, '#3b2a20');
    box(24, 8, 8, 8, '#3b2a20');
    // face details on the front (8..16, 8..16)
    box(9, 12, 2, 1, '#ffffff');
    box(13, 12, 2, 1, '#ffffff');
    box(10, 12, 1, 1, '#3d6fd6');
    box(14, 12, 1, 1, '#3d6fd6');
    box(11, 14, 2, 1, '#8b5a3c');
    // body, arms, legs
    box(16, 16, 24, 16, shirt);
    box(40, 16, 16, 16, '#c9956b');
    box(32, 48, 16, 16, '#c9956b');
    box(0, 16, 16, 16, '#2d3a66');
    box(16, 48, 16, 16, '#2d3a66');
    return c.toDataURL('image/png');
  }
  let loginCancelled = false;
  let mockMenu = false;

  // Preview servers (made-up names, so no real server is shown or contacted).
  const NL = String.fromCharCode(10);
  let mockServers: SavedServer[] = [
    { id: 's1', name: 'Peak Survival', address: 'play.peak-example.net', instanceId: 'survival-sp' },
    { id: 's2', name: 'Creative Plots', address: 'creative.example.org:25570', instanceId: null },
    { id: 's3', name: 'Old LAN box', address: '192.168.1.40', instanceId: null }
  ];
  // Preview Discover list (made-up servers).
  const listing = (l: Partial<Listing> & Pick<Listing, 'id' | 'name' | 'address'>): Listing => ({
    tagline: '',
    description: '',
    tags: [],
    region: 'Europe',
    language: 'English',
    gameVersion: '1.21.4',
    loader: 'vanilla',
    pack: null,
    votes: 0,
    featured: false,
    verified: false,
    website: null,
    discord: null,
    pageUrl: 'https://www.peaklauncher.com/servers/',
    ...l
  });
  const mockListings: Listing[] = [
    listing({ id: 'l1', name: 'Peak Survival', address: 'play.peak-example.net', bannerUrl: '/hero.jpg', tagline: 'Land claims, shops and weekly events', description: 'A friendly survival server with land claims, player shops and a new season every few months.', tags: ['Survival', 'SMP', 'Economy'], votes: 1240, featured: true, verified: true, region: 'North America' }),
    listing({ id: 'l2', name: 'Creative Plots', address: 'creative.example.org:25570', tagline: 'Build anything on your own plot', description: 'Claim a plot and build without limits. Weekly build contests.', tags: ['Creative', 'Building'], votes: 610 }),
    listing({ id: 'l3', name: 'Cobble Valley', address: 'cobble.example.invalid', tagline: 'Catch, battle and build with Cobblemon', description: 'A sample modded server. Play installs the pack, then joins.', tags: ['Cobblemon', 'Survival'], gameVersion: '1.21.1', loader: 'fabric', votes: 862, featured: true, pack: { projectId: 'cobble', name: 'Cobblemon Official Modpack', iconUrl: null } }),
    listing({ id: 'l4', name: 'Brass & Steam', address: 'brass.example.invalid', tagline: 'Build factories with Create', description: 'A sample modded server running Create.', tags: ['Create', 'Tech'], gameVersion: '1.21.1', loader: 'neoforge', votes: 540, region: 'Oceania', pack: { projectId: 'create', name: 'Create+', iconUrl: null } })
  ];
  const mockInstanceServers: Record<string, InstanceServer[]> = {};
  const mockStatus: Record<string, ServerStatus> = {
    'play.peak-example.net': {
      online: true,
      error: null,
      version: 'Paper 1.21.4',
      protocol: 769,
      playersOnline: 37,
      playersMax: 100,
      playersSample: ['Alex', 'Steve', 'Notch_fan'],
      motd: { text: '', extra: [{ text: 'Peak Survival ', color: 'aqua', bold: true }, { text: '| ', color: 'dark_gray' }, { text: 'Season 4 is live!' + NL, color: 'gold' }, { text: 'Land claims, shops and weekly events', color: 'gray' }] },
      favicon: null,
      pingMs: 38
    },
    'creative.example.org:25570': {
      online: true,
      error: null,
      version: '§aVelocity 1.20-1.21',
      protocol: 767,
      playersOnline: 4,
      playersMax: 50,
      playersSample: [],
      motd: '§d§lCreative Plots§r' + NL + '§7Build anything, 200x200 plots',
      favicon: null,
      pingMs: 112
    }
  };
  const mockContent: Record<string, InstalledItem[]> = {
    'survival-spmod': [
      { fileName: 'fabric-api-0.110.0.jar', enabled: true, size: 2_400_000, sha1: 'fabric-api-0.110.0.jar', projectId: 'fabric-api-0.110.0.jar', versionId: 'v1', title: 'Fabric API', versionNumber: '0.110.0', iconUrl: null },
      { fileName: 'sodium-0.6.5.jar', enabled: true, size: 2_400_000, sha1: 'sodium-0.6.5.jar', projectId: 'sodium-0.6.5.jar', versionId: 'v1', title: 'Sodium', versionNumber: '0.6.5', iconUrl: null },
      { fileName: 'iris-1.8.1.jar', enabled: true, size: 2_400_000, sha1: 'iris-1.8.1.jar', projectId: 'iris-1.8.1.jar', versionId: 'v1', title: 'Iris', versionNumber: '1.8.1', iconUrl: null },
      { fileName: 'modmenu-12.0.0.jar', enabled: true, size: 2_400_000, sha1: 'modmenu-12.0.0.jar', projectId: 'modmenu-12.0.0.jar', versionId: 'v1', title: 'Mod Menu', versionNumber: '12.0.0', iconUrl: null }
    ]
  };
  const mockManual: Record<string, ManualFile[]> = {};
  const mockScans: Record<string, number> = {};

  async function fakeInstall(id: string) {
    const stages: [string, number][] = [
      ['Downloading game files', 3400],
      ['Downloading Java', 240]
    ];
    for (const [stage, total] of stages) {
      for (let done = 0; done <= total; done += Math.ceil(total / 25)) {
        emit('install-progress', { instanceId: id, stage, done: Math.min(done, total), total, bytes: 0 } satisfies InstallProgress);
        await sleep(70);
      }
    }
    emit('install-progress', { instanceId: id, stage: 'Done', done: 1, total: 1, bytes: 0 });
    instances = instances.map((i) => (i.id === id ? { ...i, installed: true } : i));
  }

  const logs = [
    '[Render thread/INFO]: Setting user: PeakPlayer',
    '[Render thread/INFO]: Backend library: LWJGL version 3.3.3',
    '[Render thread/INFO]: Reloading ResourceManager: vanilla',
    '[Worker-Main-2/INFO]: Found 1 shader pack(s)',
    '[Render thread/WARN]: Missing sound for event: minecraft:item.goat_horn.play',
    '[Render thread/INFO]: Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas',
    '[Render thread/INFO]: Sound engine started',
    '\x1b[32m[Server thread/INFO]\x1b[0m: Preparing spawn area: \x1b[1;36m83%\x1b[0m',
    '\x1b[33m[modloading-worker-0/WARN]\x1b[0m: Mod \x1b[38;5;213mcreate\x1b[0m is using a deprecated API',
    '[Render thread/INFO]: [CHAT] §6[Server]§r Welcome, §bPeakPlayer§r! §lHave fun§r',
    '[Render thread/INFO]: [CHAT] §x§F§F§7§7§A§AHex pink§r and §m§cstruck red'
  ];

  async function call(cmd: string, a: Record<string, any>): Promise<unknown> {
    await sleep(120);
    switch (cmd) {
      case 'get_versions':
        return versions;
      case 'list_instances':
        return [...instances].sort((x, y) => (y.lastPlayed ?? '').localeCompare(x.lastPlayed ?? ''));
      case 'loader_versions':
        if (a.loader === 'vanilla') return [];
        return Array.from({ length: 12 }, (_, i) => ({
          id: a.loader === 'forge' ? `${a.gameVersion}-47.3.${12 - i}` : a.loader === 'neoforge' ? `21.1.${90 - i}` : `0.16.${12 - i}`,
          stable: i > 0,
          recommended: i === 1
        }));
      case 'content_search': {
        const inst = instances.find((i) => i.id === a.instanceId);
        if (a.kind === 'mod' && inst?.loader === 'vanilla')
          throw "Vanilla instances can't run mods. Add a mod loader in the instance's settings.";
        const names = a.kind === 'mod' ? MOCK_MODS : a.kind === 'shader' ? MOCK_SHADERS : MOCK_PACKS;
        const q = String(a.query).toLowerCase();
        const hits = names
          .filter(([t]) => t.toLowerCase().includes(q))
          .map(([title, description, author], i) => ({
            project_id: pid(title),
            slug: pid(title),
            title,
            description,
            author,
            icon_url: null,
            downloads: Math.round(48_000_000 / (i + 1)),
            follows: Math.round(30_000 / (i + 1)),
            categories: ['optimization', 'utility', 'library'].slice(0, (i % 3) + 1),
            date_modified: iso(864e5 * (i + 2)),
            // A mix of sides, so the "Runs on" filter has something to do.
            client_side: a.source === 'curseforge' ? null : i % 3 === 2 ? 'unsupported' : 'required',
            server_side: a.source === 'curseforge' ? null : i % 3 === 1 ? 'unsupported' : 'required'
          }))
          .filter((h) => {
            if (a.source === 'curseforge' || a.kind !== 'mod' || !a.side || a.side === 'any') return true;
            if (a.side === 'client') return h.server_side === 'unsupported';
            if (a.side === 'server') return h.client_side === 'unsupported';
            return h.client_side !== 'unsupported' && h.server_side !== 'unsupported';
          });
        const out = sourced(a.source, hits);
        return { hits: out.slice(a.offset, a.offset + 24), total_hits: out.length, offset: a.offset };
      }
      case 'content_project': {
        const t = String(a.id).startsWith('technic:') ? (['Tekkit 2', 'Tekkit is back.', 'Technic'] as [string, string, string]) : String(a.id).startsWith('ftb:') ? ftbMock(a.id) : findMock(a.id)!;
        const cf = String(a.id).startsWith('cf:');
        return {
          website_url: cf ? `https://www.curseforge.com/minecraft/mc-mods/${pid(t[0]).slice(2)}` : null,
          id: a.id,
          slug: a.id,
          title: t[0],
          description: t[1],
          icon_url: null,
          downloads: 12_400_000,
          followers: 18_000,
          categories: ['optimization', 'utility'],
          project_type: 'mod',
          source_url: 'https://github.com',
          issues_url: 'https://github.com',
          wiki_url: null,
          discord_url: null,
          gallery: [],
          updated: iso(864e5 * 3),
          body: `## About\n\n${t[1]}\n\n- Works with **Fabric** and **Quilt**\n- Client side only\n\n> Preview data shown in the browser.`
        } satisfies Project;
      }
      case 'content_versions':
        return Array.from({ length: 6 }, (_, i) => ({
          id: 'v' + i,
          project_id: a.projectId,
          name: `Release ${6 - i}.0`,
          version_number: `${6 - i}.0.${i}`,
          version_type: i === 0 ? 'beta' : 'release',
          date_published: iso(864e5 * 9 * (i + 1)),
          downloads: Math.round(90_000 / (i + 1)),
          game_versions: ['1.21.4'],
          loaders: ['fabric', 'quilt']
        }));
      case 'content_install': {
        await sleep(900);
        const t = findMock(a.projectId)!;
        const list = (mockContent[a.instanceId + a.kind] ??= []);
        if (!list.some((x) => x.projectId === a.projectId))
          list.push({
            fileName: `${pid(t[0]).slice(2)}-5.0.1.jar`,
            enabled: true,
            size: 812_000,
            sha1: a.projectId,
            projectId: a.projectId,
            versionId: 'v1',
            title: t[0],
            versionNumber: '5.0.1',
            iconUrl: null
          });
        return [t[0]];
      }
      case 'content_plan': {
        // Preview: Sodium needs nothing; most other mods need Fabric API (and Mod Menu also Placeholder API).
        await sleep(400);
        const t = findMock(a.projectId)!;
        const cf = String(a.projectId).startsWith('cf:');
        const item = (title: string, requiredBy: string | null): PlanItem => ({
          projectId: (cf ? 'cf:' : '') + pid(title),
          versionId: 'v1',
          title,
          versionNumber: '5.0.1',
          iconUrl: null,
          kind: a.kind,
          requiredBy,
          isRoot: requiredBy === null,
          url: 'https://cdn.modrinth.com/x.jar',
          fileName: `${pid(title).slice(2)}-5.0.1.jar`,
          sha1: pid(title),
          size: 812_000
        });
        const have = new Set((mockContent[a.instanceId + 'mod'] ?? []).map((x) => x.projectId));
        const out = [item(t[0], null)];
        if (a.kind === 'mod' && !['Sodium', 'Fabric API'].includes(t[0])) out.push(item('Fabric API', t[0]));
        if (t[0] === 'Mod Menu') out.push(item('Text Placeholder API', 'Mod Menu'));
        return out.filter((x) => x.isRoot || !have.has(x.projectId));
      }
      case 'content_install_items': {
        await sleep(800);
        for (const it of a.items as PlanItem[]) {
          const list = (mockContent[a.instanceId + it.kind] ??= []);
          if (!list.some((x) => x.projectId === it.projectId))
            list.push({
              fileName: it.fileName,
              enabled: true,
              size: it.size,
              sha1: it.sha1 ?? it.projectId,
              projectId: it.projectId,
              versionId: it.versionId,
              title: it.title,
              versionNumber: it.versionNumber,
              iconUrl: null
            });
        }
        return (a.items as PlanItem[]).map((i) => i.title);
      }
      case 'content_list':
        return (mockContent[a.instanceId + a.kind] ?? []).map((x) => ({ ...x }));
      case 'content_set_enabled': {
        const it = (mockContent[a.instanceId + a.kind] ?? []).find((x) => x.fileName === a.fileName);
        if (it) {
          it.enabled = a.enabled;
          it.fileName = a.enabled ? it.fileName.replace(/\.disabled$/, '') : it.fileName + '.disabled';
        }
        return;
      }
      case 'content_linked': {
        // Preview: pretend the first mod in the list is a library the next two need.
        const list = (mockContent[a.instanceId + 'mod'] ?? []) as InstalledItem[];
        const i = list.findIndex((x) => x.fileName === a.fileName);
        if (a.action === 'enable') {
          return i > 0 && i < 3 && !list[0].enabled ? [{ fileName: list[0].fileName, name: list[0].title, reason: `Needed by ${list[i].title}` }] : [];
        }
        return i === 0
          ? list.slice(1, 3).filter((x) => x.enabled).map((x) => ({ fileName: x.fileName, name: x.title, reason: `Needs ${list[0].title}` }))
          : [];
      }
      case 'content_remove':
        mockContent[a.instanceId + a.kind] = (mockContent[a.instanceId + a.kind] ?? []).filter((x) => x.fileName !== a.fileName);
        return;
      case 'content_updates':
        return (mockContent[a.instanceId + a.kind] ?? [])
          .slice(0, 1)
          .map((x) => ({ fileName: x.fileName, projectId: x.projectId!, versionId: 'v0', versionNumber: '6.0.0' }));
      case 'modpack_search': {
        const q = String(a.query).toLowerCase();
        if (a.source === 'technic') {
          const tech = [
            ['Tekkit 2', 'Tekkit is back: tech, automation and space on 1.12.2.', 'tekkit-2'],
            ['Tekkit Legends', 'Tekkit returns in this legendary pack!', 'tekkit-legends'],
            ['RLCraft', 'A hardcore RPG-style survival overhaul.', 'rlcraft']
          ]
            .filter(([t]) => t.toLowerCase().includes(q))
            .map(([title, description, slug], i) => ({
              project_id: 'technic:' + slug,
              slug,
              title,
              description,
              author: 'Technic',
              icon_url: null,
              downloads: Math.round(4_800_000 / (i + 1)),
              follows: Math.round(22_000_000 / (i + 1)),
              categories: ['Minecraft 1.12.2'],
              date_modified: ''
            }));
          return { hits: tech, total_hits: tech.length, offset: 0 };
        }
        if (a.source === 'ftb') {
          const ftb = MOCK_FTB.filter(([t]) => t.toLowerCase().includes(q)).map(([title, description], i) => ({
            project_id: 'ftb:' + (130 - i),
            slug: pid(title).slice(2),
            title,
            description,
            author: 'FTB Team',
            icon_url: null,
            downloads: Math.round(2_200_000 / (i + 1)),
            follows: Math.round(1_400_000 / (i + 1)),
            categories: ['FTB', 'Adventure'],
            date_modified: iso(864e5 * (i + 2))
          }));
          return { hits: ftb, total_hits: ftb.length, offset: 0 };
        }
        const hits = MOCK_MODPACKS.filter(([t]) => t.toLowerCase().includes(q)).map(([title, description, author], i) => ({
          project_id: pid(title),
          slug: pid(title),
          title,
          description,
          author,
          icon_url: null,
          downloads: Math.round(9_000_000 / (i + 1)),
          follows: Math.round(12_000 / (i + 1)),
          categories: ['optimization', 'adventure'].slice(0, (i % 2) + 1),
          date_modified: iso(864e5 * (i + 1))
        }));
        const out = sourced(a.source, hits);
        return { hits: out, total_hits: out.length, offset: 0 };
      }
      case 'modpack_versions':
        return Array.from({ length: 4 }, (_, i) => ({
          id: 'mv' + i,
          project_id: a.projectId,
          name: `v${14 - i}.0`,
          version_number: `${14 - i}.0.0`,
          version_type: 'release',
          date_published: iso(864e5 * 20 * (i + 1)),
          downloads: 100_000,
          game_versions: ['1.21.4'],
          loaders: ['fabric']
        }));
      case 'modpack_manual_info':
        return null;
      case 'find_download':
        return null;
      case 'modpack_updates': {
        await sleep(600);
        if (!settings.modpackUpdateCheck) return [];
        return instances
          .filter((i) => i.pack?.projectId && !i.pack.noUpdatePrompts && i.pack.versionId !== 'mv0' && i.pack.skippedVersion !== 'mv0')
          .map((i) => ({
            instanceId: i.id,
            instanceName: i.name,
            packName: i.pack!.name,
            current: i.pack!.versionNumber,
            latestId: 'mv0',
            latest: '14.0.0',
            iconUrl: i.pack!.iconUrl
          })) satisfies PackUpdate[];
      }
      case 'modpack_skip_update':
      case 'modpack_mute_updates': {
        const inst = instances.find((i) => i.id === a.id)!;
        if (cmd === 'modpack_skip_update') inst.pack!.skippedVersion = a.versionId;
        else inst.pack!.noUpdatePrompts = a.muted;
        return inst;
      }
      case 'modpack_change_version': {
        await sleep(500);
        const inst = instances.find((i) => i.id === a.id)!;
        const n = Number(String(a.versionId).slice(2));
        inst.pack = { ...inst.pack!, versionId: a.versionId, versionNumber: `${14 - n}.0.0`, skippedVersion: null };
        inst.installed = false;
        setTimeout(() => fakeInstall(inst.id), 100);
        return inst;
      }
      case 'reset_settings':
        settings = { ...JSON.parse(JSON.stringify(defaultSettings)), curseforgeApiKey: settings.curseforgeApiKey };
        return settings;
      case 'autostart_enabled':
        return mockAutostart;
      case 'set_autostart':
        mockAutostart = !!a.on;
        return null;
      case 'modpack_install': {
        const t = a.projectId ? (String(a.projectId).startsWith('technic:') ? (['Tekkit 2', '', ''] as [string, string, string]) : String(a.projectId).startsWith('ftb:') ? ftbMock(a.projectId) : findMock(a.projectId)) : undefined;
        const name = t ? t[0] : String(a.filePath).split(/[\\/]/).pop()!.replace(/\.mrpack$/, '');
        const inst = mk(pid(name).slice(2) + '-' + (Date.now() % 1000), name, '1.21.4', null, 0, 'fabric', '0.16.9');
        // Preview art: FTB packs get only an icon, others a wide banner.
        const ftbPack = String(a.projectId ?? '').startsWith('ftb:');
        inst.pack = {
          name,
          versionNumber: '14.0.0',
          projectId: a.projectId,
          versionId: a.versionId,
          iconUrl: ftbPack ? '/sources/ftb.png' : null,
          bannerUrl: ftbPack ? null : '/hero.jpg'
        };
        instances = [...instances, inst];
        setTimeout(async () => {
          await fakeInstall(inst.id);
          // Preview: CurseForge packs come with two files that need a manual download.
          if (String(a.projectId ?? '').startsWith('cf:')) {
            const files: ManualFile[] = [
              { name: 'OptiFine-ish Shaders', url: 'https://www.curseforge.com/minecraft/mc-mods/example-a/download/1', fileName: 'example-a-1.2.jar', sha1: null, size: 1, folder: 'mods', done: false },
              { name: 'Faithful Textures', url: 'https://www.curseforge.com/minecraft/texture-packs/example-b/download/2', fileName: 'example-b.zip', sha1: null, size: 1, folder: 'resourcepacks', done: false }
            ];
            mockManual[inst.id] = files;
            emit('install-blocked', { instanceId: inst.id, name: inst.name, files });
          }
        }, 50);
        return inst;
      }
      case 'manual_list':
        return mockManual[a.instanceId] ?? [];
      case 'manual_scan': {
        // Preview: every third scan "finds" the next file, as if the user downloaded it.
        const list = mockManual[a.instanceId] ?? [];
        mockScans[a.instanceId] = (mockScans[a.instanceId] ?? 0) + 1;
        if (mockScans[a.instanceId] % 3 === 0) {
          const next = list.find((f) => !f.done);
          if (next) next.done = true;
        }
        const out = list.map((f) => ({ ...f }));
        if (list.length && list.every((f) => f.done)) delete mockManual[a.instanceId];
        return out;
      }
      case 'manual_dismiss':
        delete mockManual[a.instanceId];
        return;
      case 'refresh_pack_art':
        return instances.find((i) => i.id === a.id);
      case 'open_downloads_folder':
        return;
      case 'export_entries':
        return [
          { name: 'config', isDir: true, suggested: true },
          { name: 'kubejs', isDir: true, suggested: true },
          { name: 'saves', isDir: true, suggested: false },
          { name: 'screenshots', isDir: true, suggested: false },
          { name: 'options.txt', isDir: false, suggested: false },
          { name: 'servers.dat', isDir: false, suggested: false }
        ] satisfies ExportEntry[];
      case 'serverpack_analyze':
        await sleep(900);
        return [
          { fileName: 'sodium-0.6.5.jar', name: 'Sodium', clientOnly: true, sure: true, reason: 'Marked client-only in fabric.mod.json' },
          { fileName: 'iris-1.8.1.jar', name: 'Iris', clientOnly: true, sure: true, reason: 'Modrinth: client only' },
          { fileName: 'zoomify-2.14.jar', name: 'Zoomify', clientOnly: true, sure: false, reason: 'Known client-only mod' },
          { fileName: 'fabric-api-0.110.0.jar', name: 'Fabric API', clientOnly: false, sure: true, reason: 'Modrinth: runs on servers' },
          { fileName: 'lithium-0.14.jar', name: 'Lithium', clientOnly: false, sure: true, reason: 'Modrinth: runs on servers' },
          { fileName: 'mystery-mod-1.0.jar', name: 'Mystery Mod', clientOnly: false, sure: false, reason: 'No client-only marker found, kept to be safe' }
        ] satisfies ModSide[];
      case 'serverpack_export':
        await sleep(1500);
        return { path: a.options.path, sizeBytes: 182_000_000, serverMods: 3, clientMods: a.options.clientFiles.length, javaMajor: 21 };
      case 'export_pack': {
        await sleep(1200);
        const o = a.options as ExportOptions;
        const mods = (mockContent[a.instanceId + 'mod'] ?? []).filter((m) => m.enabled);
        return {
          path: o.path,
          linked: mods.length || 12,
          bundled: o.format === 'curseforge' && !settings.curseforgeApiKey ? ['mods/sodium-5.0.1.jar'] : [],
          overrideFiles: o.include.length * 7,
          sizeBytes: 48_000 + o.include.length * 20_000
        } satisfies ExportReport;
      }
      case 'import_detect': {
        await sleep(500);
        const c = (
          source: ImportCandidate['source'],
          sourceLabel: string,
          name: string,
          gameVersion: string,
          loader: ImportCandidate['loader'],
          loaderVersion: string | null,
          path: string,
          playtimeSecs = 0,
          days: number | null = null
        ): ImportCandidate => ({
          source,
          sourceLabel,
          name,
          gameVersion,
          loader,
          loaderVersion,
          path,
          gameDir: path,
          playtimeSecs,
          lastPlayed: days === null ? null : iso(864e5 * days),
          memoryMb: null,
          jvmArgs: null,
          alreadyImported: instances.some((i) => i.importedFrom === path)
        });
        return [
          c('prism', 'Prism Launcher', 'All the Mods 10', '1.21.1', 'neoforge', '21.1.77', String.raw`C:\Users\you\AppData\Roaming\PrismLauncher\instances\ATM10`, 86400 * 2, 3),
          c('prism', 'Prism Launcher', 'Fabulously Optimized', '1.21.4', 'fabric', '0.16.9', String.raw`C:\Users\you\AppData\Roaming\PrismLauncher\instances\FO`, 7200, 12),
          c('curseforge', 'CurseForge', 'RLCraft', '1.12.2', 'forge', '1.12.2-14.23.5.2860', String.raw`C:\Users\you\curseforge\minecraft\Instances\RLCraft`, 0, 40),
          c('official', 'Minecraft Launcher', 'Minecraft 1.21.4', '1.21.4', 'vanilla', null, String.raw`C:\Users\you\AppData\Roaming\.minecraft`)
        ];
      }
      case 'import_inspect':
        return {
          source: 'multimc',
          sourceLabel: 'MultiMC / PolyMC',
          name: 'Better Survival',
          gameVersion: '1.20.1',
          loader: 'forge',
          loaderVersion: '1.20.1-47.3.0',
          path: a.path,
          gameDir: a.path + '/.minecraft',
          playtimeSecs: 0,
          lastPlayed: null,
          memoryMb: 6144,
          jvmArgs: null,
          alreadyImported: false
        } satisfies ImportCandidate;
      case 'import_instances': {
        const made = (a.candidates as ImportCandidate[]).map((c) => {
          const inst = mk(pid(c.name).slice(2) + '-' + (Date.now() % 1000), c.name, c.gameVersion, c.lastPlayed, c.playtimeSecs, c.loader, c.loaderVersion);
          inst.importedFrom = c.path;
          inst.installed = false;
          return inst;
        });
        instances = [...instances, ...made];
        made.forEach((m, i) => setTimeout(() => fakeInstall(m.id), 100 + i * 200));
        return made;
      }
      case 'open_content_folder':
        return;
      case 'list_worlds':
        return ['New World', 'Hardcore Run', 'Redstone Lab'];
      case 'java_runtimes':
        return [
          { component: 'jre-legacy', major: 8, installed: true },
          { component: 'java-runtime-gamma', major: 17, installed: false },
          { component: 'java-runtime-delta', major: 21, installed: true }
        ];
      case 'detect_java':
        await sleep(600);
        return [
          { path: String.raw`C:\Program Files\Eclipse Adoptium\jdk-21\bin\javaw.exe`, version: '21.0.5', major: 21 },
          { path: String.raw`C:\Program Files\Java\jre1.8.0_421\bin\javaw.exe`, version: '1.8.0_421', major: 8 }
        ];
      case 'test_java':
        return { path: a.path, version: '21.0.5', major: 21 };
      case 'create_instance': {
        const inst = mk(a.name.toLowerCase().replace(/[^a-z0-9]+/g, '-') + '-' + (Date.now() % 1000), a.name, a.gameVersion, null, 0, a.loader, a.loaderVersion);
        instances = [...instances, inst];
        return inst;
      }
      case 'update_instance':
        instances = instances.map((i) => (i.id === a.instance.id ? { ...a.instance } : i));
        return instances.find((i) => i.id === a.instance.id);
      case 'duplicate_instance': {
        const src = instances.find((i) => i.id === a.id)!;
        const copy = { ...src, id: src.id + '-copy' + (Date.now() % 1000), name: `${src.name} (copy)`, lastPlayed: null, playtimeSecs: 0 };
        instances = [...instances, copy];
        return copy;
      }
      case 'delete_instance':
        instances = instances.filter((i) => i.id !== a.id);
        return;
      case 'install_instance':
        await fakeInstall(a.id);
        return instances.find((i) => i.id === a.id);
      case 'launch_instance': {
        const inst = instances.find((i) => i.id === a.id)!;
        if (!inst.installed) await fakeInstall(a.id);
        running.add(a.id);
        emit('game-log-reset', { instanceId: a.id });
        emit('game-log', { instanceId: a.id, line: `[Peak] Launching ${inst.name} (${inst.gameVersion})`, stream: 'launcher' } satisfies LogLine & { instanceId: string });
        emit('game-state', { instanceId: a.id, state: 'running', exitCode: null });
        instances = instances.map((i) => (i.id === a.id ? { ...i, lastPlayed: new Date().toISOString() } : i));
        logs.forEach((line, n) =>
          setTimeout(() => emit('game-log', { instanceId: a.id, line, stream: line.includes('WARN') ? 'stderr' : 'stdout' }), 300 * (n + 1))
        );
        return;
      }
      case 'stop_instance':
        running.delete(a.id);
        emit('game-state', { instanceId: a.id, state: 'stopped', exitCode: 0 });
        return;
      case 'running_instances':
        return [...running];
      case 'game_logs':
        // The preview's pop-out is a separate page with its own mock, so show sample output.
        return [
          { line: '[Peak] Launching Survival Island (1.21.4)', stream: 'launcher' },
          ...logs.map((line) => ({ line, stream: line.includes('WARN') ? 'stderr' : 'stdout' }))
        ];
      case 'open_console_window': {
        const w = window.open(`/?console=${encodeURIComponent(a.id)}`, `console-${a.id}`, 'popup,width=920,height=600');
        popouts[a.id] = w;
        emit('console-window', { instanceId: a.id, open: true });
        // Notice when the preview's pop-up is closed.
        const timer = setInterval(() => {
          if (!w || w.closed) {
            clearInterval(timer);
            delete popouts[a.id];
            emit('console-window', { instanceId: a.id, open: false });
          }
        }, 500);
        return;
      }
      case 'close_console_window':
        popouts[a.id]?.close();
        return;
      case 'list_accounts':
        return accounts;
      case 'ms_login_start':
        loginCancelled = false;
        return {
          device: { deviceCode: 'x', userCode: '83Y72FXA', verificationUri: 'https://www.microsoft.com/link', expiresIn: 900, interval: 5 },
          browserUrl: 'https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize?client_id=preview&response_type=code'
        };
      case 'servers_list':
        return mockServers;
      case 'server_save': {
        const s = { ...(a.server as SavedServer) };
        if (!s.address.trim()) throw new Error("Enter the server's address.");
        if (!s.name.trim()) s.name = s.address;
        if (!s.id) mockServers = [...mockServers, { ...s, id: 's' + Date.now() }];
        else mockServers = mockServers.map((x) => (x.id === s.id ? s : x));
        return mockServers;
      }
      case 'server_delete':
        mockServers = mockServers.filter((x) => x.id !== a.id);
        return mockServers;
      case 'server_ping': {
        await sleep(300 + Math.random() * 700);
        const known = mockStatus[a.address as string];
        if (known) return known;
        return { online: false, error: 'Nothing is running at that address', version: null, protocol: null, playersOnline: null, playersMax: null, playersSample: [], motd: null, favicon: null, pingMs: null } satisfies ServerStatus;
      }
      case 'take_pending_link':
        // Preview: add #demo-link to the URL to act as if opened from a peak:// link.
        return location.hash === '#demo-link' ? 'l3' : null;
      case 'discover_list':
        await sleep(300);
        return mockListings;
      case 'discover_join': {
        const inst = instances.find((i) => i.id === a.instanceId)!;
        return call('launch_instance', { id: inst.id });
      }
      case 'instance_servers':
        return (mockInstanceServers[a.id] ??= [
          { name: 'Peak Survival', address: 'play.peak-example.net' },
          { name: 'Creative Plots', address: 'creative.example.org:25570' }
        ]);
      case 'instance_server_add': {
        const l = (mockInstanceServers[a.id] ??= []);
        if (l.some((s) => s.address === a.address)) throw "That server is already in this instance's list.";
        l.push({ name: a.name || a.address, address: a.address });
        return [...l];
      }
      case 'instance_server_remove':
        mockInstanceServers[a.id] = (mockInstanceServers[a.id] ?? []).filter((s) => s.address !== a.address);
        return mockInstanceServers[a.id];
      case 'instance_servers_reorder': {
        const l = mockInstanceServers[a.id] ?? [];
        mockInstanceServers[a.id] = (a.addresses as string[]).map((ad) => l.find((s) => s.address === ad)!).filter(Boolean);
        return mockInstanceServers[a.id];
      }
      case 'instance_server_join':
        return call('launch_instance', { id: a.id });
      case 'servers_reorder':
        mockServers = (a.ids as string[]).map((id) => mockServers.find((s) => s.id === id)!).filter(Boolean);
        return mockServers;
      case 'servers_from_instance':
        return [
          ['Friends SMP', 'smp.example.net'],
          ['Local test', 'localhost:25566']
        ];
      case 'server_join': {
        const inst = instances.find((i) => i.id === a.instanceId)!;
        mockServers = mockServers.map((x) => (x.id === a.serverId ? { ...x, instanceId: a.instanceId } : x));
        return call('launch_instance', { id: inst.id });
      }
      case 'appimage_menu_status':
        // Preview: add #demo-appimage to the URL to act like the Linux AppImage.
        return { isAppImage: location.hash === '#demo-appimage', installed: mockMenu, upToDate: mockMenu } satisfies MenuStatus;
      case 'appimage_menu_install':
        mockMenu = true;
        return { isAppImage: true, installed: true, upToDate: true } satisfies MenuStatus;
      case 'appimage_menu_remove':
        mockMenu = false;
        return { isAppImage: true, installed: false, upToDate: false } satisfies MenuStatus;
      case 'skin_wardrobe':
        await sleep(400);
        return { skin: { png: sampleSkin(), model: 'classic' }, capes: [{ id: 'c1', name: 'Migrator', active: false, png: null }] } satisfies Wardrobe;
      case 'skin_read_file':
        return sampleSkin('#c0392b');
      case 'skin_upload':
        await sleep(800);
        return { skin: { png: sampleSkin('#c0392b'), model: a.model }, capes: [{ id: 'c1', name: 'Migrator', active: false, png: null }] } satisfies Wardrobe;
      case 'skin_set_model':
        await sleep(500);
        return { skin: { png: sampleSkin(), model: a.model }, capes: [{ id: 'c1', name: 'Migrator', active: false, png: null }] } satisfies Wardrobe;
      case 'skin_reset':
        await sleep(500);
        return { skin: null, capes: [{ id: 'c1', name: 'Migrator', active: false, png: null }] } satisfies Wardrobe;
      case 'skin_set_cape':
        await sleep(500);
        return { skin: { png: sampleSkin(), model: 'classic' }, capes: [{ id: 'c1', name: 'Migrator', active: a.capeId === 'c1', png: null }] } satisfies Wardrobe;
      case 'skin_history': {
        await sleep(200);
        const shirts: [string, 'classic' | 'slim'][] = [['#4b8fd6', 'classic'], ['#c0392b', 'slim'], ['#27ae60', 'classic'], ['#8e44ad', 'classic']];
        return shirts.map(([shirt, model], i) => ({ id: String(i).repeat(64), png: sampleSkin(shirt), model, usedAt: new Date(Date.now() - i * 86400000 * 3).toISOString() })) satisfies PastSkin[];
      }
      case 'skin_use_saved':
        await sleep(700);
        return { skin: { png: sampleSkin('#27ae60'), model: a.model }, capes: [{ id: 'c1', name: 'Migrator', active: false, png: null }] } satisfies Wardrobe;
      case 'skin_history_remove':
        return null;
      case 'player_lookup': {
        await sleep(500);
        const name = String(a.name ?? '').trim();
        if (!/^\w{1,16}$/.test(name)) throw new Error('Minecraft names are up to 16 letters, numbers and underscores.');
        if (name.toLowerCase() === 'nobody') throw new Error(`No Minecraft player is called "${name}".`);
        return {
          id: '853c80ef-3c37-49fd-aa49-938b674adae6',
          name,
          skin: { png: sampleSkin('#e67e22'), model: 'classic' },
          cape: null,
          names: [
            { name, changedAt: '2023-08-06T23:31:18+02:00' },
            { name: 'OldName_2019', changedAt: '2019-03-02T10:00:00+01:00' },
            { name: 'FirstName', changedAt: null }
          ],
          pastSkins: [
            { png: sampleSkin('#e67e22'), model: 'classic' },
            { png: sampleSkin('#16a085'), model: 'slim' },
            { png: sampleSkin('#2c3e50'), model: 'classic' }
          ],
          capes: [],
          historyFound: true
        } satisfies Player;
      }
      case 'instance_set_picture': {
        const inst = instances.find((i) => i.id === a.id)!;
        if (a.kind === 'icon') inst.customIcon = a.path ? 'peak-icon.png' : null;
        else inst.customBanner = a.path ? 'peak-banner.png' : null;
        return inst;
      }
      case 'presence_show':
      case 'presence_clear':
        return null;
      case 'account_cape':
        return null;
      case 'account_skin':
        // Preview: a made-up skin so heads and the 3D model have something to show.
        return { png: sampleSkin(), model: 'classic' } satisfies Skin;
      case 'ms_login_cancel':
        loginCancelled = true;
        return;
      case 'ms_login_finish':
        // Wait like the real thing (it needs the user), but stop if cancelled.
        for (let i = 0; i < 100; i++) {
          if (loginCancelled) throw new Error('Sign-in cancelled.');
          await sleep(250);
        }
        accounts = [...accounts.map((x) => ({ ...x, active: false })), { id: 'a' + Date.now(), username: 'NewPlayer', kind: 'microsoft', active: true }];
        return accounts;
      case 'add_offline_account':
        accounts = [...accounts.map((x) => ({ ...x, active: false })), { id: 'o' + Date.now(), username: a.username, kind: 'offline', active: true }];
        return accounts;
      case 'set_active_account':
        accounts = accounts.map((x) => ({ ...x, active: x.id === a.id }));
        return accounts;
      case 'remove_account':
        accounts = accounts.filter((x) => x.id !== a.id);
        if (accounts.length && !accounts.some((x) => x.active)) accounts[0].active = true;
        return accounts;
      case 'app_info':
        return {
          version: '0.1.0',
          devBuild: true,
          msConfigured: true,
          tokensEncrypted: true,
          os: 'windows',
          totalMemoryMb: 32768,
          dataDir: String.raw`C:\Users\you\AppData\Roaming\PeakLauncher`,
          jvmPresets: [
            { id: 'none', name: 'None', description: "Java's own defaults. A good choice for most players and vanilla.", flags: '' },
            { id: 'balanced', name: 'Smooth (G1)', description: 'Short garbage collection pauses for steadier frame times. Works on every Java version.', flags: DEFAULT_ARGS },
            { id: 'aikar', name: "Aikar's flags", description: 'A popular G1 set for large modpacks with lots of memory. Works on every Java version.', flags: '-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200 ...' },
            { id: 'zgc', name: 'ZGC (low latency)', description: 'Very short pauses on newer Java (17+). Uses more memory; best with 8 GB or more. Older Minecraft on Java 8 falls back to Smooth (G1).', flags: '-XX:+UseZGC' }
          ],
          curseforgeBuiltIn: false
        } satisfies AppInfo;
      case 'get_settings':
        return settings;
      case 'save_settings':
        settings = a.settings;
        return settings;
      case 'open_instance_folder':
        return;
    }
    throw new Error(`mock: unknown command ${cmd}`);
  }

  function on(event: string, handler: Handler<unknown>) {
    if (!listeners.has(event)) listeners.set(event, new Set());
    listeners.get(event)!.add(handler);
    return () => listeners.get(event)!.delete(handler);
  }

  return { call, on };
})();
