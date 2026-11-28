// Lists Peak's UI text and checks the translations in src/lib/locales.
//
//   node scripts/i18n-check.mjs           report missing and unused strings
//   node scripts/i18n-check.mjs --keys    print every string, one JSON array
//   node scripts/i18n-check.mjs --prune   drop unused strings and sort the files
//
// The English text in t('...') and th('...') calls is the key.
import { readFileSync, readdirSync, writeFileSync, statSync } from 'node:fs';
import { join, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('..', import.meta.url));
const src = join(root, 'src');
const locales = join(src, 'lib', 'locales');

function files(dir) {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) return name === 'locales' ? [] : files(p);
    return ['.svelte', '.ts'].includes(extname(p)) ? [p] : [];
  });
}

// t('text') / th("text"), with escaped quotes inside.
const CALL = /(?<![\w.$])th?\(\s*(?:'((?:[^'\\]|\\.)*)'|"((?:[^"\\]|\\.)*)")/g;
const unescape = (s) => s.replace(/\\(.)/g, (_, c) => (c === 'n' ? '\n' : c));

const keys = new Set();
for (const f of files(src)) {
  const text = readFileSync(f, 'utf8');
  for (const m of text.matchAll(CALL)) keys.add(unescape(m[1] ?? m[2]));
  if (f.endsWith('gameOptions.ts')) {
    for (const m of text.matchAll(/label: '((?:[^'\\]|\\.)*)'|kb\('[^']*', '((?:[^'\\]|\\.)*)'/g)) keys.add(unescape(m[1] ?? m[2]));
  }
  if (f.endsWith('i18n.svelte.ts')) {
    const list = text.match(/BACKEND_TEXT = \[([\s\S]*?)\]/);
    for (const m of list?.[1].matchAll(/'((?:[^'\\]|\\.)*)'/g) ?? []) keys.add(unescape(m[1]));
  }
}
const all = [...keys].sort();

if (process.argv.includes('--keys')) {
  console.log(JSON.stringify(all, null, 1));
  process.exit(0);
}

const placeholders = (s) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(',');
let problems = 0;
for (const name of readdirSync(locales).filter((n) => n.endsWith('.json'))) {
  const path = join(locales, name);
  const dict = JSON.parse(readFileSync(path, 'utf8'));
  const missing = all.filter((k) => !(k in dict));
  const unused = Object.keys(dict).filter((k) => !keys.has(k));
  const badVars = all.filter((k) => k in dict && placeholders(k) !== placeholders(dict[k]));
  console.log(`${name}: ${all.length - missing.length}/${all.length} translated, ${unused.length} unused, ${badVars.length} with wrong placeholders`);
  for (const k of badVars) console.log(`  placeholders differ: ${k}`);
  problems += badVars.length;
  if (process.argv.includes('--prune')) {
    const out = Object.fromEntries(all.filter((k) => k in dict).map((k) => [k, dict[k]]));
    writeFileSync(path, JSON.stringify(out, null, 2) + '\n');
  }
}
process.exit(problems ? 1 : 0);
