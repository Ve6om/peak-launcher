// Peak's own text in the player's language. The English is written in the
// code and doubles as the key, so anything not translated yet shows in English.
// Placeholders look like {name} and are filled from the second argument.

const files = import.meta.glob<Record<string, string>>('./locales/*.json', { import: 'default' });

export const LANGUAGES: { code: string; name: string }[] = [
  { code: 'en', name: 'English' },
  { code: 'de', name: 'Deutsch' },
  { code: 'es', name: 'Español' },
  { code: 'fr', name: 'Français' },
  { code: 'pl', name: 'Polski' },
  { code: 'pt-BR', name: 'Português (Brasil)' },
  { code: 'ru', name: 'Русский' },
  { code: 'zh-CN', name: '简体中文' }
];

/** Text that reaches t() from data rather than a literal: install steps from the backend and content kinds. */
export const BACKEND_TEXT = [
  'Preparing',
  'Downloading game files',
  'Downloading Java',
  'Repairing Java',
  'Downloading mod loader',
  'Setting up mod loader',
  'Installing Minecraft',
  'Installing and updating Minecraft',
  'Downloading modpack',
  'Resolving modpack files',
  'Unpacking modpack',
  'Copying modpack files',
  'Copying files',
  'Done',
  'Mods',
  'Resource packs',
  'Shaders',
  'mods',
  'resource packs',
  'shaders',
  'mod',
  'resource pack',
  'shader',
  'General',
  'Appearance',
  'Java & memory',
  'Game',
  'Defaults',
  'Storage',
  'Commands',
  'Network',
  'Integrations',
  'About',
  'Video',
  'Sound',
  'Controls',
  'Keybinds',
  'Chat & other',
  'More',
  'Survival',
  'Creative',
  'Adventure',
  'Spectator'
];

const STORE = 'peak.language';
const lang = $state({ code: 'en', dict: {} as Record<string, string> });

/** The language to use for a setting: a code, or "" for the computer's language. */
export function resolveLanguage(setting: string): string {
  const known = LANGUAGES.map((l) => l.code);
  const wanted = setting ? [setting] : [...(navigator.languages ?? []), navigator.language].filter(Boolean);
  for (const w of wanted) {
    const exact = known.find((c) => c.toLowerCase() === w.toLowerCase());
    if (exact) return exact;
    const base = w.split('-')[0].toLowerCase();
    const near = known.find((c) => c.split('-')[0].toLowerCase() === base);
    if (near) return near;
  }
  return 'en';
}

/** The language shown right now, like "de". */
export const language = () => lang.code;

/** Switch Peak's text to a language setting ("" follows the computer). */
export async function loadLanguage(setting: string) {
  const code = resolveLanguage(setting);
  let dict: Record<string, string> = {};
  if (code !== 'en') {
    try {
      dict = (await files[`./locales/${code}.json`]?.()) ?? {};
    } catch {
      // Falls back to English.
    }
  }
  lang.dict = dict;
  lang.code = code;
  document.documentElement.lang = code;
  try {
    localStorage.setItem(STORE, setting);
  } catch {
    // Only used to show the right language before settings load.
  }
}

/** The language setting from last time, for the first paint. */
export function rememberedLanguage(): string {
  try {
    return localStorage.getItem(STORE) ?? '';
  } catch {
    return '';
  }
}

const escape = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

/**
 * For sentences with bold or code parts, used with {@html}: the text may
 * contain <b>, <code> and <em>, and everything else (including the values
 * filled in) is escaped.
 */
export function th(text: string, vars?: Record<string, string | number | null | undefined>): string {
  const s = escape(lang.dict[text] || text).replace(/&lt;(\/?)(b|code|em)&gt;/g, '<$1$2>');
  if (!vars) return s;
  return s.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? escape(String(vars[k] ?? '')) : m));
}

/** Peak's text in the current language: t('Installed {name}', { name }). */
export function t(text: string, vars?: Record<string, string | number | null | undefined>): string {
  const s = lang.dict[text] || text;
  if (!vars) return s;
  return s.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k] ?? '') : m));
}
