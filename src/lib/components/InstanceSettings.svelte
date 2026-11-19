<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { fly } from 'svelte/transition';
  import { TriangleAlert } from '@lucide/svelte';
  import Row from './Row.svelte';
  import Switch from './Switch.svelte';
  import JavaPicker from './JavaPicker.svelte';
  import JvmPresetPicker from './JvmPresetPicker.svelte';
  import { api } from '$lib/api';
  import { app, refreshInstances, toast, errorText } from '$lib/state.svelte';
  import { LOADERS, loaderLabel, type Instance, type LoaderVersion, type ManifestEntry, type RuntimeOption } from '$lib/types';

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
  const dirty = $derived(JSON.stringify(d) !== JSON.stringify(inst));
  const maxMem = $derived(Math.max(4096, Math.floor((app.info?.totalMemoryMb ?? 16384) / 512) * 512));

  // ---- data for pickers ----
  let mcVersions = $state<ManifestEntry[]>([]);
  let runtimes = $state<RuntimeOption[]>([]);
  let worlds = $state<string[]>([]);
  let loaderVersions = $state<LoaderVersion[]>([]);

  onMount(async () => {
    api.versions().then((m) => (mcVersions = m.versions)).catch(() => {});
    api.javaRuntimes().then((r) => (runtimes = r)).catch(() => {});
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
  let javaMode = $state<JavaMode>(untrack(() => (inst.javaPath ? 'custom' : inst.javaRuntime ? 'runtime' : 'auto')));
  function setJavaMode(m: JavaMode) {
    javaMode = m;
    if (m !== 'custom') d.javaPath = null;
    if (m !== 'runtime') d.javaRuntime = null;
    if (m === 'runtime' && !d.javaRuntime) d.javaRuntime = runtimes.at(-1)?.component ?? 'java-runtime-delta';
    if (m === 'custom' && d.javaPath === null) d.javaPath = '';
  }

  const qpModes = [
    { id: 'none', label: 'Off' },
    { id: 'world', label: 'World' },
    { id: 'server', label: 'Server' },
    { id: 'realm', label: 'Realm' }
  ] as const;

  async function save() {
    try {
      await api.updateInstance($state.snapshot(d) as Instance);
      await refreshInstances();
      toast(
        inst.loader !== d.loader || inst.gameVersion !== d.gameVersion || inst.loaderVersion !== d.loaderVersion
          ? 'Saved. Game files will update on next launch.'
          : 'Saved',
        'success'
      );
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  const gb = (mb: number) => `${(mb / 1024).toFixed(1)} GB`;
  const versionChanged = $derived(inst.loader !== d.loader || inst.gameVersion !== d.gameVersion || inst.loaderVersion !== d.loaderVersion);
</script>

<div class="settings">
  <section class="panel glass">
    <h3>General</h3>
    <div class="rows">
      <Row title="Name" desc="Shown in your library">
        <input class="input" bind:value={d.name} maxlength="48" />
      </Row>
      <Row title="Notes" desc="Anything you want to remember about this instance" wide>
        <textarea class="input" bind:value={d.notes} placeholder="Seed, server IPs, mod list…"></textarea>
      </Row>
    </div>
  </section>

  <section class="panel glass">
    <h3>Game & mod loader</h3>
    <div class="rows">
      <Row title="Minecraft version">
        <select class="input" bind:value={d.gameVersion}>
          {#each versionChoices as v (v.id)}<option value={v.id}>{v.id}</option>{/each}
        </select>
      </Row>
      <Row title="Mod loader">
        <div class="seg">
          {#each LOADERS as l (l.id)}
            <button class:active={d.loader === l.id} onclick={() => (d.loader = l.id)}>{l.label}</button>
          {/each}
        </div>
      </Row>
      {#if d.loader !== 'vanilla'}
        <Row title="{loaderLabel(d.loader)} version">
          {#if loaderVersions.length}
            <select class="input" bind:value={d.loaderVersion}>
              {#each loaderVersions as v (v.id)}
                <option value={v.id}>{v.id}{v.recommended ? ' (recommended)' : v.stable ? '' : ' (beta)'}</option>
              {/each}
            </select>
          {:else}
            <span class="hint warn">Not available for Minecraft {d.gameVersion}</span>
          {/if}
        </Row>
      {/if}
      {#if versionChanged}
        <p class="warning"><TriangleAlert size={15} /> Game files are reinstalled on next launch. Back up your worlds and check your mods are compatible.</p>
      {/if}
    </div>
  </section>

  <section class="panel glass">
    <h3>Java & memory</h3>
    <div class="rows">
      <Row
        title="Maximum memory"
        desc="RAM the game may use"
        override={d.memoryMb !== null}
        onoverride={(on) => (d.memoryMb = on ? g.memoryMb : null)}
        globalHint={gb(g.memoryMb)}
      >
        <input type="range" min="1024" max={maxMem} step="512" value={d.memoryMb ?? g.memoryMb} oninput={(e) => (d.memoryMb = +e.currentTarget.value)} />
        <span class="val">{gb(d.memoryMb ?? g.memoryMb)}</span>
      </Row>
      <Row
        title="Metaspace"
        desc="Initial metaspace size (MB). Larger modpacks can benefit from 512."
        override={d.metaspaceMb !== null}
        onoverride={(on) => (d.metaspaceMb = on ? g.metaspaceMb : null)}
        globalHint="{g.metaspaceMb} MB"
      >
        <input class="input num" type="number" min="0" step="64" value={d.metaspaceMb ?? g.metaspaceMb} oninput={(e) => (d.metaspaceMb = +e.currentTarget.value)} />
      </Row>
      <Row title="Java" desc="Automatic picks the exact Java version Mojang ships for this game version.">
        <div class="seg">
          <button class:active={javaMode === 'auto'} onclick={() => setJavaMode('auto')}>Automatic</button>
          <button class:active={javaMode === 'runtime'} onclick={() => setJavaMode('runtime')}>Pick version</button>
          <button class:active={javaMode === 'custom'} onclick={() => setJavaMode('custom')}>Custom path</button>
        </div>
      </Row>
      {#if javaMode === 'runtime'}
        <Row title="Java runtime" desc="Downloaded automatically if needed">
          <select class="input" bind:value={d.javaRuntime}>
            {#each runtimes as r (r.component)}
              <option value={r.component}>Java {r.major} ({r.component}){r.installed ? ' · installed' : ''}</option>
            {/each}
          </select>
        </Row>
      {:else if javaMode === 'custom'}
        <Row title="Java executable" wide>
          <JavaPicker bind:value={() => d.javaPath ?? '', (v) => (d.javaPath = v)} />
        </Row>
      {/if}
      <Row
        title="Performance flags"
        desc="Java tuning preset for this instance"
        override={d.jvmPreset !== null}
        onoverride={(on) => (d.jvmPreset = on ? g.jvmPreset : null)}
        wide
      >
        <JvmPresetPicker bind:value={() => d.jvmPreset ?? g.jvmPreset, (v) => (d.jvmPreset = v)} disabled={d.jvmPreset === null} />
      </Row>
      <Row
        title="Extra Java arguments"
        desc="Your own flags, added after the preset"
        override={d.jvmArgs !== null}
        onoverride={(on) => (d.jvmArgs = on ? g.jvmArgs : null)}
        wide
      >
        <textarea class="input mono" rows="2" value={d.jvmArgs ?? g.jvmArgs} oninput={(e) => (d.jvmArgs = e.currentTarget.value)}></textarea>
      </Row>
    </div>
  </section>

  <section class="panel glass">
    <h3>Launch</h3>
    <div class="rows">
      <Row title="Account" desc="Play this instance with a specific account">
        <select class="input" bind:value={() => d.accountId ?? '', (v) => (d.accountId = v || null)}>
          <option value="">Active account</option>
          {#each app.accounts as a (a.id)}<option value={a.id}>{a.username}</option>{/each}
        </select>
      </Row>
      <Row title="Quick play" desc="Jump straight into a world, server or realm when you press Play. Worlds and realms need 1.20+.">
        <div class="seg">
          {#each qpModes as m (m.id)}
            <button class:active={(d.quickPlay.mode || 'none') === m.id} onclick={() => (d.quickPlay = { mode: m.id, value: '' })}>{m.label}</button>
          {/each}
        </div>
      </Row>
      {#if d.quickPlay.mode === 'world'}
        <Row title="World">
          {#if worlds.length}
            <select class="input" bind:value={d.quickPlay.value}>
              <option value="" disabled>Choose a world</option>
              {#each worlds as w (w)}<option value={w}>{w}</option>{/each}
            </select>
          {:else}
            <span class="hint">No worlds yet. Play once to create one.</span>
          {/if}
        </Row>
      {:else if d.quickPlay.mode === 'server'}
        <Row title="Server address"><input class="input" bind:value={d.quickPlay.value} placeholder="play.example.net:25565" /></Row>
      {:else if d.quickPlay.mode === 'realm'}
        <Row title="Realm ID"><input class="input" bind:value={d.quickPlay.value} placeholder="1234567" /></Row>
      {/if}
      <Row
        title="Window size"
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
        title="Start fullscreen"
        override={d.fullscreen !== null}
        onoverride={(on) => (d.fullscreen = on ? g.fullscreen : null)}
        globalHint={g.fullscreen ? 'on' : 'off'}
      >
        <Switch bind:checked={() => d.fullscreen ?? g.fullscreen, (v) => (d.fullscreen = v)} label="Fullscreen" />
      </Row>
    </div>
  </section>

  <section class="panel glass">
    <h3>Commands</h3>
    <p class="sub">Run your own commands around the game. Available: <code>$INST_NAME</code> <code>$INST_ID</code> <code>$INST_DIR</code> <code>$INST_MC_DIR</code> <code>$INST_JAVA</code></p>
    <div class="rows">
      <Row
        title="Enable commands"
        override={d.enableCommands !== null}
        onoverride={(on) => (d.enableCommands = on ? true : null)}
        globalHint={g.enableCommands ? 'on' : 'off'}
      >
        <Switch bind:checked={() => d.enableCommands ?? g.enableCommands, (v) => (d.enableCommands = v)} label="Enable commands" />
      </Row>
      <Row title="Pre-launch command" desc="Runs before the game starts; a failure cancels the launch" override={d.preLaunchCommand !== null} onoverride={(on) => (d.preLaunchCommand = on ? g.preLaunchCommand : null)}>
        <input class="input mono" value={d.preLaunchCommand ?? g.preLaunchCommand} oninput={(e) => (d.preLaunchCommand = e.currentTarget.value)} />
      </Row>
      <Row title="Post-exit command" desc="Runs after the game closes" override={d.postExitCommand !== null} onoverride={(on) => (d.postExitCommand = on ? g.postExitCommand : null)}>
        <input class="input mono" value={d.postExitCommand ?? g.postExitCommand} oninput={(e) => (d.postExitCommand = e.currentTarget.value)} />
      </Row>
      <Row title="Wrapper command" desc="Prefixed to the Java command, e.g. prime-run or mangohud" override={d.wrapperCommand !== null} onoverride={(on) => (d.wrapperCommand = on ? g.wrapperCommand : null)}>
        <input class="input mono" value={d.wrapperCommand ?? g.wrapperCommand} oninput={(e) => (d.wrapperCommand = e.currentTarget.value)} />
      </Row>
    </div>
  </section>

  {#if app.info?.os === 'linux'}
    <section class="panel glass">
      <h3>Advanced</h3>
      <div class="rows">
        <Row title="Use system GLFW" desc="Use your distro's GLFW instead of Minecraft's (Wayland, ARM)" override={d.useSystemGlfw !== null} onoverride={(on) => (d.useSystemGlfw = on ? true : null)} globalHint={g.useSystemGlfw ? 'on' : 'off'}>
          <Switch bind:checked={() => d.useSystemGlfw ?? g.useSystemGlfw, (v) => (d.useSystemGlfw = v)} label="System GLFW" />
        </Row>
        <Row title="Use system OpenAL" override={d.useSystemOpenal !== null} onoverride={(on) => (d.useSystemOpenal = on ? true : null)} globalHint={g.useSystemOpenal ? 'on' : 'off'}>
          <Switch bind:checked={() => d.useSystemOpenal ?? g.useSystemOpenal, (v) => (d.useSystemOpenal = v)} label="System OpenAL" />
        </Row>
      </div>
    </section>
  {/if}
</div>

{#if dirty}
  <div class="savebar glass" transition:fly={{ y: 20, duration: 220 }}>
    <span>You have unsaved changes</span>
    <button class="btn ghost" onclick={() => (d = clone(inst))}>Discard</button>
    <button class="btn primary" onclick={save} disabled={!d.name.trim()}>Save changes</button>
  </div>
{/if}

<style>
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
