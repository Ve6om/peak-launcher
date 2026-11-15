// Turns colour codes in game output into styled text. Handles ANSI escape
// sequences (used by log4j "highlight" patterns and many mods) and Minecraft's
// own § formatting codes (chat messages and some mod logs).

export interface Segment {
  text: string;
  /** Inline CSS for this piece, empty when it has no formatting. */
  style: string;
}

interface Fmt {
  fg: string | null;
  bg: string | null;
  bold: boolean;
  dim: boolean;
  italic: boolean;
  underline: boolean;
  strike: boolean;
}

const plain = (): Fmt => ({ fg: null, bg: null, bold: false, dim: false, italic: false, underline: false, strike: false });

// Standard ANSI colours, tuned to read well on the console's dark background.
const ANSI = [
  '#5c6370', '#f87171', '#86efac', '#fcd34d', '#7aa2f7', '#e879f9', '#67e8f9', '#d4d8e2',
  '#7f8796', '#fca5a5', '#bbf7d0', '#fde68a', '#a5c0ff', '#f5b4fc', '#a5f3fc', '#ffffff'
];

// Minecraft's 16 colours; black and dark blue are lifted so they stay visible.
const MC: Record<string, string> = {
  '0': '#4b5263', '1': '#4f5bd5', '2': '#00aa00', '3': '#00aaaa',
  '4': '#c03838', '5': '#aa00aa', '6': '#ffaa00', '7': '#aaaaaa',
  '8': '#6b6b6b', '9': '#5555ff', a: '#55ff55', b: '#55ffff',
  c: '#ff5555', d: '#ff55ff', e: '#ffff55', f: '#ffffff'
};

/** Colour n of the 256-colour ANSI palette. */
function xterm(n: number): string {
  if (n < 16) return ANSI[n];
  if (n < 232) {
    const v = [0, 95, 135, 175, 215, 255];
    const i = n - 16;
    return `rgb(${v[Math.floor(i / 36)]},${v[Math.floor(i / 6) % 6]},${v[i % 6]})`;
  }
  const g = 8 + (n - 232) * 10;
  return `rgb(${g},${g},${g})`;
}

const hex = (s: string) => /^[0-9a-f]{6}$/i.test(s);

/** Quick check so lines without codes skip parsing. */
const CODES = /\x1b\[|§/;

function applySgr(f: Fmt, params: number[]) {
  if (!params.length) params = [0];
  for (let i = 0; i < params.length; i++) {
    const p = params[i];
    if (p === 0) Object.assign(f, plain());
    else if (p === 1) f.bold = true;
    else if (p === 2) f.dim = true;
    else if (p === 3) f.italic = true;
    else if (p === 4) f.underline = true;
    else if (p === 9) f.strike = true;
    else if (p === 22) f.bold = f.dim = false;
    else if (p === 23) f.italic = false;
    else if (p === 24) f.underline = false;
    else if (p === 29) f.strike = false;
    else if (p >= 30 && p <= 37) f.fg = ANSI[p - 30];
    else if (p >= 90 && p <= 97) f.fg = ANSI[p - 90 + 8];
    else if (p === 39) f.fg = null;
    else if (p >= 40 && p <= 47) f.bg = ANSI[p - 40];
    else if (p >= 100 && p <= 107) f.bg = ANSI[p - 100 + 8];
    else if (p === 49) f.bg = null;
    else if (p === 38 || p === 48) {
      // 38;5;n / 38;2;r;g;b (and 48 for the background)
      let color: string | null = null;
      if (params[i + 1] === 5 && i + 2 < params.length) {
        color = xterm(params[i + 2] & 255);
        i += 2;
      } else if (params[i + 1] === 2 && i + 4 < params.length) {
        color = `rgb(${params[i + 2]},${params[i + 3]},${params[i + 4]})`;
        i += 4;
      }
      if (color) {
        if (p === 38) f.fg = color;
        else f.bg = color;
      }
    }
  }
}

function css(f: Fmt): string {
  const s: string[] = [];
  if (f.fg) s.push(`color:${f.fg}`);
  if (f.bg) s.push(`background:${f.bg}`);
  if (f.bold) s.push('font-weight:700');
  if (f.dim) s.push('opacity:.65');
  if (f.italic) s.push('font-style:italic');
  const deco = [f.underline && 'underline', f.strike && 'line-through'].filter(Boolean);
  if (deco.length) s.push(`text-decoration:${deco.join(' ')}`);
  return s.join(';');
}

/** Split a line into styled pieces. Returns null when it has no colour codes. */
export function parseColors(line: string): Segment[] | null {
  if (!CODES.test(line)) return null;
  const out: Segment[] = [];
  const f = plain();
  let text = '';
  const flush = () => {
    if (!text) return;
    const style = css(f);
    const last = out[out.length - 1];
    if (last && last.style === style) last.text += text;
    else out.push({ text, style });
    text = '';
  };

  for (let i = 0; i < line.length; i++) {
    const c = line[i];
    if (c === '\x1b' && line[i + 1] === '[') {
      // CSI sequence: ESC [ params final-byte
      let j = i + 2;
      while (j < line.length && !/[@-~]/.test(line[j])) j++;
      if (j >= line.length) break;
      if (line[j] === 'm') {
        flush();
        const params = line
          .slice(i + 2, j)
          .split(/[;:]/)
          .filter((p) => p !== '')
          .map(Number)
          .filter((n) => Number.isFinite(n));
        applySgr(f, params);
      }
      // Other sequences (cursor moves, clears) are dropped.
      i = j;
    } else if (c === '§' && i + 1 < line.length) {
      const code = line[i + 1].toLowerCase();
      // §x§R§R§G§G§B§B: hex colour (Bukkit/Spigot style)
      if (code === 'x') {
        const digits = line.slice(i + 2, i + 14).replace(/§/g, '');
        if (digits.length === 6 && hex(digits) && /^(§[0-9a-f]){6}$/i.test(line.slice(i + 2, i + 14))) {
          flush();
          Object.assign(f, plain(), { fg: `#${digits}` });
          i += 13;
          continue;
        }
      }
      if (MC[code]) {
        flush();
        // In Minecraft a colour code also clears bold/italic/etc.
        Object.assign(f, plain(), { fg: MC[code] });
      } else if ('lmnor'.includes(code)) {
        flush();
        if (code === 'l') f.bold = true;
        else if (code === 'm') f.strike = true;
        else if (code === 'n') f.underline = true;
        else if (code === 'o') f.italic = true;
        else Object.assign(f, plain());
      } else if (code !== 'k') {
        // Not a formatting code; keep the § as text.
        text += c;
        continue;
      }
      i++; // skip the code character (§k "obfuscated" is just hidden)
    } else if (c === '\x1b') {
      // Stray escape character
    } else {
      text += c;
    }
  }
  flush();
  return out;
}

/** The line's text without any colour codes (for search, copy and warning detection). */
export function stripColors(line: string): string {
  if (!CODES.test(line)) return line;
  return (parseColors(line) ?? []).map((s) => s.text).join('');
}
