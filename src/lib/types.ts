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
  /** Java flag preset for this instance; null = the global one. */
  jvmPreset: string | null;
  javaPath: string | null;
  /** A Mojang runtime, or "mojang" / "temurin" / "graalvm" (that build, in
   *  the Java version the game needs); null = the global choice. */
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
  /** The player's own logo picture (a file in the instance folder). */
  customIcon?: string | null;
  /** The player's own banner picture (a file in the instance folder). */
  customBanner?: string | null;
  /** What this instance shares with the others. */
  shared?: SharedParts;
}

/** What the crash helper found after a game closed with an error. */
/** A search for the mod that breaks an instance (see ModSearchDialog). */
export interface ModSearch {
  phase: 'allOff' | 'halves' | 'found' | 'notAMod';
  /** The test being run now (1-based). */
  test: number;
  /** Roughly how many tests it takes in all. */
  tests: number;
  mods: number;
  suspects: number;
  off: number;
  culprits: { fileName: string; name: string }[];
  needsIt: { fileName: string; name: string }[];
}

export interface CrashDiagnosis {
  kind: 'missing-mod' | 'wrong-version' | 'java' | 'memory' | 'duplicate' | 'mod-crash' | 'unknown';
  title: string;
  detail: string;
  fix: CrashFix | null;
  report: string | null;
  excerpt: string[];
}

export type CrashFix =
  | { action: 'installMod'; modId: string; name: string }
  | { action: 'memory'; mb: number }
  | { action: 'java'; major: number }
  | { action: 'changeVersion'; fileName: string; name: string }
  | { action: 'disableMod'; fileName: string; name: string };

/** Performance mods the Optimize dialog can add. */
export interface PerfMods {
  mods: { projectId: string; title: string; what: string; status: 'add' | 'installed' | 'clash' | 'unavailable'; note: string | null }[];
  /** Titles of mods added earlier that Undo would take out. */
  added: string[];
}

/** What takes up space in Peak's folder. */
export interface StorageUsage {
  total: number;
  parts: { id: string; name: string; bytes: number }[];
  instances: { id: string; name: string; bytes: number }[];
  cleanable: { id: string; name: string; desc: string; bytes: number; more: { id: string; label: string; bytes: number } | null }[];
}

/** The saved default keybinds and game settings. */
export interface GameDefaults {
  from: string;
  savedAt: string;
  keybinds: number;
  settings: number;
  fov: number | null;
  renderDistance: number | null;
  guiScale: string | null;
  /** Changed in Peak since (or instead of) being copied from an instance. */
  edited: boolean;
  /** Keybinds left out of a copy from Minecraft 1.12 or older. */
  skippedKeybinds?: number;
}

/** Default settings and shared packs, per instance. */
export interface SharedParts {
  /** Use the default keybinds. */
  keybinds: boolean;
  /** Use the default game settings (the rest of options.txt). */
  options: boolean;
  resourcepacks: boolean;
  shaderpacks: boolean;
}

export const NO_SHARING: SharedParts = { keybinds: false, options: false, resourcepacks: false, shaderpacks: false };

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
  /** An update the player chose to skip (its version id). */
  skippedVersion?: string | null;
  /** The player asked not to hear about updates to this pack. */
  noUpdatePrompts?: boolean;
  /** The version before the last version change, to go back to. */
  previousVersionId?: string | null;
  previousVersionNumber?: string | null;
}

/** A modpack instance with a newer version available. */
export interface PackUpdate {
  instanceId: string;
  instanceName: string;
  packName: string;
  current: string;
  latestId: string;
  latest: string;
  iconUrl: string | null;
  /** The player skipped this version. */
  skipped?: boolean;
  /** The player turned update reminders off for this pack. */
  muted?: boolean;
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

/** A Java flag preset (see the launcher's jvmflags module). */
export interface JvmPreset {
  id: string;
  name: string;
  description: string;
  flags: string;
}

/** A server in an instance's own (in-game) multiplayer list. */
export interface InstanceServer {
  name: string;
  address: string;
}

/** A server in the Servers tab. */
export interface SavedServer {
  id: string;
  name: string;
  address: string;
  /** Instance last used to join it. */
  instanceId: string | null;
  /** Starred: shown at the top of the list. */
  favorite?: boolean;
}

/** A CurseForge pack the player has to download by hand. */
export interface ManualPack {
  name: string;
  version: string;
  fileId: string;
  fileName: string;
  size: number;
  sha1: string | null;
  downloadPage: string;
  iconUrl: string | null;
}

/** A server on the Discover list. */
export interface Listing {
  id: string;
  name: string;
  address: string;
  tagline: string;
  description: string;
  tags: string[];
  region: string;
  language: string;
  /** Minecraft version to join with. */
  gameVersion: string;
  loader: LoaderId;
  /** The modpack the server runs, installed before joining. */
  pack: { projectId: string; name: string; iconUrl: string | null; versionId?: string | null; versionName?: string | null } | null;
  votes: number;
  featured: boolean;
  verified: boolean;
  website: string | null;
  discord: string | null;
  /** The owner's banner, if a moderator approved one. */
  bannerUrl?: string | null;
  /** The server's page on the website, where players vote. */
  pageUrl?: string | null;
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

/** Linux AppImage: whether Peak has an applications-menu entry. */
export interface MenuStatus {
  isAppImage: boolean;
  installed: boolean;
  /** The entry points at this AppImage (false after it was moved). */
  upToDate: boolean;
}

/** A cape the account owns. */
export interface Cape {
  id: string;
  name: string;
  active: boolean;
  png: string | null;
}

/** An account's current skin (null = Minecraft's default) and its capes. */
/** A skin the account wore in Peak before (Peak keeps its own history). */
export interface PastSkin {
  id: string;
  png: string;
  model: 'slim' | 'classic';
  usedAt: string;
}

/** Any player, looked up by name. */
export interface Player {
  id: string;
  name: string;
  skin: Skin | null;
  cape: string | null;
  /** Names used, newest first (from Crafty.gg). */
  names: PastName[];
  /** Skins worn, newest first (from Crafty.gg). */
  pastSkins: Skin[];
  /** Capes the player has, the worn one first (from Crafty.gg). */
  capes: PastCape[];
  /** False when Crafty.gg couldn't be reached. */
  historyFound: boolean;
}

export interface PastCape {
  name: string;
  png: string;
  current: boolean;
}

export interface PastName {
  name: string;
  /** When Crafty.gg recorded the change; null for the first name. */
  changedAt: string | null;
}

export interface Wardrobe {
  skin: Skin | null;
  capes: Cape[];
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
  /** Java flag preset: 'none' | 'balanced' | 'aikar' | 'zgc'. */
  jvmPreset: string;
  useBundledJava: boolean;
  javaPath: string;
  /** Which Java build automatic Java uses. */
  javaVendor: JavaVendor;
  /** Picked a Java build (asked once, on first play). */
  javaVendorChosen: boolean;
  /** What new instances share with the others. */
  sharedDefault: SharedParts;
  windowWidth: number;
  windowHeight: number;
  fullscreen: boolean;
  afterLaunch: 'open' | 'minimize' | 'hide' | 'close';
  openConsole: boolean;
  /** Look for a new version of Peak on start-up and every few hours. */
  autoUpdateCheck: boolean;
  /** The AppImage "add to applications menu" offer was dismissed. */
  appimageMenuDismissed: boolean;
  /** Tell the player when an installed modpack has a new version. */
  modpackUpdateCheck: boolean;
  /** Show what the player is doing on their Discord profile. */
  discordPresence: boolean;
  /** Join button on the Discord status for any server, not only listed ones. */
  discordJoinAnyServer: boolean;
  /** No blur or moving glow, for slower PCs (off unless turned on). */
  lightEffects: boolean;
  /** The aurora glow drifts slowly (off by default on Linux). */
  movingGlow: boolean;
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
  jvmPresets: JvmPreset[];
  curseforgeBuiltIn: boolean;
}

export type View = 'home' | 'library' | 'instance' | 'browse' | 'servers' | 'accounts' | 'screenshots' | 'settings';

/** Java builds automatic Java can use. */
export type JavaVendor = 'mojang' | 'temurin' | 'graalvm';

export interface InstalledJava {
  id: string;
  vendor: string;
  label: string;
  sizeBytes: number;
}

/** What the Optimize button suggests for an instance. */
export interface TuningSuggestion {
  memoryMb: number;
  jvmPreset: string;
  graalvm: boolean;
  mods: number;
  shaders: boolean;
  javaMajor: number;
  totalMemoryMb: number;
  reasons: string[];
}

export interface Screenshot {
  key: string;
  instanceId: string;
  file: string;
  path: string;
  takenAt: string;
  sizeBytes: number;
  tags: string[];
}

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
  /** Launchers can't download it: `url` is the page to get it from in the browser. */
  manual?: boolean;
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
  /** Modrinth only: required, optional, unsupported or unknown. */
  client_side?: string | null;
  server_side?: string | null;
}

/** The mod search's "runs on" filter. */
export type SideFilter = 'any' | 'client' | 'server' | 'both';

/** A short label for where a mod runs, or null if it runs on both or we can't tell. */
export function sideLabel(hit: Pick<SearchHit, 'client_side' | 'server_side'>): string | null {
  const runs = (v?: string | null) => v === 'required' || v === 'optional';
  if (runs(hit.client_side) && hit.server_side === 'unsupported') return 'Client only';
  if (runs(hit.server_side) && hit.client_side === 'unsupported') return 'Server only';
  return null;
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
