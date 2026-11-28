<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  import { onMount, untrack } from 'svelte';
  import Select from './Select.svelte';
  import { fly } from 'svelte/transition';
  import { TriangleAlert } from '@lucide/svelte';
  import Row from './Row.svelte';
  import Switch from './Switch.svelte';
  import SharedSwitches from './SharedSwitches.svelte';
  import GameDefaults from './GameDefaults.svelte';
  import JavaPicker from './JavaPicker.svelte';
  import JvmPresetPicker from './JvmPresetPicker.svelte';
  import OptimizeDialog from './OptimizeDialog.svelte';
  import { Sparkles } from '@lucide/svelte';
  import PackVersionPanel from './PackVersionPanel.svelte';
  import InstanceArt from './InstanceArt.svelte';
  import { instanceIcon, instanceBanner } from '$lib/instanceImages';
  import { api } from '$lib/api';
  import { app, refreshInstances, toast, errorText } from '$lib/state.svelte';
  import { shownName } from '$lib/privacy.svelte';
  import { LOADERS, loaderLabel, type Instance, type LoaderVersion, type ManifestEntry, type RuntimeOption, type TuningSuggestion, type DetectedJava, NO_SHARING } from '$lib/types';
  import { javaFolderName } from '$lib/javaNames';

  let { inst }: { inst: Instance } = $props();

  const clone = (i: Instance): Instance => JSON.parse(JSON.stringify(i));
  let d = $state<Instance>(untrack(() => clone(inst)));
  let lastId = untrack(() => inst.id);
  $effect(() => {
    if (inst.id !== lastId) {
      lastId = inst.id;
      d = clone(inst);
    }
  });
  const g = $derived(app.settings!);
  // Only the fields this page edits count as "unsaved changes": saving can also
  // change bookkeeping fields (a new loader resets `installed`), and playtime
  // or last-played can change while the page is open.
  const NOT_EDITED = new Set(['id', 'createdAt', 'lastPlayed', 'playtimeSecs', 'installed', 'versionId', 'pack', 'importedFrom', 'customIcon', 'customBanner']);
  function editable(i: Instance): string {
    return JSON.stringify(i, (key, value) => {
      if (NOT_EDITED.has(key)) return undefined;
      // Blank text is saved as "not set", so treat the two as the same.
      if (typeof value === 'string') return value.trim() || null;
      return value;
    });
  }
  const dirty = $derived(editable(d) !== editable(inst));
  const maxMem = $derived(Math.max(4096, Math.floor((app.info?.totalMemoryMb ?? 16384) / 512) * 512));

  // ---- data for pickers ----
  let mcVersions = $state<ManifestEntry[]>([]);
  let runtimes = $state<RuntimeOption[]>([]);
  /** Java already installed on this computer (not Peak's own downloads). */
  let pcJava = $state<DetectedJava[]>([]);
  let worlds = $state<string[]>([]);
  let loaderVersions = $state<LoaderVersion[]>([]);

  onMount(async () => {
    api.versions().then((m) => (mcVersions = m.versions)).catch(() => {});
    api.javaRuntimes().then((r) => (runtimes = r)).catch(() => {});
    api
      .detectJava()
      .then((list) => {
        const slash = (p: string) => p.replace(/\\/g, '/');
        const own = slash(app.info?.dataDir ?? '').replace(/\/$/, '') + '/runtimes/';
        pcJava = list.filter((j) => !slash(j.path).startsWith(own));
        // A path picked from this list shows here again, not under Custom path.
        if (javaMode === 'custom' && d.javaPath && pcJava.some((j) => j.path === d.javaPath)) javaMode = 'runtime';
      })
      .catch(() => {});
    api.worlds(inst.id).then((w) => (worlds = w)).catch(() => {});
  });

  let req = 0;
  $effect(() => {
    const l = d.loader;
    const v = d.gameVersion;
    if (l === 'vanilla') {
      loaderVersions = [];
      return;
    }
    const my = ++req;
    api.loaderVersions(l, v).then((list) => {
      if (my !== req) return;
      loaderVersions = list;
      if (!d.loaderVersion || !list.some((x) => x.id === d.loaderVersion)) {
        d.loaderVersion = (list.find((x) => x.recommended) ?? list[0])?.id ?? null;
      }
    });
  });

  const versionChoices = $derived.by(() => {
    const list = mcVersions.filter((v) => v.type === 'release' || v.id === d.gameVersion);
    return list.length ? list : [{ id: d.gameVersion } as ManifestEntry];
  });

  // ---- Java mode ----
  type JavaMode = 'auto' | 'runtime' | 'custom';
  // In "Automatic", javaRuntime may name a build ("mojang", "temurin",
  // "graalvm") that Peak downloads in whichever Java version the game needs.
  const BUILDS = ['mojang', 'temurin', 'graalvm'];
  const isBuild = (r: string | null) => !!r && BUILDS.includes(r);
  let javaMode = $state<JavaMode>(untrack(() => (inst.javaPath ? 'custom' : inst.javaRuntime && !isBuild(inst.javaRuntime) ? 'runtime' : 'auto')));
  const buildNames: Record<string, string> = { mojang: "Mojang's Java", temurin: 'Eclipse Temurin', graalvm: 'GraalVM' };
  function setJavaMode(m: JavaMode) {
    javaMode = m;
    if (m !== 'custom') d.javaPath = null;
    if (m === 'custom' || (m === 'auto' && !isBuild(d.javaRuntime)) || (m === 'runtime' && isBuild(d.javaRuntime))) d.javaRuntime = null;
    if (m === 'runtime' && !d.javaRuntime) d.javaRuntime = runtimes.at(-1)?.component ?? 'java-runtime-delta';
    if (m === 'custom' && d.javaPath === null) d.javaPath = '';
  }

  const qpModes = [
    { id: 'none', label: t('Off') },
    { id: 'world', label: t('World') },
    { id: 'server', label: t('Server') },
    { id: 'realm', label: t('Realm') }
  ] as const;

  async function save() {
    try {
      const versionChange = inst.loader !== d.loader || inst.gameVersion !== d.gameVersion || inst.loaderVersion !== d.loaderVersion;
      const saved = await api.updateInstance($state.snapshot(d) as Instance);
      await refreshInstances();
      // Carry on editing from exactly what was saved, so the bar goes away.
      d = clone(saved);
      toast(versionChange ? t('Saved. Game files will update on next launch.') : t('Saved'), 'success');
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  const gb = (mb: number) => t('{x} GB', { x: (mb / 1024).toFixed(1) });

  // ---- Optimize ----
  let optimizing = $state(false);
  async function applyTuning(tuning: TuningSuggestion, useGraal: boolean) {
    optimizing = false;
    d.memoryMb = tuning.memoryMb;
    d.jvmPreset = tuning.jvmPreset;
    if (useGraal) {
      javaMode = 'auto';
      d.javaPath = null;
      d.javaRuntime = 'graalvm';
    }
    await save();
  }

  // ---- the instance's own logo and banner ----
  async function setPicture(kind: 'icon' | 'banner', reset = false) {
    const path = reset ? null : await api.pickPicture(kind === 'icon' ? t('Choose a logo') : t('Choose a banner'));
    if (!reset && !path) return;
    try {
      await api.instanceSetPicture(inst.id, kind, path);
      await refreshInstances();
      toast(reset ? t('Back to the default picture') : kind === 'icon' ? t('Logo updated') : t('Banner updated'), 'success');
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }
  const versionChanged = $derived(inst.loader !== d.loader || inst.gameVersion !== d.gameVersion || inst.loaderVersion !== d.loaderVersion);
</script>

<div class="settings">
  <section class="panel glass">
    <h3>{t('General')}</h3>
    <div class="rows">
      <Row title={t('Name')} desc={t('Shown in your library')}>
        <input class="input" bind:value={d.name} maxlength="48" />
      </Row>
      <Row title={t('Notes')} desc={t('Anything you want to remember about this instance')} wide>
        <textarea class="input" bind:value={d.notes} placeholder={t('Seed, server IPs, mod list…')}></textarea>
      </Row>
      <Row title={t('Logo')} desc={t('Square picture shown on cards and in the sidebar')}>
        <div class="pic">
          <span class="pic-icon"><InstanceArt loader={inst.loader} icon={instanceIcon(inst)} /></span>
          <button class="btn" onclick={() => setPicture('icon')}>{t('Choose…')}</button>
          {#if inst.customIcon}<button class="btn ghost" onclick={() => setPicture('icon', true)}>{t('Reset')}</button>{/if}
        </div>
      </Row>
      <Row title={t('Banner')} desc={t('Wide picture shown at the top of the instance and on the home page')}>
        <div class="pic">
          <span class="pic-banner"><InstanceArt loader={inst.loader} image={instanceBanner(inst)} icon={instanceIcon(inst)} /></span>
          <button class="btn" onclick={() => setPicture('banner')}>{t('Choose…')}</button>
          {#if inst.customBanner}<button class="btn ghost" onclick={() => setPicture('banner', true)}>{t('Reset')}</button>{/if}
        </div>
      </Row>
    </div>
  </section>

  {#if inst.pack}
    <PackVersionPanel {inst} onchanged={(i) => (d = clone(i))} />
  {/if}

  <section class="panel glass">
    <h3>{t('Game & mod loader')}</h3>
    <div class="rows">
      <Row title={t('Minecraft version')}>
        <Select bind:value={() => d.gameVersion, (v) => v && (d.gameVersion = v)} ariaLabel={t('Minecraft version')} options={versionChoices.map((v) => ({ value: v.id, label: v.id }))} />
      </Row>
      <Row title={t('Mod loader')}>
        <div class="seg">
          {#each LOADERS as l (l.id)}
            <button class:active={d.loader === l.id} onclick={() => (d.loader = l.id)}>{l.label}</button>
          {/each}
        </div>
      </Row>
      {#if d.loader !== 'vanilla'}
        <Row title="{loaderLabel(d.loader)} version">
          {#if loaderVersions.length}
            <Select
              bind:value={d.loaderVersion}
              ariaLabel={t('Loader version')}
              options={loaderVersions.map((v) => ({ value: v.id, label: v.id, hint: v.recommended ? 'recommended' : v.stable ? undefined : 'beta' }))}
            />
          {:else}
            <span class="hint warn">{t('Not available for Minecraft {gameVersion}', { gameVersion: d.gameVersion })}</span>
          {/if}
        </Row>
      {/if}
      {#if versionChanged}
        <p class="warning"><TriangleAlert size={15} /> {t('Game files are reinstalled on next launch. Back up your worlds and check your mods are compatible.')}</p>
      {/if}
    </div>
  </section>

  <section class="panel glass">
    <div class="head">
      <h3>{t('Java & memory')}</h3>
      <button class="btn small" onclick={() => (optimizing = true)} title={t('Pick memory, flags and Java for this instance')}>
        <Sparkles size={14} /> {t('Optimize')}
      </button>
    </div>
    <div class="rows">
      <Row
        title={t('Maximum memory')}
        desc={t('RAM the game may use')}
        override={d.memoryMb !== null}
        onoverride={(on) => (d.memoryMb = on ? g.memoryMb : null)}
        globalHint={gb(g.memoryMb)}
      >
        <input type="range" min="1024" max={maxMem} step="512" value={d.memoryMb ?? g.memoryMb} oninput={(e) => (d.memoryMb = +e.currentTarget.value)} />
        <span class="val">{gb(d.memoryMb ?? g.memoryMb)}</span>
      </Row>
      <Row
        title={t('Metaspace')}
        desc={t('Initial metaspace size (MB). Larger modpacks can benefit from 512.')}
        override={d.metaspaceMb !== null}
        onoverride={(on) => (d.metaspaceMb = on ? g.metaspaceMb : null)}
        globalHint="{g.metaspaceMb} MB"
      >
        <input class="input num" type="number" min="0" step="64" value={d.metaspaceMb ?? g.metaspaceMb} oninput={(e) => (d.metaspaceMb = +e.currentTarget.value)} />
      </Row>
      <Row title={t('Java')} desc={t('Automatic downloads the Java version this game needs, from the build below.')}>
        <div class="seg">
          <button class:active={javaMode === 'auto'} onclick={() => setJavaMode('auto')}>{t('Automatic')}</button>
          <button class:active={javaMode === 'runtime'} onclick={() => setJavaMode('runtime')}>{t('Pick version')}</button>
          <button class:active={javaMode === 'custom'} onclick={() => setJavaMode('custom')}>{t('Custom path')}</button>
        </div>
      </Row>
      {#if javaMode === 'auto'}
        <Row title={t('Java build')} desc={t('Which build Peak downloads. GraalVM is often a little faster; switch back if the game acts strangely.')}>
          <Select
            bind:value={d.javaRuntime}
            ariaLabel={t('Java build')}
            options={[{ value: null, label: t('Same as Settings ({x})', { x: buildNames[g.javaVendor] ?? t("Mojang's Java") }) }, ...BUILDS.map((b) => ({ value: b, label: buildNames[b] }))]}
          />
        </Row>
      {:else if javaMode === 'runtime'}
        <Row title={t('Java runtime')} desc={t('Mojang\'s Java (downloaded if needed) or any Java already on this computer')}>
          <Select
            ariaLabel={t('Java runtime')}
            value={d.javaPath ? `path:${d.javaPath}` : d.javaRuntime}
            onchange={(v) => {
              if (!v) return;
              if (v.startsWith('path:')) {
                d.javaPath = v.slice(5);
                d.javaRuntime = null;
              } else {
                d.javaRuntime = v;
                d.javaPath = null;
              }
            }}
            options={[
              ...runtimes.map((r) => ({
                value: r.component,
                label: t('Java {major} ({component})', { major: r.major, component: r.component }),
                hint: r.installed ? 'downloaded' : undefined,
                group: t('Mojang\'s Java (Peak downloads it)')
              })),
              ...pcJava.map((j) => ({ value: `path:${j.path}`, label: t('Java {major} · {javaFolderName}', { major: j.major, javaFolderName: javaFolderName(j.path) }), group: t('Installed on this computer') })),
              ...(d.javaPath && !pcJava.some((j) => j.path === d.javaPath)
                ? [{ value: `path:${d.javaPath}`, label: javaFolderName(d.javaPath), group: t('Installed on this computer') }]
                : [])
            ]}
          />
        </Row>
      {:else if javaMode === 'custom'}
        <Row title={t('Java executable')} wide>
          <JavaPicker bind:value={() => d.javaPath ?? '', (v) => (d.javaPath = v)} />
        </Row>
      {/if}
      <Row
        title={t('Performance flags')}
        desc={t('Java tuning preset for this instance')}
        override={d.jvmPreset !== null}
        onoverride={(on) => (d.jvmPreset = on ? g.jvmPreset : null)}
        wide
      >
        <JvmPresetPicker bind:value={() => d.jvmPreset ?? g.jvmPreset, (v) => (d.jvmPreset = v)} disabled={d.jvmPreset === null} />
      </Row>
      <Row
        title={t('Extra Java arguments')}
        desc={t('Your own flags, added after the preset')}
        override={d.jvmArgs !== null}
        onoverride={(on) => (d.jvmArgs = on ? g.jvmArgs : null)}
        wide
      >
        <textarea class="input mono" rows="2" value={d.jvmArgs ?? g.jvmArgs} oninput={(e) => (d.jvmArgs = e.currentTarget.value)}></textarea>
      </Row>
    </div>
  </section>

  <section class="panel glass">
    <h3>{t('Default game settings')}</h3>
    <p class="shared-note">
      {t('Switched on, this instance gets your default keybinds and settings every time it starts. Changes you make while playing aren\'t saved back, so the defaults stay as they are.')}
    </p>
    <GameDefaults instance={inst} />
    <SharedSwitches show="defaults" bind:value={() => d.shared ?? NO_SHARING, (v) => (d.shared = v)} />
  </section>

  <section class="panel glass">
    <h3>{t('Shared packs')}</h3>
    <p class="shared-note">{t('Resource and shader packs kept the same in every instance that shares them.')}</p>
    <SharedSwitches show="packs" bind:value={() => d.shared ?? NO_SHARING, (v) => (d.shared = v)} />
  </section>

  <section class="panel glass">
    <h3>{t('Launch')}</h3>
    <div class="rows">
      <Row title={t('Account')} desc={t('Play this instance with a specific account')}>
        <Select
          bind:value={() => d.accountId ?? '', (v) => (d.accountId = v || null)}
          ariaLabel={t('Account')}
          options={[{ value: '', label: t('Active account') }, ...app.accounts.map((a) => ({ value: a.id, label: shownName(a.username) }))]}
        />
      </Row>
      <Row title={t('Quick play')} desc={t('Jump straight into a world, server or realm when you press Play. Worlds and realms need 1.20+.')}>
        <div class="seg">
          {#each qpModes as m (m.id)}
            <button class:active={(d.quickPlay.mode || 'none') === m.id} onclick={() => (d.quickPlay = { mode: m.id, value: '' })}>{m.label}</button>
          {/each}
        </div>
      </Row>
      {#if d.quickPlay.mode === 'world'}
        <Row title={t('World')}>
          {#if worlds.length}
            <Select
              bind:value={() => d.quickPlay.value, (v) => (d.quickPlay.value = v ?? '')}
              ariaLabel={t('World')}
              placeholder={t('Choose a world')}
              options={worlds.map((w) => ({ value: w, label: w }))}
            />
          {:else}
            <span class="hint">{t('No worlds yet. Play once to create one.')}</span>
          {/if}
        </Row>
      {:else if d.quickPlay.mode === 'server'}
        <Row title={t('Server address')}><input class="input" bind:value={d.quickPlay.value} placeholder="play.example.net:25565" /></Row>
      {:else if d.quickPlay.mode === 'realm'}
        <Row title={t('Realm ID')}><input class="input" bind:value={d.quickPlay.value} placeholder="1234567" /></Row>
      {/if}
      <Row
        title={t('Window size')}
        override={d.windowWidth !== null}
        onoverride={(on) => {
          d.windowWidth = on ? g.windowWidth : null;
          d.windowHeight = on ? g.windowHeight : null;
        }}
        globalHint="{g.windowWidth}×{g.windowHeight}"
      >
        <input class="input num" type="number" min="640" value={d.windowWidth ?? g.windowWidth} oninput={(e) => (d.windowWidth = +e.currentTarget.value)} />
        <span class="x">×</span>
        <input class="input num" type="number" min="480" value={d.windowHeight ?? g.windowHeight} oninput={(e) => (d.windowHeight = +e.currentTarget.value)} />
      </Row>
      <Row
        title={t('Start fullscreen')}
        override={d.fullscreen !== null}
        onoverride={(on) => (d.fullscreen = on ? g.fullscreen : null)}
        globalHint={g.fullscreen ? 'on' : 'off'}
      >
        <Switch bind:checked={() => d.fullscreen ?? g.fullscreen, (v) => (d.fullscreen = v)} label={t('Fullscreen')} />
      </Row>
    </div>
  </section>

  <section class="panel glass">
    <h3>{t('Commands')}</h3>
    <p class="sub">{t('Run your own commands around the game. Available:')} <code>$INST_NAME</code> <code>$INST_ID</code> <code>$INST_DIR</code> <code>$INST_MC_DIR</code> <code>$INST_JAVA</code></p>
    <div class="rows">
      <Row
        title={t('Enable commands')}
        override={d.enableCommands !== null}
        onoverride={(on) => (d.enableCommands = on ? true : null)}
        globalHint={g.enableCommands ? 'on' : 'off'}
      >
        <Switch bind:checked={() => d.enableCommands ?? g.enableCommands, (v) => (d.enableCommands = v)} label={t('Enable commands')} />
      </Row>
      <Row title={t('Pre-launch command')} desc={t('Runs before the game starts; a failure cancels the launch')} override={d.preLaunchCommand !== null} onoverride={(on) => (d.preLaunchCommand = on ? g.preLaunchCommand : null)}>
        <input class="input mono" value={d.preLaunchCommand ?? g.preLaunchCommand} oninput={(e) => (d.preLaunchCommand = e.currentTarget.value)} />
      </Row>
      <Row title={t('Post-exit command')} desc={t('Runs after the game closes')} override={d.postExitCommand !== null} onoverride={(on) => (d.postExitCommand = on ? g.postExitCommand : null)}>
        <input class="input mono" value={d.postExitCommand ?? g.postExitCommand} oninput={(e) => (d.postExitCommand = e.currentTarget.value)} />
      </Row>
      <Row title={t('Wrapper command')} desc={t('Prefixed to the Java command, e.g. prime-run or mangohud')} override={d.wrapperCommand !== null} onoverride={(on) => (d.wrapperCommand = on ? g.wrapperCommand : null)}>
        <input class="input mono" value={d.wrapperCommand ?? g.wrapperCommand} oninput={(e) => (d.wrapperCommand = e.currentTarget.value)} />
      </Row>
      <Row
        title={t('Environment variables')}
        desc={t('One KEY=value per line, added to the ones in Settings (a name set in both uses this one)')}
        override={d.envVars != null}
        onoverride={(on) => (d.envVars = on ? '' : null)}
        wide
      >
        <textarea class="input mono" rows="3" spellcheck="false" placeholder={t('MESA_GL_VERSION_OVERRIDE=4.5')} disabled={d.envVars == null} value={d.envVars ?? ''} oninput={(e) => (d.envVars = e.currentTarget.value)}></textarea>
      </Row>
    </div>
  </section>

  {#if app.info?.os === 'linux'}
    <section class="panel glass">
      <h3>{t('Advanced')}</h3>
      <div class="rows">
        <Row title={t('Use system GLFW')} desc={t('Use your distro\'s GLFW instead of Minecraft\'s (Wayland, ARM)')} override={d.useSystemGlfw !== null} onoverride={(on) => (d.useSystemGlfw = on ? true : null)} globalHint={g.useSystemGlfw ? 'on' : 'off'}>
          <Switch bind:checked={() => d.useSystemGlfw ?? g.useSystemGlfw, (v) => (d.useSystemGlfw = v)} label={t('System GLFW')} />
        </Row>
        <Row title={t('Use system OpenAL')} override={d.useSystemOpenal !== null} onoverride={(on) => (d.useSystemOpenal = on ? true : null)} globalHint={g.useSystemOpenal ? 'on' : 'off'}>
          <Switch bind:checked={() => d.useSystemOpenal ?? g.useSystemOpenal, (v) => (d.useSystemOpenal = v)} label={t('System OpenAL')} />
        </Row>
        <Row title={t('Feral GameMode')} override={d.gamemode != null} onoverride={(on) => (d.gamemode = on ? !g.gamemode : null)} globalHint={g.gamemode ? 'on' : 'off'}>
          <Switch bind:checked={() => d.gamemode ?? g.gamemode, (v) => (d.gamemode = v)} label={t('GameMode')} />
        </Row>
        <Row title={t('MangoHud')} desc={t('FPS and frame-time overlay')} override={d.mangohud != null} onoverride={(on) => (d.mangohud = on ? !g.mangohud : null)} globalHint={g.mangohud ? 'on' : 'off'}>
          <Switch bind:checked={() => d.mangohud ?? g.mangohud, (v) => (d.mangohud = v)} label={t('MangoHud')} />
        </Row>
        <Row title={t('Use the dedicated graphics card')} override={d.discreteGpu != null} onoverride={(on) => (d.discreteGpu = on ? !g.discreteGpu : null)} globalHint={g.discreteGpu ? 'on' : 'off'}>
          <Switch bind:checked={() => d.discreteGpu ?? g.discreteGpu, (v) => (d.discreteGpu = v)} label={t('Dedicated graphics card')} />
        </Row>
      </div>
    </section>
  {/if}
</div>

{#if optimizing}
  <OptimizeDialog {inst} onapply={applyTuning} onclose={() => (optimizing = false)} />
{/if}

{#if dirty}
  <div class="savebar glass" transition:fly={{ y: 20, duration: 220 }}>
    <span>{t('You have unsaved changes')}</span>
    <button class="btn ghost" onclick={() => (d = clone(inst))}>{t('Discard')}</button>
    <button class="btn primary" onclick={save} disabled={!d.name.trim()}>{t('Save changes')}</button>
  </div>
{/if}

<style>
  .shared-note {
    margin: -4px 0 10px;
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--muted);
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 4px;
  }
  .head h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 650;
  }
  .btn.small {
    height: 30px;
    padding: 0 12px;
    font-size: 12px;
    gap: 6px;
  }
  .pic {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .pic-icon,
  .pic-banner {
    display: block;
    overflow: hidden;
    border-radius: 8px;
    border: 1px solid var(--stroke);
    flex-shrink: 0;
  }
  .pic-icon {
    width: 40px;
    height: 40px;
  }
  .pic-banner {
    width: 120px;
    height: 40px;
  }
  .settings {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding-bottom: 70px;
  }
  .val {
    width: 60px;
    text-align: right;
    font-size: 13px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  input[type='range'] {
    flex: 1;
    accent-color: var(--accent);
  }
  .num {
    width: 110px;
  }
  .x {
    color: var(--faint);
  }
  .hint {
    font-size: 12.5px;
    color: var(--faint);
  }
  .hint.warn {
    color: #fcd34d;
  }
  .warning {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 4px 0 0;
    padding: 10px 12px;
    border-radius: 10px;
    font-size: 12.5px;
    color: #fde68a;
    background: rgba(245, 158, 11, 0.1);
    border: 1px solid rgba(245, 158, 11, 0.25);
  }
  code {
    font-size: 11.5px;
    padding: 1px 5px;
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.07);
    color: var(--muted);
  }
  .savebar {
    position: sticky;
    bottom: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: -54px;
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
