<script lang="ts">
  import { fly } from 'svelte/transition';
  import { ExternalLink, RefreshCw, MessageCircle } from '@lucide/svelte';
  import { updates, checkForUpdate } from '$lib/updates.svelte';
  import { menu, loadMenuStatus, addToMenu, removeFromMenu } from '$lib/menuEntry.svelte';
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import { app, ACCENTS, accentName, applyAccent, toast, errorText } from '$lib/state.svelte';
  import Logo from '$lib/components/Logo.svelte';
  import Row from '$lib/components/Row.svelte';
  import Switch from '$lib/components/Switch.svelte';
  import JavaPicker from '$lib/components/JavaPicker.svelte';
  import JvmPresetPicker from '$lib/components/JvmPresetPicker.svelte';
  import type { Settings } from '$lib/types';

  let s = $state<Settings>(JSON.parse(JSON.stringify(app.settings)));
  const dirty = $derived(JSON.stringify(s) !== JSON.stringify(app.settings));
  const maxMem = $derived(Math.max(4096, Math.floor((app.info?.totalMemoryMb ?? 16384) / 512) * 512));
  const linux = $derived(app.info?.os === 'linux');

  onMount(() => {
    if (!menu.status) loadMenuStatus();
  });

  const afterLaunchHelp: Record<string, string> = {
    open: 'Peak stays where it is while you play.',
    minimize: 'Peak minimizes once the game is on screen and comes back when the game closes or crashes.',
    hide: 'Peak hides once the game is on screen; click its tray icon to bring it back. It returns by itself when the game closes or crashes.',
    close: "Peak closes once the game is on screen. The game keeps running, but that session's playtime and console output aren't kept."
  };

  async function checkUpdates() {
    const r = await checkForUpdate(true);
    if (r === 'current') toast(`You're on the latest version (${app.info?.version}).`, 'success');
    else if (r === 'error') toast(`Couldn't check for updates: ${updates.error}`, 'error');
  }

  const sections = ['Appearance', 'Java & memory', 'Game', 'Commands', 'Network', 'Integrations', 'About'] as const;
  let showKey = $state(false);
  let current = $state<(typeof sections)[number]>('Appearance');

  // Leaving the page without saving reverts any accent / background preview.
  $effect(() => () => {
    applyAccent(app.settings?.accent ?? 'glacier');
    app.bgPreview = null;
  });

  function pickAccent(name: string) {
    s.accent = name;
    applyAccent(name);
  }

  function pickBackground(name: Settings['background']) {
    s.background = name;
    app.bgPreview = name;
  }

  async function save() {
    try {
      app.settings = await api.saveSettings($state.snapshot(s));
      app.bgPreview = null;
      toast('Settings saved', 'success');
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  function reset() {
    s = JSON.parse(JSON.stringify(app.settings));
    applyAccent(s.accent);
    app.bgPreview = null;
  }

  // Reset everything to the defaults (keeps your own CurseForge key).
  let confirmReset = $state(false);
  let resetting = $state(false);
  async function resetAll() {
    if (!confirmReset) {
      confirmReset = true;
      setTimeout(() => (confirmReset = false), 4000);
      return;
    }
    confirmReset = false;
    resetting = true;
    try {
      app.settings = await api.resetSettings();
      s = JSON.parse(JSON.stringify(app.settings));
      applyAccent(s.accent);
      app.bgPreview = null;
      toast('Settings are back to their defaults', 'success');
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      resetting = false;
    }
  }

  function jump(name: (typeof sections)[number]) {
    current = name;
    document.getElementById('sec-' + name)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }

  const gb = (mb: number) => `${(mb / 1024).toFixed(1)} GB`;
</script>

<div class="page">
  <header>
    <h1>Settings</h1>
    <p>Defaults for every instance. Instances can override most of these in their own Settings tab.</p>
  </header>

  <div class="layout">
    <nav class="toc">
      {#each sections as sec (sec)}
        <button class:active={current === sec} onclick={() => jump(sec)}>{sec}</button>
      {/each}
    </nav>

    <div class="content">
      <section id="sec-Appearance" class="panel glass">
        <h3>Appearance</h3>
        <div class="rows">
          <Row title="Background" desc="The colourful aurora glow, a quiet topographic map, or both">
            <div class="seg">
              <button class:active={s.background === 'aurora'} onclick={() => pickBackground('aurora')}>Aurora</button>
              <button class:active={s.background === 'both'} onclick={() => pickBackground('both')}>Aurora + contours</button>
              <button class:active={s.background === 'contours'} onclick={() => pickBackground('contours')}>Contours</button>
            </div>
          </Row>
          <Row title="Accent color" desc="Used for buttons, highlights and markers">
            <div class="accents">
              {#each Object.entries(ACCENTS) as [name, [a]] (name)}
                <button class="swatch" class:active={accentName(s.accent) === name} onclick={() => pickAccent(name)} title={name}>
                  <span class="dot" style:background={a}></span>
                  <span class="name">{name}</span>
                </button>
              {/each}
            </div>
          </Row>
        </div>
      </section>

      <section id="sec-Java & memory" class="panel glass">
        <h3>Java & memory</h3>
        <div class="rows">
          <Row title="Maximum memory" desc="How much RAM Minecraft may use. 4–6 GB suits most modpacks; vanilla is happy with 2–4 GB.">
            <input type="range" min="1024" max={maxMem} step="512" bind:value={s.memoryMb} />
            <span class="val">{gb(s.memoryMb)}</span>
          </Row>
          <Row title="Metaspace" desc="Initial metaspace size in MB (0 to leave it to Java)">
            <input class="input num" type="number" min="0" step="64" bind:value={s.metaspaceMb} />
          </Row>
          <Row title="Use Minecraft's Java" desc="Download and use the exact Java runtime Mojang ships for each version (recommended)">
            <Switch bind:checked={s.useBundledJava} label="Use Minecraft's Java" />
          </Row>
          {#if !s.useBundledJava}
            <Row title="Java executable" desc="Used for every instance unless it picks its own" wide>
              <JavaPicker bind:value={s.javaPath} />
            </Row>
          {/if}
          <Row title="Performance flags" desc="Java tuning presets. None is fine for most players; the others can smooth out big modpacks." wide>
            <JvmPresetPicker bind:value={s.jvmPreset} />
          </Row>
          <Row title="Extra Java arguments" desc="Your own flags, added after the preset" wide>
            <textarea class="input mono" rows="2" bind:value={s.jvmArgs} placeholder="e.g. -Dfml.readTimeout=180"></textarea>
          </Row>
        </div>
      </section>

      <section id="sec-Game" class="panel glass">
        <h3>Game</h3>
        <div class="rows">
          <Row title="Window size" desc="Starting resolution of the game window">
            <input class="input num" type="number" min="640" bind:value={s.windowWidth} />
            <span class="x">×</span>
            <input class="input num" type="number" min="480" bind:value={s.windowHeight} />
          </Row>
          <Row title="Start fullscreen">
            <Switch bind:checked={s.fullscreen} label="Start fullscreen" />
          </Row>
          <Row title="When the game window opens" desc={afterLaunchHelp[s.afterLaunch] ?? afterLaunchHelp.open}>
            <div class="seg">
              <button class:active={s.afterLaunch === 'open'} onclick={() => (s.afterLaunch = 'open')}>Stay open</button>
              <button class:active={s.afterLaunch === 'minimize'} onclick={() => (s.afterLaunch = 'minimize')}>Minimize</button>
              <button class:active={s.afterLaunch === 'hide'} onclick={() => (s.afterLaunch = 'hide')}>Hide to tray</button>
              <button class:active={s.afterLaunch === 'close'} onclick={() => (s.afterLaunch = 'close')}>Close Peak</button>
            </div>
          </Row>
          <Row title="Open console on launch" desc="Switch to the instance's console when the game starts">
            <Switch bind:checked={s.openConsole} label="Open console on launch" />
          </Row>
          {#if linux}
            <Row title="Feral GameMode" desc="Run the game through gamemoderun for better performance">
              <Switch bind:checked={s.gamemode} label="GameMode" />
            </Row>
            <Row title="Use system GLFW" desc="Use your distro's GLFW instead of Minecraft's (helps on Wayland and ARM)">
              <Switch bind:checked={s.useSystemGlfw} label="System GLFW" />
            </Row>
            <Row title="Use system OpenAL">
              <Switch bind:checked={s.useSystemOpenal} label="System OpenAL" />
            </Row>
          {/if}
        </div>
      </section>

      <section id="sec-Commands" class="panel glass">
        <h3>Commands</h3>
        <p class="sub">
          Variables: <code>$INST_NAME</code> <code>$INST_ID</code> <code>$INST_DIR</code> <code>$INST_MC_DIR</code> <code>$INST_JAVA</code>
        </p>
        <div class="rows">
          <Row title="Enable commands">
            <Switch bind:checked={s.enableCommands} label="Enable commands" />
          </Row>
          <Row title="Pre-launch command" desc="Runs before the game starts; a failure cancels the launch">
            <input class="input mono" bind:value={s.preLaunchCommand} disabled={!s.enableCommands} />
          </Row>
          <Row title="Post-exit command" desc="Runs after the game closes">
            <input class="input mono" bind:value={s.postExitCommand} disabled={!s.enableCommands} />
          </Row>
          <Row title="Wrapper command" desc="Prefixed to the Java command (e.g. prime-run, mangohud)">
            <input class="input mono" bind:value={s.wrapperCommand} disabled={!s.enableCommands} />
          </Row>
        </div>
      </section>

      <section id="sec-Network" class="panel glass">
        <h3>Network</h3>
        <div class="rows">
          {#if menu.status?.isAppImage}
            <Row title="Applications menu" desc={menu.status.installed && menu.status.upToDate ? 'Peak is in your applications menu.' : menu.status.installed ? 'Your menu entry points at an old location of this AppImage.' : 'Add Peak to your applications menu so you can open it like any other app.'}>
              {#if menu.status.installed && menu.status.upToDate}
                <button class="btn" onclick={() => removeFromMenu().catch((e) => toast(errorText(e), 'error'))} disabled={menu.busy}>Remove from menu</button>
              {:else}
                <button class="btn primary" onclick={() => addToMenu().catch((e) => toast(errorText(e), 'error'))} disabled={menu.busy}>{menu.status.installed ? 'Update entry' : 'Add to menu'}</button>
              {/if}
            </Row>
          {/if}
          <Row title="Check for updates automatically" desc="Peak asks GitHub for a newer version when it starts and every few hours. Updates are only installed when you choose to.">
            <Switch bind:checked={s.autoUpdateCheck} label="Check for updates automatically" />
          </Row>
          <Row title="Tell me about modpack updates" desc="Check your modpacks for new versions, mark them Latest or Outdated, and ask when you press Play on one that has an update.">
            <Switch bind:checked={s.modpackUpdateCheck} label="Tell me about modpack updates" />
          </Row>
          <Row title="Parallel downloads" desc="Files downloaded at once. Lower this on slow connections.">
            <input type="range" min="1" max="64" bind:value={s.concurrentDownloads} />
            <span class="val">{s.concurrentDownloads}</span>
          </Row>
          <Row title="Connection timeout" desc="Seconds to wait before giving up on a server">
            <input class="input num" type="number" min="5" max="600" bind:value={s.connectionTimeout} />
          </Row>
          <Row title="Use a proxy">
            <Switch bind:checked={s.proxyEnabled} label="Use a proxy" />
          </Row>
          {#if s.proxyEnabled}
            <Row title="Proxy">
              <select class="input type" bind:value={s.proxyType}>
                <option value="http">HTTP</option>
                <option value="socks5">SOCKS5</option>
              </select>
              <input class="input" bind:value={s.proxyHost} placeholder="host" />
              <input class="input port" type="number" min="1" max="65535" bind:value={s.proxyPort} />
            </Row>
          {/if}
        </div>
      </section>

      <section id="sec-Integrations" class="panel glass">
        <h3>Integrations</h3>
        <div class="rows">
          <Row title="Show what I'm doing in Discord" desc="Your Discord profile shows Playing Peak Launcher, with what you're doing: the modpack you're playing, the server you're on, or that you're browsing. It's only shared with the Discord app on this computer.">
            <Switch bind:checked={s.discordPresence} label="Show what I'm doing in Discord" />
          </Row>
          <Row
            title="CurseForge API key"
            desc={app.info?.curseforgeBuiltIn
              ? 'This build already includes a key. Add your own only if you want to use it instead.'
              : 'Needed to browse and install from CurseForge. Get a free key from the CurseForge console. It stays on this computer.'}
            wide
          >
            <div class="key">
              <input
                class="input mono"
                type={showKey ? 'text' : 'password'}
                bind:value={s.curseforgeApiKey}
                placeholder={app.info?.curseforgeBuiltIn ? 'Using the built-in key' : 'Paste your key'}
                autocomplete="off"
                spellcheck="false"
              />
              <button class="btn" onclick={() => (showKey = !showKey)}>{showKey ? 'Hide' : 'Show'}</button>
              <button class="btn" onclick={() => api.openUrl('https://console.curseforge.com/')}><ExternalLink size={14} /> Get a key</button>
            </div>
          </Row>
        </div>
      </section>

      <section class="panel glass">
        <h3>Reset</h3>
        <div class="rows">
          <Row
            title="Reset all settings"
            desc="Put every setting on this page back to its default. Your instances, accounts and worlds aren't touched, your own CurseForge key is kept, and settings you set on a single instance stay."
          >
            <button class="btn danger" class:confirm-reset={confirmReset} onclick={resetAll} disabled={resetting}>
              {confirmReset ? 'Click again to reset' : 'Reset to defaults'}
            </button>
          </Row>
        </div>
      </section>

      <section id="sec-About" class="panel glass about">
        <Logo size={34} />
        <div>
          <b>Peak Launcher</b>
          <span>Version {app.info?.version}{app.info?.devBuild ? ' · development build' : ''} · GPL-3.0</span>
          {#if app.info?.dataDir}<span class="mono path">{app.info.dataDir}</span>{/if}
        </div>
        <button class="btn discord" title="Help, news and support" onclick={() => api.openUrl('https://discord.gg/DR2XVfVCku')}>
          <MessageCircle size={14} /> Discord
        </button>
        <button class="btn check" disabled={updates.checking} onclick={checkUpdates}>
          <RefreshCw size={14} class={updates.checking ? 'spin' : ''} />
          {updates.checking ? 'Checking…' : 'Check for updates'}
        </button>
      </section>
    </div>
  </div>
</div>

{#if dirty}
  <div class="savebar glass" transition:fly={{ y: 20, duration: 220 }}>
    <span>You have unsaved changes</span>
    <button class="btn ghost" onclick={reset}>Discard</button>
    <button class="btn primary" onclick={save}>Save changes</button>
  </div>
{/if}

<style>
  .confirm-reset {
    color: #fecaca;
    background: rgba(239, 68, 68, 0.2);
    border-color: rgba(239, 68, 68, 0.4);
  }
  .page {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding-bottom: 60px;
  }
  h1 {
    font-size: 28px;
    font-weight: 720;
  }
  header p {
    margin: 6px 0 0;
    color: var(--muted);
    font-size: 13.5px;
  }
  .layout {
    display: grid;
    grid-template-columns: 170px 1fr;
    gap: 22px;
    align-items: start;
  }
  .toc {
    position: sticky;
    top: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .toc button {
    text-align: left;
    height: 34px;
    padding: 0 12px;
    border-radius: 10px;
    font-size: 13px;
    font-weight: 540;
    color: var(--muted);
  }
  .toc button:hover {
    color: var(--text);
    background: rgba(255, 255, 255, 0.04);
  }
  .toc button.active {
    color: var(--text);
    background: rgba(255, 255, 255, 0.07);
  }
  .content {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  section {
    scroll-margin-top: 12px;
  }
  .accents {
    display: flex;
    gap: 10px;
  }
  .accents {
    flex-wrap: wrap;
    justify-content: flex-end;
  }
  .swatch {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 30px;
    padding: 0 10px 0 8px;
    border-radius: 6px;
    border: 1px solid var(--stroke);
    background: #0e1013;
    font-size: 12px;
    color: var(--muted);
    text-transform: capitalize;
    transition: border-color 0.15s, color 0.15s;
  }
  .swatch:hover {
    border-color: var(--stroke-strong);
    color: var(--text);
  }
  .swatch.active {
    border-color: var(--accent);
    color: var(--text);
  }
  .dot {
    width: 12px;
    height: 12px;
    border-radius: 3px;
  }
  input[type='range'] {
    flex: 1;
    accent-color: var(--accent);
  }
  .val {
    width: 60px;
    text-align: right;
    font-size: 13px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .num {
    width: 110px;
  }
  .type {
    width: 120px;
    flex-shrink: 0;
  }
  .port {
    width: 100px;
    flex-shrink: 0;
  }
  .x {
    color: var(--faint);
  }
  .key {
    display: flex;
    gap: 8px;
  }
  .key .input {
    flex: 1;
  }
  code {
    font-size: 11.5px;
    padding: 1px 5px;
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.07);
    color: var(--muted);
  }
  .discord {
    margin-left: auto;
  }
  .about .discord + .check {
    margin-left: 0;
  }
  .about {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .about div {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .about .check {
    margin-left: auto;
  }
  .about b {
    font-size: 14px;
  }
  .about span {
    font-size: 12.5px;
    color: var(--faint);
  }
  .path {
    font-size: 11.5px !important;
    user-select: text;
  }
  .savebar {
    position: sticky;
    bottom: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: -44px;
    padding: 10px 10px 10px 18px;
    background: rgba(20, 22, 34, 0.85);
    border-color: var(--stroke-strong);
    z-index: 5;
  }
  .savebar span {
    flex: 1;
    font-size: 13px;
    color: var(--muted);
  }
</style>
