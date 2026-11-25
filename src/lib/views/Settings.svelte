<script lang="ts">
  import { fly } from 'svelte/transition';
  import Select from '$lib/components/Select.svelte';
  import { ExternalLink, RefreshCw, MessageCircle } from '@lucide/svelte';
  import { updates, checkForUpdate } from '$lib/updates.svelte';
  import { menu, loadMenuStatus, addToMenu, removeFromMenu } from '$lib/menuEntry.svelte';
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import { app, ACCENTS, accentName, applyAccent, isHexColor, hexToRgb, rgbToHex, toast, errorText } from '$lib/state.svelte';
  import Logo from '$lib/components/Logo.svelte';
  import Row from '$lib/components/Row.svelte';
  import Switch from '$lib/components/Switch.svelte';
  import JavaPicker from '$lib/components/JavaPicker.svelte';
  import JvmPresetPicker from '$lib/components/JvmPresetPicker.svelte';
  import JavaVendorPicker from '$lib/components/JavaVendorPicker.svelte';
  import JavaDownloads from '$lib/components/JavaDownloads.svelte';
  import SharedSwitches from '$lib/components/SharedSwitches.svelte';
  import GameDefaults from '$lib/components/GameDefaults.svelte';
  import StoragePanel from '$lib/components/StoragePanel.svelte';
  import type { Settings } from '$lib/types';

  let s = $state<Settings>(JSON.parse(JSON.stringify(app.settings)));

  let applyingShare = $state(false);
  // Give every instance the sharing chosen above (and save it as the default).
  async function shareWithAll() {
    applyingShare = true;
    try {
      app.settings = await api.saveSettings({ ...$state.snapshot(app.settings!), sharedDefault: $state.snapshot(s.sharedDefault) });
      let n = 0;
      for (const inst of app.instances) {
        const updated = await api.updateInstance({ ...$state.snapshot(inst), shared: $state.snapshot(s.sharedDefault) });
        app.instances = app.instances.map((x) => (x.id === updated.id ? updated : x));
        n++;
      }
      toast(`Updated ${n} instance${n === 1 ? '' : 's'}`, 'success');
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      applyingShare = false;
    }
  }
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

  // Each one is its own tab; the last one opened is remembered.
  const sections = ['General', 'Appearance', 'Java & memory', 'Game', 'Defaults', 'Storage', 'Commands', 'Network', 'Integrations', 'About'] as const;
  type Section = (typeof sections)[number];
  let showKey = $state(false);
  let current = $state<Section>(lastTab());

  function lastTab(): Section {
    try {
      const saved = localStorage.getItem('peak-settings-tab');
      if (saved && (sections as readonly string[]).includes(saved)) return saved as Section;
    } catch {
      // No storage: start on General.
    }
    return 'General';
  }

  // Start Peak with the computer. Kept by the OS (not in settings.json), and
  // switched straight away rather than on Save.
  let autostart = $state<boolean | null>(null);
  let autostartBusy = $state(false);
  onMount(() => {
    api.autostartEnabled().then((on) => (autostart = on)).catch(() => (autostart = null));
  });
  async function setAutostart(on: boolean) {
    autostartBusy = true;
    autostart = on;
    try {
      await api.setAutostart(on);
      toast(on ? 'Peak will start when your computer does' : "Peak won't start with your computer", 'success');
    } catch (e) {
      autostart = !on;
      toast(errorText(e), 'error');
    } finally {
      autostartBusy = false;
    }
  }

  // Leaving the page without saving reverts any accent / background preview.
  $effect(() => () => {
    applyAccent(app.settings?.accent ?? 'glacier');
    app.bgPreview = null;
    app.lightPreview = null;
    app.glowPreview = null;
  });

  function pickAccent(name: string) {
    s.accent = name;
    applyAccent(name);
  }

  // Custom accent: the last colour picked here, kept while trying presets.
  let custom = $state(isHexColor(accentName(app.settings?.accent ?? '')) ? accentName(app.settings!.accent) : '#5cc8f0');
  const accentNow = $derived(accentName(s.accent));
  const rgb = $derived(hexToRgb(custom));
  let hexText = $state('');
  $effect(() => {
    hexText = custom;
  });
  function setCustom(hex: string) {
    if (!isHexColor(hex)) return;
    custom = hex.toLowerCase();
    pickAccent(custom);
  }
  function setChannel(i: number, value: number) {
    const next = [...rgb] as [number, number, number];
    next[i] = Number.isFinite(value) ? Math.max(0, Math.min(255, Math.round(value))) : 0;
    setCustom(rgbToHex(next));
  }
  function typedHex(v: string) {
    const t = v.trim();
    setCustom(t.startsWith('#') ? t : `#${t}`);
  }

  function pickBackground(name: Settings['background']) {
    s.background = name;
    app.bgPreview = name;
  }

  async function save() {
    // Picking a build here counts as answering the first-play question.
    if (s.javaVendor !== app.settings?.javaVendor) s.javaVendorChosen = true;
    try {
      app.settings = await api.saveSettings($state.snapshot(s));
      app.bgPreview = null;
      app.lightPreview = null;
      app.glowPreview = null;
      toast('Settings saved', 'success');
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  function reset() {
    s = JSON.parse(JSON.stringify(app.settings));
    applyAccent(s.accent);
    app.bgPreview = null;
    app.lightPreview = null;
    app.glowPreview = null;
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
      app.lightPreview = null;
      app.glowPreview = null;
      toast('Settings are back to their defaults', 'success');
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      resetting = false;
    }
  }

  function jump(name: Section) {
    current = name;
    try {
      localStorage.setItem('peak-settings-tab', name);
    } catch {
      // Not remembered; fine.
    }
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
      <section id="sec-General" class="panel glass" hidden={current !== 'General'}>
        <h3>General</h3>
        <div class="rows">
          <Row title="Start Peak when my computer starts" desc="Peak opens minimized when you sign in to your computer, ready when you are. This takes effect straight away.">
            <Switch
              checked={!!autostart}
              disabled={autostart === null || autostartBusy}
              onchange={(on: boolean) => setAutostart(on)}
              label="Start Peak when my computer starts"
            />
          </Row>
          <Row title="When the game window opens" desc={afterLaunchHelp[s.afterLaunch] ?? afterLaunchHelp.open}>
            <div class="seg">
              <button class:active={s.afterLaunch === 'open'} onclick={() => (s.afterLaunch = 'open')}>Stay open</button>
              <button class:active={s.afterLaunch === 'minimize'} onclick={() => (s.afterLaunch = 'minimize')}>Minimize</button>
              <button class:active={s.afterLaunch === 'hide'} onclick={() => (s.afterLaunch = 'hide')}>Hide to tray</button>
              <button class:active={s.afterLaunch === 'close'} onclick={() => (s.afterLaunch = 'close')}>Close Peak</button>
            </div>
          </Row>
        </div>
      </section>

      <section id="sec-Appearance" class="panel glass" hidden={current !== 'Appearance'}>
        <h3>Appearance</h3>
        <div class="rows">
          <Row title="Background" desc="The colourful aurora glow, a quiet topographic map, or both">
            <div class="seg">
              <button class:active={s.background === 'aurora'} onclick={() => pickBackground('aurora')}>Aurora</button>
              <button class:active={s.background === 'both'} onclick={() => pickBackground('both')}>Aurora + contours</button>
              <button class:active={s.background === 'contours'} onclick={() => pickBackground('contours')}>Contours</button>
            </div>
          </Row>
          {#if s.background !== 'contours'}
            <Row
              title="Moving glow"
              desc={linux ? 'The aurora slowly drifts. Off by default on Linux, where it can make Peak sluggish.' : 'The aurora slowly drifts while Peak is open in front of you.'}
            >
              <Switch bind:checked={s.movingGlow} disabled={s.lightEffects} onchange={(on: boolean) => (app.glowPreview = on)} label="Moving glow" />
            </Row>
          {/if}
          <Row title="Lighter effects" desc="Skip the blur and the moving background glow. Can be smoother on older PCs, and on Linux.">
            <Switch bind:checked={s.lightEffects} onchange={(on: boolean) => (app.lightPreview = on)} label="Lighter effects" />
          </Row>
          <Row title="Accent color" desc="Used for buttons, highlights and markers" wide>
            <div class="accents">
              {#each Object.entries(ACCENTS) as [name, [a]] (name)}
                <button class="swatch" class:active={accentNow === name} onclick={() => pickAccent(name)} title={name}>
                  <span class="dot" style:background={a}></span>
                  <span class="name">{name}</span>
                </button>
              {/each}
            </div>
            <div class="accents special">
              <button class="swatch" class:active={isHexColor(accentNow)} onclick={() => pickAccent(custom)} title="Pick your own colour">
                <span class="dot" style:background={custom}></span>
                <span class="name">Custom</span>
              </button>
              <button class="swatch" class:active={accentNow === 'rainbow'} onclick={() => pickAccent('rainbow')} title="Cycles through every colour">
                <span class="dot rainbow-dot"></span>
                <span class="name">Rainbow</span>
              </button>
            </div>
            {#if isHexColor(accentNow)}
              <div class="custom-color">
                <input type="color" value={custom} oninput={(e) => setCustom(e.currentTarget.value)} aria-label="Pick a colour" />
                {#each ['R', 'G', 'B'] as ch, i (ch)}
                  <label class="chan">
                    <span>{ch}</span>
                    <input class="input num" type="number" min="0" max="255" value={rgb[i]} onchange={(e) => setChannel(i, e.currentTarget.valueAsNumber)} />
                  </label>
                {/each}
                <label class="chan hex">
                  <span>Hex</span>
                  <input class="input mono" maxlength="7" bind:value={hexText} onchange={() => typedHex(hexText)} spellcheck="false" />
                </label>
              </div>
            {:else if accentNow === 'rainbow'}
              <p class="note">Cycles through every colour, and pauses while Peak is minimized. It keeps Peak redrawing, so it uses a little more CPU than a fixed colour{linux ? ', and noticeably more on Linux' : ''}.</p>
            {/if}
          </Row>
        </div>
      </section>

      <section id="sec-Java & memory" class="panel glass" hidden={current !== 'Java & memory'}>
        <h3>Java & memory</h3>
        <div class="rows">
          <Row title="Maximum memory" desc="How much RAM Minecraft may use. 4–6 GB suits most modpacks; vanilla is happy with 2–4 GB.">
            <input type="range" min="1024" max={maxMem} step="512" bind:value={s.memoryMb} />
            <span class="val">{gb(s.memoryMb)}</span>
          </Row>
          <Row title="Metaspace" desc="Initial metaspace size in MB (0 to leave it to Java)">
            <input class="input num" type="number" min="0" step="64" bind:value={s.metaspaceMb} />
          </Row>
          <Row title="Use Minecraft's Java" desc="Let Peak download the Java each game version needs (recommended)">
            <Switch bind:checked={s.useBundledJava} label="Use Minecraft's Java" />
          </Row>
          {#if s.useBundledJava}
            <Row title="Java build" desc="Peak downloads the Java version each game needs, from the build you pick here. Instances can pick their own." wide>
              <JavaVendorPicker bind:value={s.javaVendor} />
            </Row>
          {:else}
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
          <Row title="Downloaded Java" desc="Every Java Peak has downloaded for your games, and the space each one takes" wide>
            <JavaDownloads />
          </Row>
        </div>
      </section>

      <section id="sec-Game" class="panel glass" hidden={current !== 'Game'}>
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

      <section id="sec-Defaults" class="panel glass" hidden={current !== 'Defaults'}>
        <h3>Default game settings</h3>
        <p class="sub">
          Keybinds and options saved from one instance. Instances with them switched on get them every time they start; each instance has the switches in its own Settings tab.
        </p>
        {#if current === 'Defaults'}<GameDefaults />{/if}
        <h4 class="minor">New instances</h4>
        <SharedSwitches show="defaults" bind:value={s.sharedDefault} />
        <SharedSwitches show="packs" bind:value={s.sharedDefault} />
        <div class="share-actions">
          <button class="btn" disabled={applyingShare} onclick={shareWithAll}>Use these for all instances now</button>
          <button class="btn ghost" onclick={() => api.openSharedFolder()}>Open shared packs folder</button>
        </div>
      </section>

      <section id="sec-Storage" class="panel glass" hidden={current !== 'Storage'}>
        <h3>Storage</h3>
        {#if current === 'Storage'}<StoragePanel />{/if}
      </section>

      <section id="sec-Commands" class="panel glass" hidden={current !== 'Commands'}>
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

      <section id="sec-Network" class="panel glass" hidden={current !== 'Network'}>
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
              <Select
                bind:value={() => s.proxyType, (v) => v && (s.proxyType = v as typeof s.proxyType)}
                ariaLabel="Proxy type"
                style="width: 120px; flex-shrink: 0;"
                options={[
                  { value: 'http', label: 'HTTP' },
                  { value: 'socks5', label: 'SOCKS5' }
                ]}
              />
              <input class="input" bind:value={s.proxyHost} placeholder="host" />
              <input class="input port" type="number" min="1" max="65535" bind:value={s.proxyPort} />
            </Row>
          {/if}
        </div>
      </section>

      <section id="sec-Integrations" class="panel glass" hidden={current !== 'Integrations'}>
        <h3>Integrations</h3>
        <div class="rows">
          <Row title="Show what I'm doing in Discord" desc="Your Discord profile shows Playing Peak Launcher, with what you're doing: the modpack you're playing, the server you're on, or that you're browsing. It's only shared with the Discord app on this computer.">
            <Switch bind:checked={s.discordPresence} label="Show what I'm doing in Discord" />
          </Row>
          <Row
            title="Join button for any server"
            desc="On servers from Peak's server list, your Discord status has a Join on Peak button so friends can join you. Turn this on to show it on other servers too. Their address is shown to anyone who sees your status."
          >
            <Switch bind:checked={s.discordJoinAnyServer} disabled={!s.discordPresence} label="Join button for any server" />
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

      <section class="panel glass" hidden={current !== 'About'}>
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

      <section id="sec-About" class="panel glass about" hidden={current !== 'About'}>
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
  .minor {
    margin: 18px 0 4px;
    font-size: 13px;
    font-weight: 650;
  }
  .share-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 12px;
  }
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
  section[hidden] {
    display: none !important;
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
  .accents.special {
    margin-top: 12px;
    padding-top: 12px;
    border-top: 1px dashed var(--stroke);
  }
  .rainbow-dot {
    background: conic-gradient(#ff6464, #f2c14e, #86d17a, #3fd6b4, #5cc8f0, #a596f7, #f27ac0, #ff6464);
  }
  .custom-color {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    margin-top: 10px;
  }
  .custom-color input[type='color'] {
    width: 38px;
    height: 32px;
    padding: 2px;
    border: 1px solid var(--stroke);
    border-radius: 6px;
    background: #0e1013;
    cursor: pointer;
  }
  .chan {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--muted);
  }
  .chan .input {
    width: 64px;
  }
  .chan.hex .input {
    width: 90px;
  }
  .note {
    margin: 10px 0 0;
    font-size: 12px;
    color: var(--muted);
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
