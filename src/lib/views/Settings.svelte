<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  import { fly } from 'svelte/transition';
  import Select from '$lib/components/Select.svelte';
  import { ExternalLink, RefreshCw, MessageCircle } from '@lucide/svelte';
  import { updates, checkForUpdate } from '$lib/updates.svelte';
  import { menu, loadMenuStatus, addToMenu, removeFromMenu } from '$lib/menuEntry.svelte';
  import { onMount, untrack } from 'svelte';
  import { api } from '$lib/api';
  import { app, ACCENTS, accentName, applyAccent, isHexColor, hexToRgb, rgbToHex, toast, errorText } from '$lib/state.svelte';
  import { shownPath, streaming, setStreamerMode } from '$lib/privacy.svelte';
  import { LANGUAGES, loadLanguage } from '$lib/i18n.svelte';
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
  import NetworkCheck from '$lib/components/NetworkCheck.svelte';
  import type { Settings } from '$lib/types';

  let s = $state<Settings>(JSON.parse(JSON.stringify(app.settings)));

  // Streamer mode and the language apply straight away (from here, the title
  // bar or the shortcut), so the form follows them instead of showing them as
  // unsaved changes.
  $effect(() => {
    const on = app.settings?.streamerMode ?? false;
    const lang = app.settings?.language ?? '';
    untrack(() => {
      s.streamerMode = on;
      s.language = lang;
    });
  });

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
      toast((n === 1 ? t('Updated {n} instance', { n }) : t('Updated {n} instances', { n })), 'success');
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
    open: t('Peak stays where it is while you play.'),
    minimize: t('Peak minimizes once the game is on screen and comes back when the game closes or crashes.'),
    hide: t('Peak hides once the game is on screen; click its tray icon to bring it back. It returns by itself when the game closes or crashes.'),
    close: t('Peak closes once the game is on screen. The game keeps running, but that session\'s playtime and console output aren\'t kept.')
  };

  async function checkUpdates() {
    const r = await checkForUpdate(true);
    if (r === 'current') toast(t('You\'re on the latest version ({version}).', { version: app.info?.version }), 'success');
    else if (r === 'error') toast(t('Couldn\'t check for updates: {error}', { error: updates.error }), 'error');
  }

  // Each one is its own tab; the last one opened is remembered.
  const sections = ['General', 'Appearance', t('Java & memory'), 'Game', 'Defaults', 'Storage', 'Commands', 'Network', 'Integrations', 'About'] as const;
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
    return t('General');
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
      toast(on ? t('Peak will start when your computer does') : t('Peak won\'t start with your computer'), 'success');
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
      toast(t('Settings saved'), 'success');
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
      toast(t('Settings are back to their defaults'), 'success');
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      resetting = false;
    }
  }

  // The language applies straight away, like streamer mode.
  async function setLanguage(code: string) {
    if (!app.settings || (app.settings.language ?? '') === code) return;
    try {
      app.settings = await api.saveSettings({ ...$state.snapshot(app.settings), language: code });
      await loadLanguage(code);
    } catch (e) {
      toast(errorText(e), 'error');
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

  const gb = (mb: number) => t('{x} GB', { x: (mb / 1024).toFixed(1) });
</script>

<div class="page">
  <header>
    <h1>{t('Settings')}</h1>
    <p>{t('Defaults for every instance. Instances can override most of these in their own Settings tab.')}</p>
  </header>

  <div class="layout">
    <nav class="toc">
      {#each sections as sec (sec)}
        <button class:active={current === sec} onclick={() => jump(sec)}>{t(sec)}</button>
      {/each}
    </nav>

    <div class="content">
      <section id="sec-General" class="panel glass" hidden={current !== 'General'}>
        <h3>{t('General')}</h3>
        <div class="rows">
          <Row title={t('Start Peak when my computer starts')} desc={t('Peak opens minimized when you sign in to your computer, ready when you are. This takes effect straight away.')}>
            <Switch
              checked={!!autostart}
              disabled={autostart === null || autostartBusy}
              onchange={(on: boolean) => setAutostart(on)}
              label={t('Start Peak when my computer starts')}
            />
          </Row>
          <Row title={t('Language')} desc={t('The language Peak shows its own text in. Mod and modpack descriptions stay as their authors wrote them.')}>
            <Select
              value={app.settings?.language ?? ''}
              options={[{ value: '', label: t('Same as this computer') }, ...LANGUAGES.map((l) => ({ value: l.code, label: l.name }))]}
              onchange={(v: string | null) => setLanguage(v ?? '')}
              style="min-width: 220px;"
            />
          </Row>
          <Row
            title={t('Streamer mode')}
            desc={t('Hides account names, your UUID, server addresses and the folder paths that contain your computer\'s user name, here and in the game console. Turn it on or off from anywhere with Ctrl+Shift+S.')}
          >
            <Switch checked={streaming()} onchange={(on: boolean) => setStreamerMode(on)} label={t('Streamer mode')} />
          </Row>
          <Row title={t('When the game window opens')} desc={afterLaunchHelp[s.afterLaunch] ?? afterLaunchHelp.open}>
            <div class="seg">
              <button class:active={s.afterLaunch === 'open'} onclick={() => (s.afterLaunch = 'open')}>{t('Stay open')}</button>
              <button class:active={s.afterLaunch === 'minimize'} onclick={() => (s.afterLaunch = 'minimize')}>{t('Minimize')}</button>
              <button class:active={s.afterLaunch === 'hide'} onclick={() => (s.afterLaunch = 'hide')}>{t('Hide to tray')}</button>
              <button class:active={s.afterLaunch === 'close'} onclick={() => (s.afterLaunch = 'close')}>{t('Close Peak')}</button>
            </div>
          </Row>
        </div>
      </section>

      <section id="sec-Appearance" class="panel glass" hidden={current !== 'Appearance'}>
        <h3>{t('Appearance')}</h3>
        <div class="rows">
          <Row title={t('Background')} desc={t('The colourful aurora glow, a quiet topographic map, or both')}>
            <div class="seg">
              <button class:active={s.background === 'aurora'} onclick={() => pickBackground('aurora')}>{t('Aurora')}</button>
              <button class:active={s.background === 'both'} onclick={() => pickBackground('both')}>{t('Aurora + contours')}</button>
              <button class:active={s.background === 'contours'} onclick={() => pickBackground('contours')}>{t('Contours')}</button>
            </div>
          </Row>
          {#if s.background !== 'contours'}
            <Row
              title={t('Moving glow')}
              desc={linux ? t('The aurora slowly drifts. Off by default on Linux, where it can make Peak sluggish.') : t('The aurora slowly drifts while Peak is open in front of you.')}
            >
              <Switch bind:checked={s.movingGlow} disabled={s.lightEffects} onchange={(on: boolean) => (app.glowPreview = on)} label={t('Moving glow')} />
            </Row>
          {/if}
          <Row title={t('Lighter effects')} desc={t('Skip the blur and the moving background glow. Can be smoother on older PCs, and on Linux.')}>
            <Switch bind:checked={s.lightEffects} onchange={(on: boolean) => (app.lightPreview = on)} label={t('Lighter effects')} />
          </Row>
          <Row title={t('Accent color')} desc={t('Used for buttons, highlights and markers')} wide>
            <div class="accents">
              {#each Object.entries(ACCENTS) as [name, [a]] (name)}
                <button class="swatch" class:active={accentNow === name} onclick={() => pickAccent(name)} title={name}>
                  <span class="dot" style:background={a}></span>
                  <span class="name">{name}</span>
                </button>
              {/each}
            </div>
            <div class="accents special">
              <button class="swatch" class:active={isHexColor(accentNow)} onclick={() => pickAccent(custom)} title={t('Pick your own colour')}>
                <span class="dot" style:background={custom}></span>
                <span class="name">{t('Custom')}</span>
              </button>
              <button class="swatch" class:active={accentNow === 'rainbow'} onclick={() => pickAccent('rainbow')} title={t('Cycles through every colour')}>
                <span class="dot rainbow-dot"></span>
                <span class="name">{t('Rainbow')}</span>
              </button>
            </div>
            {#if isHexColor(accentNow)}
              <div class="custom-color">
                <input type="color" value={custom} oninput={(e) => setCustom(e.currentTarget.value)} aria-label={t('Pick a colour')} />
                {#each ['R', 'G', 'B'] as ch, i (ch)}
                  <label class="chan">
                    <span>{ch}</span>
                    <input class="input num" type="number" min="0" max="255" value={rgb[i]} onchange={(e) => setChannel(i, e.currentTarget.valueAsNumber)} />
                  </label>
                {/each}
                <label class="chan hex">
                  <span>{t('Hex')}</span>
                  <input class="input mono" maxlength="7" bind:value={hexText} onchange={() => typedHex(hexText)} spellcheck="false" />
                </label>
              </div>
            {:else if accentNow === 'rainbow'}
              <p class="note">{linux ? t('Cycles through every colour, and pauses while Peak is minimized. It keeps Peak redrawing, so it uses a little more CPU than a fixed colour, and noticeably more on Linux.') : t('Cycles through every colour, and pauses while Peak is minimized. It keeps Peak redrawing, so it uses a little more CPU than a fixed colour.')}</p>
            {/if}
          </Row>
        </div>
      </section>

      <section id="sec-Java & memory" class="panel glass" hidden={current !== 'Java & memory'}>
        <h3>{t('Java & memory')}</h3>
        <div class="rows">
          <Row title={t('Maximum memory')} desc={t('How much RAM Minecraft may use. 4–6 GB suits most modpacks; vanilla is happy with 2–4 GB.')}>
            <input type="range" min="1024" max={maxMem} step="512" bind:value={s.memoryMb} />
            <span class="val">{gb(s.memoryMb)}</span>
          </Row>
          <Row title={t('Metaspace')} desc={t('Initial metaspace size in MB (0 to leave it to Java)')}>
            <input class="input num" type="number" min="0" step="64" bind:value={s.metaspaceMb} />
          </Row>
          <Row title={t('Use Minecraft\'s Java')} desc={t('Let Peak download the Java each game version needs (recommended)')}>
            <Switch bind:checked={s.useBundledJava} label={t('Use Minecraft\'s Java')} />
          </Row>
          {#if s.useBundledJava}
            <Row title={t('Java build')} desc={t('Peak downloads the Java version each game needs, from the build you pick here. Instances can pick their own.')} wide>
              <JavaVendorPicker bind:value={s.javaVendor} />
            </Row>
          {:else}
            <Row title={t('Java executable')} desc={t('Used for every instance unless it picks its own')} wide>
              <JavaPicker bind:value={s.javaPath} />
            </Row>
          {/if}
          <Row title={t('Performance flags')} desc={t('Java tuning presets. None is fine for most players; the others can smooth out big modpacks.')} wide>
            <JvmPresetPicker bind:value={s.jvmPreset} />
          </Row>
          <Row title={t('Extra Java arguments')} desc={t('Your own flags, added after the preset')} wide>
            <textarea class="input mono" rows="2" bind:value={s.jvmArgs} placeholder={t('e.g. -Dfml.readTimeout=180')}></textarea>
          </Row>
          <Row title={t('Downloaded Java')} desc={t('Every Java Peak has downloaded for your games, and the space each one takes')} wide>
            <JavaDownloads />
          </Row>
        </div>
      </section>

      <section id="sec-Game" class="panel glass" hidden={current !== 'Game'}>
        <h3>{t('Game')}</h3>
        <div class="rows">
          <Row title={t('Window size')} desc={t('Starting resolution of the game window')}>
            <input class="input num" type="number" min="640" bind:value={s.windowWidth} />
            <span class="x">×</span>
            <input class="input num" type="number" min="480" bind:value={s.windowHeight} />
          </Row>
          <Row title={t('Start fullscreen')}>
            <Switch bind:checked={s.fullscreen} label={t('Start fullscreen')} />
          </Row>
          <Row title={t('Open console on launch')} desc={t('Switch to the instance\'s console when the game starts')}>
            <Switch bind:checked={s.openConsole} label={t('Open console on launch')} />
          </Row>
          {#if linux}
            <Row title={t('Feral GameMode')} desc={t('Run the game through gamemoderun for better performance')}>
              <Switch bind:checked={s.gamemode} label={t('GameMode')} />
            </Row>
            <Row title={t('MangoHud')} desc={t('Show FPS, frame times and GPU use over the game (needs MangoHud installed)')}>
              <Switch bind:checked={s.mangohud} label={t('MangoHud')} />
            </Row>
            <Row title={t('Use the dedicated graphics card')} desc={t('On laptops with two graphics chips, run the game on the faster one (NVIDIA PRIME or DRI_PRIME)')}>
              <Switch bind:checked={s.discreteGpu} label={t('Dedicated graphics card')} />
            </Row>
            <Row title={t('Use system GLFW')} desc={t('Use your distro\'s GLFW instead of Minecraft\'s (helps on Wayland and ARM)')}>
              <Switch bind:checked={s.useSystemGlfw} label={t('System GLFW')} />
            </Row>
            <Row title={t('Use system OpenAL')}>
              <Switch bind:checked={s.useSystemOpenal} label={t('System OpenAL')} />
            </Row>
          {/if}
          <Row title={t('Environment variables')} desc={t('For every game, one KEY=value per line. Instances can add their own.')} wide>
            <textarea class="input mono" rows="3" spellcheck="false" placeholder={t('MESA_GL_VERSION_OVERRIDE=4.5')} bind:value={s.envVars}></textarea>
          </Row>
        </div>
      </section>

      <section id="sec-Defaults" class="panel glass" hidden={current !== 'Defaults'}>
        <h3>{t('Default game settings')}</h3>
        <p class="sub">
          {t('Keybinds and options saved from one instance. Instances with them switched on get them every time they start; each instance has the switches in its own Settings tab.')}
        </p>
        {#if current === 'Defaults'}<GameDefaults />{/if}
        <h4 class="minor">{t('New instances')}</h4>
        <SharedSwitches show="defaults" bind:value={s.sharedDefault} />
        <SharedSwitches show="packs" bind:value={s.sharedDefault} />
        <div class="share-actions">
          <button class="btn" disabled={applyingShare} onclick={shareWithAll}>{t('Use these for all instances now')}</button>
          <button class="btn ghost" onclick={() => api.openSharedFolder()}>{t('Open shared packs folder')}</button>
        </div>
      </section>

      <section id="sec-Storage" class="panel glass" hidden={current !== 'Storage'}>
        <h3>{t('Storage')}</h3>
        <div class="rows">
          <Row title={t('Back up worlds when the game closes')} desc={t('Each world that was played gets a backup, in every instance. Find them in an instance\'s Worlds tab.')}>
            <Switch bind:checked={s.worldBackupsAuto} label={t('Back up worlds when the game closes')} />
          </Row>
          <Row title={t('Automatic backups to keep')} desc={t('Per world; the oldest go first. Backups made by hand are always kept.')}>
            <Select
              small
              disabled={!s.worldBackupsAuto}
              bind:value={() => String(s.worldBackupsKeep), (v) => (s.worldBackupsKeep = +(v ?? 5))}
              ariaLabel={t('Automatic backups to keep')}
              style="width: 90px;"
              options={[1, 2, 3, 5, 10, 20].map((n) => ({ value: String(n), label: String(n) }))}
            />
          </Row>
        </div>
        {#if current === 'Storage'}<StoragePanel />{/if}
      </section>

      <section id="sec-Commands" class="panel glass" hidden={current !== 'Commands'}>
        <h3>{t('Commands')}</h3>
        <p class="sub">
          {t('Variables:')} <code>$INST_NAME</code> <code>$INST_ID</code> <code>$INST_DIR</code> <code>$INST_MC_DIR</code> <code>$INST_JAVA</code>
        </p>
        <div class="rows">
          <Row title={t('Enable commands')}>
            <Switch bind:checked={s.enableCommands} label={t('Enable commands')} />
          </Row>
          <Row title={t('Pre-launch command')} desc={t('Runs before the game starts; a failure cancels the launch')}>
            <input class="input mono" bind:value={s.preLaunchCommand} disabled={!s.enableCommands} />
          </Row>
          <Row title={t('Post-exit command')} desc={t('Runs after the game closes')}>
            <input class="input mono" bind:value={s.postExitCommand} disabled={!s.enableCommands} />
          </Row>
          <Row title={t('Wrapper command')} desc={t('Prefixed to the Java command (e.g. prime-run, mangohud)')}>
            <input class="input mono" bind:value={s.wrapperCommand} disabled={!s.enableCommands} />
          </Row>
        </div>
      </section>

      <section id="sec-Network" class="panel glass" hidden={current !== 'Network'}>
        <h3>{t('Network')}</h3>
        <div class="rows">
          {#if menu.status?.isAppImage}
            <Row title={t('Applications menu')} desc={menu.status.installed && menu.status.upToDate ? t('Peak is in your applications menu.') : menu.status.installed ? t('Your menu entry points at an old location of this AppImage.') : t('Add Peak to your applications menu so you can open it like any other app.')}>
              {#if menu.status.installed && menu.status.upToDate}
                <button class="btn" onclick={() => removeFromMenu().catch((e) => toast(errorText(e), 'error'))} disabled={menu.busy}>{t('Remove from menu')}</button>
              {:else}
                <button class="btn primary" onclick={() => addToMenu().catch((e) => toast(errorText(e), 'error'))} disabled={menu.busy}>{menu.status.installed ? t('Update entry') : t('Add to menu')}</button>
              {/if}
            </Row>
          {/if}
          <Row title={t('Check for updates automatically')} desc={t('Peak asks GitHub for a newer version when it starts and every few hours. Updates are only installed when you choose to.')}>
            <Switch bind:checked={s.autoUpdateCheck} label={t('Check for updates automatically')} />
          </Row>
          <Row title={t('Tell me about modpack updates')} desc={t('Check your modpacks for new versions, mark them Latest or Outdated, and ask when you press Play on one that has an update.')}>
            <Switch bind:checked={s.modpackUpdateCheck} label={t('Tell me about modpack updates')} />
          </Row>
          <Row title={t('Parallel downloads')} desc={t('Files downloaded at once. Lower this on slow connections.')}>
            <input type="range" min="1" max="64" bind:value={s.concurrentDownloads} />
            <span class="val">{s.concurrentDownloads}</span>
          </Row>
          <Row title={t('Connection timeout')} desc={t('Seconds to wait before giving up on a server')}>
            <input class="input num" type="number" min="5" max="600" bind:value={s.connectionTimeout} />
          </Row>
          <Row title={t('Use a proxy')}>
            <Switch bind:checked={s.proxyEnabled} label={t('Use a proxy')} />
          </Row>
          {#if s.proxyEnabled}
            <Row title={t('Proxy')}>
              <Select
                bind:value={() => s.proxyType, (v) => v && (s.proxyType = v as typeof s.proxyType)}
                ariaLabel={t('Proxy type')}
                style="width: 120px; flex-shrink: 0;"
                options={[
                  { value: 'http', label: 'HTTP' },
                  { value: 'socks5', label: t('SOCKS5') }
                ]}
              />
              <input class="input" bind:value={s.proxyHost} placeholder="host" />
              <input class="input port" type="number" min="1" max="65535" bind:value={s.proxyPort} />
            </Row>
          {/if}
        </div>
        <NetworkCheck />
      </section>

      <section id="sec-Integrations" class="panel glass" hidden={current !== 'Integrations'}>
        <h3>{t('Integrations')}</h3>
        <div class="rows">
          <Row title={t('Show what I\'m doing in Discord')} desc={t('Your Discord profile shows Playing Peak Launcher, with what you\'re doing: the modpack you\'re playing, the server you\'re on, or that you\'re browsing. It\'s only shared with the Discord app on this computer.')}>
            <Switch bind:checked={s.discordPresence} label={t('Show what I\'m doing in Discord')} />
          </Row>
          <Row
            title={t('Join button for any server')}
            desc={t('On servers from Peak\'s server list, your Discord status has a Join on Peak button so friends can join you. Turn this on to show it on other servers too. Their address is shown to anyone who sees your status.')}
          >
            <Switch bind:checked={s.discordJoinAnyServer} disabled={!s.discordPresence} label={t('Join button for any server')} />
          </Row>
          <Row
            title={t('CurseForge API key')}
            desc={app.info?.curseforgeBuiltIn
              ? t('This build already includes a key. Add your own only if you want to use it instead.')
              : t('Needed to browse and install from CurseForge. Get a free key from the CurseForge console. It stays on this computer.')}
            wide
          >
            <div class="key">
              <input
                class="input mono"
                type={showKey ? 'text' : 'password'}
                bind:value={s.curseforgeApiKey}
                placeholder={app.info?.curseforgeBuiltIn ? t('Using the built-in key') : t('Paste your key')}
                autocomplete="off"
                spellcheck="false"
              />
              <button class="btn" onclick={() => (showKey = !showKey)}>{showKey ? t('Hide') : t('Show')}</button>
              <button class="btn" onclick={() => api.openUrl('https://console.curseforge.com/')}><ExternalLink size={14} /> {t('Get a key')}</button>
            </div>
          </Row>
        </div>
      </section>

      <section class="panel glass" hidden={current !== 'About'}>
        <h3>{t('Reset')}</h3>
        <div class="rows">
          <Row
            title={t('Reset all settings')}
            desc={t('Put every setting on this page back to its default. Your instances, accounts and worlds aren\'t touched, your own CurseForge key is kept, and settings you set on a single instance stay.')}
          >
            <button class="btn danger" class:confirm-reset={confirmReset} onclick={resetAll} disabled={resetting}>
              {confirmReset ? t('Click again to reset') : t('Reset to defaults')}
            </button>
          </Row>
        </div>
      </section>

      <section id="sec-About" class="panel glass about" hidden={current !== 'About'}>
        <Logo size={34} />
        <div>
          <b>{t('Peak Launcher')}</b>
          <span>{t('Version {version}{x} · GPL-3.0', { version: app.info?.version, x: app.info?.devBuild ? ` · ${t('development build')}` : '' })}</span>
          {#if app.info?.dataDir}<span class="mono path">{shownPath(app.info.dataDir)}</span>{/if}
        </div>
        <button class="btn discord" title={t('Help, news and support')} onclick={() => api.openUrl('https://discord.gg/DR2XVfVCku')}>
          <MessageCircle size={14} /> {t('Discord')}
        </button>
        <button class="btn check" disabled={updates.checking} onclick={checkUpdates}>
          <RefreshCw size={14} class={updates.checking ? 'spin' : ''} />
          {updates.checking ? t('Checking…') : t('Check for updates')}
        </button>
      </section>
    </div>
  </div>
</div>

{#if dirty}
  <div class="savebar glass" transition:fly={{ y: 20, duration: 220 }}>
    <span>{t('You have unsaved changes')}</span>
    <button class="btn ghost" onclick={reset}>{t('Discard')}</button>
    <button class="btn primary" onclick={save}>{t('Save changes')}</button>
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
