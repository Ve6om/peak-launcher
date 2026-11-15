// Sets the app version everywhere it lives: node scripts/set-version.mjs 0.2.0
import { readFileSync, writeFileSync } from 'node:fs';

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version ?? '')) {
  console.error('Usage: node scripts/set-version.mjs <major.minor.patch>');
  process.exit(1);
}

function edit(path, fn) {
  writeFileSync(path, fn(readFileSync(path, 'utf8')));
  console.log(`${path} -> ${version}`);
}

const json = (text) => text.replace(/("version"\s*:\s*")[^"]+(")/, `$1${version}$2`);
edit('package.json', json);
edit('src-tauri/tauri.conf.json', json);
edit('src-tauri/Cargo.toml', (t) => t.replace(/^version = "[^"]+"/m, `version = "${version}"`));
