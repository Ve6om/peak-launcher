// A server's message of the day comes as plain text with § codes or as a
// Minecraft text component (JSON). Both become § text here, so the existing
// colour parser can turn them into styled pieces.
import { parseColors, type Segment } from './ansi';

const NAMED: Record<string, string> = {
  black: '0',
  dark_blue: '1',
  dark_green: '2',
  dark_aqua: '3',
  dark_red: '4',
  dark_purple: '5',
  gold: '6',
  gray: '7',
  dark_gray: '8',
  blue: '9',
  green: 'a',
  aqua: 'b',
  red: 'c',
  light_purple: 'd',
  yellow: 'e',
  white: 'f'
};

interface Style {
  color?: string;
  bold?: boolean;
  italic?: boolean;
  underlined?: boolean;
  strikethrough?: boolean;
  obfuscated?: boolean;
}

function codes(s: Style): string {
  let out = '§r';
  if (s.color) {
    if (NAMED[s.color]) out += '§' + NAMED[s.color];
    else if (/^#[0-9a-f]{6}$/i.test(s.color)) out += '§x' + [...s.color.slice(1)].map((c) => '§' + c).join('');
  }
  if (s.bold) out += '§l';
  if (s.italic) out += '§o';
  if (s.underlined) out += '§n';
  if (s.strikethrough) out += '§m';
  if (s.obfuscated) out += '§k';
  return out;
}

/** Flatten a text component (string, array or object) to § text. */
export function toLegacy(value: unknown, parent: Style = {}): string {
  if (value == null) return '';
  if (typeof value === 'string') return value;
  if (typeof value === 'number' || typeof value === 'boolean') return String(value);
  if (Array.isArray(value)) {
    // In arrays the first element sets the style the rest inherit.
    if (!value.length) return '';
    const [first, ...rest] = value;
    const firstStyle = typeof first === 'object' && first && !Array.isArray(first) ? inherit(parent, first as Record<string, unknown>) : parent;
    return toLegacy(first, parent) + rest.map((v) => toLegacy(v, firstStyle)).join('');
  }
  if (typeof value === 'object') {
    const v = value as Record<string, unknown>;
    const style = inherit(parent, v);
    const own = typeof v.text === 'string' ? v.text : typeof v.translate === 'string' ? v.translate : '';
    const extra = Array.isArray(v.extra) ? v.extra.map((e) => toLegacy(e, style)).join('') : '';
    return (own ? codes(style) + own : '') + extra;
  }
  return '';
}

function inherit(parent: Style, v: Record<string, unknown>): Style {
  const pick = (k: keyof Style) => (typeof v[k] === 'boolean' ? (v[k] as boolean) : parent[k]);
  return {
    color: typeof v.color === 'string' ? v.color : parent.color,
    bold: pick('bold') as boolean | undefined,
    italic: pick('italic') as boolean | undefined,
    underlined: pick('underlined') as boolean | undefined,
    strikethrough: pick('strikethrough') as boolean | undefined,
    obfuscated: pick('obfuscated') as boolean | undefined
  };
}

/** The MOTD as up to two lines of styled pieces, edges trimmed. */
export function motdLines(value: unknown): Segment[][] {
  const text = toLegacy(value).replace(/\r/g, '');
  return text
    .split('\n')
    .slice(0, 2)
    .map((line) => {
      const segs = parseColors(line) ?? [{ text: line, style: '' }];
      // Servers pad MOTDs with spaces to centre them in the game; trim for our layout.
      if (segs.length) {
        segs[0] = { ...segs[0], text: segs[0].text.replace(/^\s+/, '') };
        const last = segs.length - 1;
        segs[last] = { ...segs[last], text: segs[last].text.replace(/\s+$/, '') };
      }
      return segs.filter((s) => s.text);
    })
    .filter((l) => l.length);
}

/** Plain text without codes (e.g. for version names that contain them). */
export function plain(value: unknown): string {
  return (parseColors(toLegacy(value)) ?? [{ text: toLegacy(value) }]).map((s) => s.text).join('');
}
