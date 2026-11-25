// Minecraft's options.txt settings and keybinds, with friendly names, for
// editing the default game settings inside Peak. Values are stored exactly
// as Minecraft writes them.

export type OptionControl =
  | { kind: 'range'; min: number; max: number; step: number; show: (v: number) => string; toFile?: (v: number) => string; fromFile?: (s: string) => number }
  | { kind: 'toggle' }
  | { kind: 'choice'; options: { value: string; label: string }[] };

export interface GameOption {
  key: string;
  label: string;
  group: 'Video' | 'Sound' | 'Controls' | 'Chat & other';
  control: OptionControl;
  /** Minecraft's own default, used when a setting is first switched on. */
  initial: string;
}

const pct = (v: number) => `${Math.round(v * 100)}%`;
const volume = (key: string, label: string): GameOption => ({
  key: `soundCategory_${key}`,
  label,
  group: 'Sound',
  control: { kind: 'range', min: 0, max: 1, step: 0.01, show: pct },
  initial: '1.0'
});

export const GAME_OPTIONS: GameOption[] = [
  {
    key: 'renderDistance',
    label: 'Render distance',
    group: 'Video',
    control: { kind: 'range', min: 2, max: 32, step: 1, show: (v) => `${v} chunks` },
    initial: '12'
  },
  {
    key: 'simulationDistance',
    label: 'Simulation distance',
    group: 'Video',
    control: { kind: 'range', min: 5, max: 32, step: 1, show: (v) => `${v} chunks` },
    initial: '12'
  },
  {
    key: 'maxFps',
    label: 'Max framerate',
    group: 'Video',
    control: { kind: 'range', min: 10, max: 260, step: 10, show: (v) => (v >= 260 ? 'Unlimited' : `${v} fps`) },
    initial: '120'
  },
  { key: 'enableVsync', label: 'VSync', group: 'Video', control: { kind: 'toggle' }, initial: 'true' },
  {
    key: 'fov',
    label: 'FOV',
    group: 'Video',
    // Saved as -1..1 around 70.
    control: {
      kind: 'range',
      min: 30,
      max: 110,
      step: 1,
      show: (v) => (v === 70 ? 'Normal (70)' : v === 110 ? 'Quake Pro (110)' : `${v}`),
      toFile: (v) => String((v - 70) / 40),
      fromFile: (s) => Math.round(70 + parseFloat(s) * 40)
    },
    initial: '0.0'
  },
  {
    key: 'guiScale',
    label: 'GUI scale',
    group: 'Video',
    control: { kind: 'choice', options: [{ value: '0', label: 'Auto' }, ...[1, 2, 3, 4, 5, 6].map((n) => ({ value: String(n), label: String(n) }))] },
    initial: '0'
  },
  {
    key: 'gamma',
    label: 'Brightness',
    group: 'Video',
    control: { kind: 'range', min: 0, max: 1, step: 0.01, show: (v) => (v === 0 ? 'Moody' : v === 1 ? 'Bright' : pct(v)) },
    initial: '0.5'
  },
  {
    key: 'graphicsMode',
    label: 'Graphics',
    group: 'Video',
    control: { kind: 'choice', options: [{ value: '0', label: 'Fast' }, { value: '1', label: 'Fancy' }, { value: '2', label: 'Fabulous!' }] },
    initial: '1'
  },
  {
    key: 'renderClouds',
    label: 'Clouds',
    group: 'Video',
    control: { kind: 'choice', options: [{ value: '"true"', label: 'Fancy' }, { value: '"fast"', label: 'Fast' }, { value: '"false"', label: 'Off' }] },
    initial: '"true"'
  },
  {
    key: 'particles',
    label: 'Particles',
    group: 'Video',
    control: { kind: 'choice', options: [{ value: '0', label: 'All' }, { value: '1', label: 'Decreased' }, { value: '2', label: 'Minimal' }] },
    initial: '0'
  },
  { key: 'entityShadows', label: 'Entity shadows', group: 'Video', control: { kind: 'toggle' }, initial: 'true' },
  { key: 'bobView', label: 'View bobbing', group: 'Video', control: { kind: 'toggle' }, initial: 'true' },
  {
    key: 'mipmapLevels',
    label: 'Mipmap levels',
    group: 'Video',
    control: { kind: 'range', min: 0, max: 4, step: 1, show: (v) => (v === 0 ? 'Off' : `${v}`) },
    initial: '4'
  },
  { key: 'fullscreen', label: 'Fullscreen', group: 'Video', control: { kind: 'toggle' }, initial: 'false' },

  volume('master', 'Master volume'),
  volume('music', 'Music'),
  volume('record', 'Jukebox / note blocks'),
  volume('weather', 'Weather'),
  volume('block', 'Blocks'),
  volume('hostile', 'Hostile creatures'),
  volume('neutral', 'Friendly creatures'),
  volume('player', 'Players'),
  volume('ambient', 'Ambient / environment'),
  volume('voice', 'Voice / speech'),

  {
    key: 'mouseSensitivity',
    label: 'Mouse sensitivity',
    group: 'Controls',
    control: { kind: 'range', min: 0, max: 1, step: 0.01, show: (v) => `${Math.round(v * 200)}%` },
    initial: '0.5'
  },
  { key: 'invertYMouse', label: 'Invert mouse', group: 'Controls', control: { kind: 'toggle' }, initial: 'false' },
  { key: 'rawMouseInput', label: 'Raw mouse input', group: 'Controls', control: { kind: 'toggle' }, initial: 'true' },
  { key: 'autoJump', label: 'Auto-jump', group: 'Controls', control: { kind: 'toggle' }, initial: 'false' },
  { key: 'toggleCrouch', label: 'Toggle sneak', group: 'Controls', control: { kind: 'toggle' }, initial: 'false' },
  { key: 'toggleSprint', label: 'Toggle sprint', group: 'Controls', control: { kind: 'toggle' }, initial: 'false' },

  {
    key: 'chatScale',
    label: 'Chat text size',
    group: 'Chat & other',
    control: { kind: 'range', min: 0, max: 1, step: 0.01, show: pct },
    initial: '1.0'
  },
  {
    key: 'chatOpacity',
    label: 'Chat text opacity',
    group: 'Chat & other',
    control: { kind: 'range', min: 0.1, max: 1, step: 0.01, show: pct },
    initial: '1.0'
  },
  { key: 'pauseOnLostFocus', label: 'Pause when the window loses focus', group: 'Chat & other', control: { kind: 'toggle' }, initial: 'true' },
  {
    key: 'narrator',
    label: 'Narrator',
    group: 'Chat & other',
    control: {
      kind: 'choice',
      options: [
        { value: '0', label: 'Off' },
        { value: '1', label: 'Narrates all' },
        { value: '2', label: 'Narrates chat' },
        { value: '3', label: 'Narrates system' }
      ]
    },
    initial: '0'
  }
];

export interface Keybind {
  key: string;
  label: string;
  initial: string;
}

const kb = (id: string, label: string, initial: string): Keybind => ({ key: `key_key.${id}`, label, initial: `key.${initial}` });

export const KEYBINDS: Keybind[] = [
  kb('forward', 'Walk forwards', 'keyboard.w'),
  kb('left', 'Strafe left', 'keyboard.a'),
  kb('back', 'Walk backwards', 'keyboard.s'),
  kb('right', 'Strafe right', 'keyboard.d'),
  kb('jump', 'Jump', 'keyboard.space'),
  kb('sneak', 'Sneak', 'keyboard.left.shift'),
  kb('sprint', 'Sprint', 'keyboard.left.control'),
  kb('attack', 'Attack / destroy', 'mouse.left'),
  kb('use', 'Use item / place block', 'mouse.right'),
  kb('pickItem', 'Pick block', 'mouse.middle'),
  kb('drop', 'Drop selected item', 'keyboard.q'),
  kb('swapOffhand', 'Swap item with offhand', 'keyboard.f'),
  kb('inventory', 'Open / close inventory', 'keyboard.e'),
  kb('chat', 'Open chat', 'keyboard.t'),
  kb('command', 'Open command', 'keyboard.slash'),
  kb('playerlist', 'List players', 'keyboard.tab'),
  kb('socialInteractions', 'Social interactions', 'keyboard.p'),
  kb('advancements', 'Advancements', 'keyboard.l'),
  kb('screenshot', 'Take screenshot', 'keyboard.f2'),
  kb('togglePerspective', 'Toggle perspective', 'keyboard.f5'),
  kb('smoothCamera', 'Cinematic camera', 'keyboard.unknown'),
  kb('fullscreen', 'Toggle fullscreen', 'keyboard.f11'),
  kb('spectatorOutlines', 'Highlight players (spectators)', 'keyboard.unknown'),
  kb('saveToolbarActivator', 'Save hotbar', 'keyboard.c'),
  kb('loadToolbarActivator', 'Load hotbar', 'keyboard.x'),
  ...[1, 2, 3, 4, 5, 6, 7, 8, 9].map((n) => kb(`hotbar.${n}`, `Hotbar slot ${n}`, `keyboard.${n}`))
];

// ---- key names: browser key codes <-> Minecraft's names ----

const CODE_NAMES: Record<string, string> = {
  Space: 'space',
  Enter: 'enter',
  Escape: 'escape',
  Tab: 'tab',
  Backspace: 'backspace',
  Insert: 'insert',
  Delete: 'delete',
  Home: 'home',
  End: 'end',
  PageUp: 'page.up',
  PageDown: 'page.down',
  ArrowUp: 'up',
  ArrowDown: 'down',
  ArrowLeft: 'left',
  ArrowRight: 'right',
  ShiftLeft: 'left.shift',
  ShiftRight: 'right.shift',
  ControlLeft: 'left.control',
  ControlRight: 'right.control',
  AltLeft: 'left.alt',
  AltRight: 'right.alt',
  MetaLeft: 'left.win',
  MetaRight: 'right.win',
  CapsLock: 'caps.lock',
  Backquote: 'grave.accent',
  Minus: 'minus',
  Equal: 'equal',
  BracketLeft: 'left.bracket',
  BracketRight: 'right.bracket',
  Backslash: 'backslash',
  Semicolon: 'semicolon',
  Quote: 'apostrophe',
  Comma: 'comma',
  Period: 'period',
  Slash: 'slash',
  NumpadAdd: 'keypad.add',
  NumpadSubtract: 'keypad.subtract',
  NumpadMultiply: 'keypad.multiply',
  NumpadDivide: 'keypad.divide',
  NumpadDecimal: 'keypad.decimal',
  NumpadEnter: 'keypad.enter',
  NumpadEqual: 'keypad.equal',
  ScrollLock: 'scroll.lock',
  NumLock: 'num.lock',
  PrintScreen: 'print.screen',
  Pause: 'pause',
  ContextMenu: 'menu'
};

/** Minecraft's name for a key pressed in the browser, or null if it has none. */
export function keyFromEvent(e: KeyboardEvent): string | null {
  const c = e.code;
  let name: string | undefined = CODE_NAMES[c];
  if (!name && /^Key[A-Z]$/.test(c)) name = c.slice(3).toLowerCase();
  if (!name && /^Digit\d$/.test(c)) name = c.slice(5);
  if (!name && /^F\d{1,2}$/.test(c)) name = c.toLowerCase();
  if (!name && /^Numpad\d$/.test(c)) name = `keypad.${c.slice(6)}`;
  return name ? `key.keyboard.${name}` : null;
}

/** Minecraft's name for a mouse button. */
export function keyFromMouse(button: number): string {
  return ['key.mouse.left', 'key.mouse.middle', 'key.mouse.right'][button] ?? `key.mouse.${button + 1}`;
}

export const UNBOUND = 'key.keyboard.unknown';

/** "key.keyboard.left.shift" -> "Left Shift", "key.mouse.4" -> "Mouse 4". */
export function keyLabel(value: string): string {
  if (!value || value === UNBOUND) return 'Not bound';
  const m = value.match(/^key\.(keyboard|mouse)\.(.+)$/);
  if (!m) return value; // A number (old versions) or something unusual.
  const [, device, rest] = m;
  if (device === 'mouse') {
    return { left: 'Left click', right: 'Right click', middle: 'Middle click' }[rest] ?? `Mouse ${rest}`;
  }
  if (/^[a-z0-9]$/.test(rest)) return rest.toUpperCase();
  if (/^f\d+$/.test(rest)) return rest.toUpperCase();
  const special: Record<string, string> = { 'grave.accent': '`', minus: '-', equal: '=', 'left.bracket': '[', 'right.bracket': ']', backslash: '\\', semicolon: ';', apostrophe: "'", comma: ',', period: '.', slash: '/', 'left.win': 'Left Win', 'right.win': 'Right Win' };
  if (special[rest]) return special[rest];
  return rest
    .split('.')
    .map((w) => (w === 'keypad' ? 'Numpad' : w[0].toUpperCase() + w.slice(1)))
    .join(' ');
}

/** A readable name for a mod's keybind id, e.g. "key_key.journeymap.map" -> "journeymap: map". */
export function modKeyLabel(key: string): string {
  const id = key.replace(/^key_/, '').replace(/^key\./, '');
  const parts = id.split('.');
  return parts.length > 1 ? `${parts[0]}: ${parts.slice(1).join(' ')}` : id;
}
