// Thin wrapper over Tauri commands. When the UI runs in a normal browser
// (e.g. `npm run dev` without Tauri) it falls back to an in-memory mock so the
// interface can be designed and tested without the Rust backend.
import type {
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
  contentSearch: (source: Source, instanceId: string, kind: ContentKind, query: string, sort: string, offset: number) =>
    call<SearchResult>('content_search', { source, instanceId, kind, query, sort, offset }),
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
  contentRemove: (instanceId: string, kind: ContentKind, fileName: string) =>
    call<void>('content_remove', { instanceId, kind, fileName }),
  contentUpdates: (instanceId: string, kind: ContentKind) => call<UpdateInfo[]>('content_updates', { instanceId, kind }),
  openContentFolder: (instanceId: string, kind: ContentKind) => call<void>('open_content_folder', { instanceId, kind }),
  modpackSearch: (source: Source, query: string, sort: string, offset: number) =>
    call<SearchResult>('modpack_search', { source, query, sort, offset }),
  modpackVersions: (projectId: string) => call<ContentVersion[]>('modpack_versions', { projectId }),
  modpackInstall: (opts: { projectId?: string; versionId?: string | null; filePath?: string }) =>
    call<Instance>('modpack_install', {
      projectId: opts.projectId ?? null,
      versionId: opts.versionId ?? null,
      filePath: opts.filePath ?? null
    }),
  exportEntries: (instanceId: string) => call<ExportEntry[]>('export_entries', { instanceId }),
  exportPack: (instanceId: string, options: ExportOptions) => call<ExportReport>('export_pack', { instanceId, options }),
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
  stop: (id: string) => call<void>('stop_instance', { id }),
  running: () => call<string[]>('running_instances'),
  gameLogs: (id: string) => call<LogLine[]>('game_logs', { id }),
  openConsoleWindow: (id: string) => call<void>('open_console_window', { id }),
  accounts: () => call<AccountInfo[]>('list_accounts'),
  msLoginStart: () => call<DeviceCode>('ms_login_start'),
  msLoginFinish: (code: DeviceCode) => call<AccountInfo[]>('ms_login_finish', { code }),
  addOffline: (username: string) => call<AccountInfo[]>('add_offline_account', { username }),
  setActiveAccount: (id: string) => call<AccountInfo[]>('set_active_account', { id }),
  removeAccount: (id: string) => call<AccountInfo[]>('remove_account', { id }),
  info: () => call<AppInfo>('app_info'),
  settings: () => call<Settings>('get_settings'),
  saveSettings: (settings: Settings) => call<Settings>('save_settings', { settings }),
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
  let settings: Settings = {
    accent: 'glacier',
    background: 'aurora',
    memoryMb: 4096,
    metaspaceMb: 256,
    jvmArgs: DEFAULT_ARGS,
    useBundledJava: true,
    javaPath: '',
    windowWidth: 1280,
    windowHeight: 720,
    fullscreen: false,
    afterLaunch: 'open',
    openConsole: true,
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
  const mockContent: Record<string, InstalledItem[]> = {};
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
            date_modified: iso(864e5 * (i + 2))
          }));
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
              { name: 'OptiFine-ish Shaders', url: 'https://www.curseforge.com/minecraft/mc-mods/example-a/files/1', fileName: 'example-a-1.2.jar', sha1: null, size: 1, folder: 'mods', done: false },
              { name: 'Faithful Textures', url: 'https://www.curseforge.com/minecraft/texture-packs/example-b/files/2', fileName: 'example-b.zip', sha1: null, size: 1, folder: 'resourcepacks', done: false }
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
      case 'open_console_window':
        window.open(`/?console=${encodeURIComponent(a.id)}`, `console-${a.id}`, 'popup,width=920,height=600');
        return;
      case 'list_accounts':
        return accounts;
      case 'ms_login_start':
        return { deviceCode: 'x', userCode: 'PK7-Q2MX', verificationUri: 'https://www.microsoft.com/link', expiresIn: 900, interval: 5 };
      case 'ms_login_finish':
        await sleep(4000);
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
          msConfigured: false,
          os: 'windows',
          totalMemoryMb: 32768,
          dataDir: String.raw`C:\Users\you\AppData\Roaming\PeakLauncher`,
          defaultJvmArgs: DEFAULT_ARGS,
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
