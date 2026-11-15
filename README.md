# Peak Launcher

A fast, modern Minecraft: Java Edition launcher, built with Tauri (Rust) and Svelte.

- Glass-style dark UI with accent colors
- One-click instances: game files, assets and the right Java version are downloaded automatically
- Parallel, checksum-verified downloads; shared files are stored once for all instances
- Microsoft account sign-in (device code flow)
- Mod loaders: Fabric, Quilt, Forge (1.7.10+) and NeoForge
- Modpacks page: browse Modrinth, CurseForge, Feed The Beast and Technic modpacks (Technic packs from Minecraft 1.6 onward; the loader is detected from the pack and installed by Peak); installing one creates a ready-to-play instance. Or import a `.mrpack` / CurseForge `.zip`
- Per-instance content: each instance's Content tab browses Modrinth and CurseForge for mods, resource packs and shaders that fit it. Required dependencies are listed with the reason ("Required by …") and confirmed before installing
- Import from other launchers: Prism / PolyMC / MultiMC, ATLauncher, the CurseForge app, the official Minecraft Launcher and older Modrinth App profiles. Files are copied; the other launcher is never changed
- Export any instance as a modpack: Modrinth `.mrpack` or CurseForge `.zip`. Content available on the platform is linked (CurseForge matches files by fingerprint when an API key is set); anything else is bundled and listed so you can check you may share it
- CurseForge: mods, resource packs, shaders and modpacks (including importing a CurseForge `.zip`), with update checks. Needs an API key (see below)
- Per-instance overrides: memory, metaspace, Java runtime or custom Java path, JVM arguments, account, quick play (world/server/realm), window size, fullscreen, pre-launch/post-exit/wrapper commands, system GLFW/OpenAL
- Global options: Java detection, after-launch behaviour, parallel downloads, timeouts, HTTP/SOCKS5 proxy, Feral GameMode on Linux

## Development

Prerequisites: Rust (MSVC toolchain on Windows), Node.js 20+, and the Visual Studio C++ build tools.

```bash
npm install
npm run tauri dev
```

`npm run dev` runs only the UI in a browser with mock data, which is handy for design work.

### Microsoft sign-in

Each launcher needs its own Azure app. Register one, request Minecraft API access from Mojang, then build with:

```bash
PEAK_MS_CLIENT_ID=<your-client-id> npm run tauri build
```

Development builds can also add offline accounts for testing.

### CurseForge

CurseForge's API needs a key. Get one free at https://console.curseforge.com, then either:

- paste it into **Settings → Integrations** (stored only on that computer), or
- build it in: `PEAK_CF_API_KEY=<your-key> npm run tauri build`, or better for public releases,
- build against a proxy that adds the key on the server, so the app ships without one:
  `PEAK_CF_PROXY=https://api.peaklauncher.com/v1 npm run tauri build`

A key entered in Settings always takes priority over a built-in key or the proxy.

Files whose authors turned off third-party downloads are never fetched automatically; the instance page lists them with links so they can be downloaded by hand.

### Backend test

Installs a real version into the launcher's data folder and checks the launch command:

```bash
cd src-tauri
cargo test --lib -- --ignored --nocapture install_and_plan
```

Pick a version and loader with `PEAK_TEST_VERSION=1.20.1 PEAK_TEST_LOADER=forge` (optionally `PEAK_TEST_LOADER_VERSION`).

## Data location

`%APPDATA%\PeakLauncher` on Windows, `~/.local/share/PeakLauncher` on Linux, `~/Library/Application Support/PeakLauncher` on macOS.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
