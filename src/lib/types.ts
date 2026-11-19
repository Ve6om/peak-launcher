export type LoaderId = 'vanilla' | 'fabric' | 'quilt' | 'forge' | 'neoforge';

export interface QuickPlay {
  mode: '' | 'none' | 'world' | 'server' | 'realm';
  value: string;
}

export interface Instance {
  id: string;
  name: string;
  gameVersion: string;
  loader: LoaderId;
  loaderVersion: string | null;
  createdAt: string;
  lastPlayed: string | null;
  playtimeSecs: number;
  installed: boolean;
  versionId: string | null;
  notes: string;
  // Overrides: null = use global setting
  memoryMb: number | null;
  metaspaceMb: number | null;
  jvmArgs: string | null;
  javaPath: string | null;
  javaRuntime: string | null;
  accountId: string | null;
  quickPlay: QuickPlay;
  windowWidth: number | null;
  windowHeight: number | null;
  fullscreen: boolean | null;
  enableCommands: boolean | null;
  preLaunchCommand: string | null;
  postExitCommand: string | null;
  wrapperCommand: string | null;
  useSystemGlfw: boolean | null;
  useSystemOpenal: boolean | null;
  pack: PackInfo | null;
  importedFrom: string | null;
}

export interface ExportEntry {
  name: string;
  isDir: boolean;
  suggested: boolean;
}

export interface ExportOptions {
  format: 'modrinth' | 'curseforge';
  name: string;
  version: string;
  author: string;
  summary: string;
  include: string[];
  path: string;
}

export interface ExportReport {
  path: string;
  linked: number;
  bundled: string[];
  overrideFiles: number;
  sizeBytes: number;
}

/** An instance found in another launcher. */
export interface ImportCandidate {
  source: 'prism' | 'multimc' | 'atlauncher' | 'curseforge' | 'modrinth' | 'official';
  sourceLabel: string;
  path: string;
  gameDir: string;
  name: string;
  gameVersion: string;
  loader: LoaderId;
  loaderVersion: string | null;
  playtimeSecs: number;
  lastPlayed: string | null;
  memoryMb: number | null;
  jvmArgs: string | null;
  alreadyImported: boolean;
}

export interface PackInfo {
  name: string;
  versionNumber: string;
  projectId: string | null;
  versionId: string | null;
  iconUrl: string | null;
  bannerUrl: string | null;
  description?: string | null;
}

/** Pack version without the file extension or a repeated pack name. */
export function packVersion(pack: PackInfo | null | undefined): string {
  if (!pack?.versionNumber) return '';
  let v = pack.versionNumber.replace(/\.(zip|mrpack)$/i, '').trim();
  if (v.toLowerCase().startsWith(pack.name.toLowerCase())) v = v.slice(pack.name.length).trim();
  return v.replace(/^[-_:\s]+/, '') || pack.versionNumber;
}

export interface ManifestEntry {
  id: string;
  type: 'release' | 'snapshot' | 'old_beta' | 'old_alpha';
  url: string;
  releaseTime: string;
}

export interface VersionManifest {
  latest: { release: string; snapshot: string };
  versions: ManifestEntry[];
}

export interface LoaderVersion {
  id: string;
  stable: boolean;
  recommended: boolean;
}

export interface RuntimeOption {
  component: string;
  major: number;
  installed: boolean;
}

export interface DetectedJava {
  path: string;
  version: string;
  major: number;
}

/** A server in the Servers tab. */
export interface SavedServer {
  id: string;
  name: string;
  address: string;
  /** Instance last used to join it. */
  instanceId: string | null;
}

/** Live status straight from the server. */
export interface ServerStatus {
  online: boolean;
  error: string | null;
  version: string | null;
  protocol: number | null;
  playersOnline: number | null;
  playersMax: number | null;
  playersSample: string[];
  /** Minecraft text: a string with § codes or a text component. */
  motd: unknown;
  favicon: string | null;
  pingMs: number | null;
}

/** An account's skin, as a data: URL, and its arm model. */
export interface Skin {
  png: string;
  model: 'slim' | 'classic';
}

export interface AccountInfo {
  id: string;
  username: string;
  kind: 'microsoft' | 'offline';
  active: boolean;
}

export interface DeviceCode {
  deviceCode: string;
  userCode: string;
  verificationUri: string;
  expiresIn: number;
  interval: number;
}

/** What the sign-in window needs: the code for another device, plus the page for this computer's browser. */
export interface LoginStart {
  device: DeviceCode;
  browserUrl: string | null;
}

export interface Settings {
  accent: string;
  background: 'aurora' | 'contours' | 'both';
  memoryMb: number;
  metaspaceMb: number;
  jvmArgs: string;
  useBundledJava: boolean;
  javaPath: string;
  windowWidth: number;
  windowHeight: number;
  fullscreen: boolean;
  afterLaunch: 'open' | 'minimize' | 'hide' | 'close';
  openConsole: boolean;
  enableCommands: boolean;
  preLaunchCommand: string;
  postExitCommand: string;
  wrapperCommand: string;
  gamemode: boolean;
  useSystemGlfw: boolean;
  useSystemOpenal: boolean;
  concurrentDownloads: number;
  connectionTimeout: number;
  proxyEnabled: boolean;
  proxyType: 'http' | 'socks5';
  proxyHost: string;
  proxyPort: number;
  curseforgeApiKey: string;
}

export interface InstallProgress {
  instanceId: string;
  stage: string;
  done: number;
  total: number;
  bytes: number;
}

export interface LogLine {
  line: string;
  stream: 'stdout' | 'stderr' | 'launcher';
}

export interface AppInfo {
  version: string;
  devBuild: boolean;
  msConfigured: boolean;
  /** Sign-in tokens are encrypted with a key in the OS password store. */
  tokensEncrypted: boolean;
  os: string;
  totalMemoryMb: number;
  dataDir: string;
  defaultJvmArgs: string;
  curseforgeBuiltIn: boolean;
}

export type View = 'home' | 'library' | 'instance' | 'browse' | 'servers' | 'accounts' | 'settings';

export const LOADERS: { id: LoaderId; label: string }[] = [
  { id: 'vanilla', label: 'Vanilla' },
  { id: 'fabric', label: 'Fabric' },
  { id: 'quilt', label: 'Quilt' },
  { id: 'neoforge', label: 'NeoForge' },
  { id: 'forge', label: 'Forge' }
];

export const loaderLabel = (id: string) => LOADERS.find((l) => l.id === id)?.label ?? id;

// ---- mods & content (Modrinth) ----

export type ContentKind = 'mod' | 'resourcepack' | 'shader';
/** What the Browse page can show: instance content, or whole modpacks. */
export type BrowseKind = ContentKind | 'modpack';
export type Source = 'modrinth' | 'curseforge' | 'ftb' | 'technic';

/** One file in an install: the requested project or a dependency it needs. */
export interface PlanItem {
  projectId: string;
  versionId: string;
  title: string;
  versionNumber: string;
  iconUrl: string | null;
  kind: ContentKind;
  /** Title of the project that needs this one; null for what was requested. */
  requiredBy: string | null;
  isRoot: boolean;
  url: string;
  fileName: string;
  sha1: string | null;
  size: number;
}
/** CurseForge ids are prefixed with "cf:". */
export const isCurseForge = (id: string | null | undefined) => !!id?.startsWith('cf:');

export interface BlockedFile {
  name: string;
  url: string;
}

/** A file the user must download by hand; Peak moves it into place once it appears. */
export interface ManualFile {
  name: string;
  url: string;
  fileName: string;
  sha1: string | null;
  size: number;
  folder: string;
  done: boolean;
}

export const CONTENT_KINDS: { id: ContentKind; label: string; single: string }[] = [
  { id: 'mod', label: 'Mods', single: 'mod' },
  { id: 'resourcepack', label: 'Resource packs', single: 'resource pack' },
  { id: 'shader', label: 'Shaders', single: 'shader' }
];

export interface SearchHit {
  project_id: string;
  slug: string;
  title: string;
  description: string;
  icon_url: string | null;
  downloads: number;
  follows: number;
  author: string;
  categories: string[];
  date_modified: string;
}

export interface SearchResult {
  hits: SearchHit[];
  total_hits: number;
  offset: number;
}

export interface Project {
  id: string;
  slug: string;
  title: string;
  description: string;
  body: string;
  icon_url: string | null;
  downloads: number;
  followers: number;
  categories: string[];
  project_type: string;
  source_url: string | null;
  issues_url: string | null;
  wiki_url: string | null;
  discord_url: string | null;
  gallery: { url: string; featured: boolean; title: string | null }[];
  updated: string | null;
  website_url: string | null;
}

export interface ContentVersion {
  id: string;
  project_id: string;
  name: string;
  version_number: string;
  version_type: 'release' | 'beta' | 'alpha';
  date_published: string;
  downloads: number;
  game_versions: string[];
  loaders: string[];
}

/** A mod sorted for a server pack. */
export interface ModSide {
  fileName: string;
  name: string;
  clientOnly: boolean;
  /** False when it's a best guess worth checking. */
  sure: boolean;
  reason: string;
}

export interface ServerPackReport {
  path: string;
  sizeBytes: number;
  serverMods: number;
  clientMods: number;
  javaMajor: number;
}

/** A mod affected by disabling, removing or enabling another one. */
export interface LinkedMod {
  fileName: string;
  name: string;
  /** "Needs Fabric API" / "Needed by Create" */
  reason: string;
}

export interface InstalledItem {
  fileName: string;
  enabled: boolean;
  size: number;
  sha1: string;
  projectId: string | null;
  versionId: string | null;
  title: string;
  versionNumber: string | null;
  iconUrl: string | null;
}

export interface UpdateInfo {
  fileName: string;
  projectId: string;
  versionId: string;
  versionNumber: string;
}
