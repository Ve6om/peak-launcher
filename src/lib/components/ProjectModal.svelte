<script lang="ts">
  import { onMount } from 'svelte';
  import { Download, Heart, Check, LoaderCircle, ExternalLink, Code, Bug, BookOpen } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import Markdown from './Markdown.svelte';
  import ProjectIcon from './ProjectIcon.svelte';
  import { api } from '$lib/api';
  import { compact, ago, toast, errorText, installModpack, requestInstall } from '$lib/state.svelte';
  import type { BrowseKind, ContentVersion, Project } from '$lib/types';

  let {
    projectId,
    instanceId,
    kind,
    installed = false,
    onclose,
    oninstalled
  }: {
    projectId: string;
    instanceId: string;
    kind: BrowseKind;
    installed?: boolean;
    onclose: () => void;
    oninstalled: () => void;
  } = $props();

  let project = $state<Project | null>(null);
  let versions = $state<ContentVersion[]>([]);
  let versionId = $state('');
  let installing = $state(false);
  let image = $state<string | null>(null);

  onMount(async () => {
    try {
      const k = kind;
      const [p, v] = await Promise.all([
        api.contentProject(projectId),
        k === 'modpack' ? api.modpackVersions(projectId) : api.contentVersions(instanceId, k, projectId)
      ]);
      project = p;
      versions = v;
      versionId = (v.find((x) => x.version_type === 'release') ?? v[0])?.id ?? '';
    } catch (e) {
      toast(errorText(e), 'error');
      onclose();
    }
  });

  async function install() {
    const k = kind;
    installing = true;
    if (k === 'modpack') {
      const inst = await installModpack({ projectId, versionId: versionId || null });
      installing = false;
      if (inst) onclose();
      return;
    }
    // Close first so the dependency confirmation (if any) is on top.
    const [inst, pid, vid] = [instanceId, projectId, versionId || null];
    onclose();
    if (await requestInstall(inst, k, pid, vid)) oninstalled();
  }

  const links = $derived(
    project
      ? ([
          ['Source', project.source_url, Code],
          ['Issues', project.issues_url, Bug],
          ['Wiki', project.wiki_url, BookOpen],
          ['Discord', project.discord_url, ExternalLink],
          project.id.startsWith('cf:')
            ? ['CurseForge', project.website_url, ExternalLink]
            : project.id.startsWith('ftb:')
              ? ['Feed The Beast', project.website_url, ExternalLink]
              : ['Modrinth', `https://modrinth.com/${project.project_type}/${project.slug}`, ExternalLink]
        ].filter(([, url]) => !!url) as [string, string, typeof Code][])
      : []
  );
</script>

<Modal title={project?.title ?? 'Loading…'} subtitle={project?.description ?? ''} width={760} {onclose}>
  {#if !project}
    <div class="loading"><LoaderCircle size={22} class="spin" /></div>
  {:else}
    <div class="wrap">
      <div class="top">
        <ProjectIcon url={project.icon_url} title={project.title} size={64} />
        <div class="stats">
          <span><Download size={14} /> {compact(project.downloads)} downloads</span>
          <span><Heart size={14} /> {compact(project.followers)} followers</span>
          {#if project.updated}<span>Updated {ago(project.updated).toLowerCase()}</span>{/if}
          <div class="cats">
            {#each project.categories.slice(0, 5) as c (c)}<span class="chip">{c}</span>{/each}
          </div>
        </div>
        <div class="install">
          {#if versions.length}
            <select class="input" bind:value={versionId}>
              {#each versions as v (v.id)}
                <option value={v.id}>{v.version_number}{v.version_type !== 'release' ? ` (${v.version_type})` : ''}</option>
              {/each}
            </select>
            <button class="btn primary" disabled={installing || !versionId} onclick={install}>
              {#if installing}<LoaderCircle size={16} class="spin" /> Installing{:else if kind === 'modpack'}<Download size={16} /> Install as new instance{:else if installed}<Check size={16} /> Reinstall{:else}<Download size={16} /> Install{/if}
            </button>
          {:else}
            <span class="none">{kind === 'modpack' ? 'This modpack has no versions' : "No version for this instance's game version and loader"}</span>
          {/if}
        </div>
      </div>

      {#if links.length}
        <div class="links">
          {#each links as [label, url, Icon] (label)}
            <button class="btn ghost" onclick={() => api.openUrl(url)}><Icon size={14} /> {label}</button>
          {/each}
        </div>
      {/if}

      {#if project.gallery.length}
        <div class="gallery">
          {#each project.gallery.slice(0, 8) as g (g.url)}
            <button class="shot" onclick={() => (image = g.url)}><img src={g.url} alt={g.title ?? ''} loading="lazy" /></button>
          {/each}
        </div>
      {/if}

      <div class="body"><Markdown source={project.body} /></div>
    </div>
  {/if}
</Modal>

{#if image}
  <button class="lightbox" onclick={() => (image = null)} aria-label="Close image"><img src={image} alt="" /></button>
{/if}

<style>
  .loading {
    display: grid;
    place-items: center;
    height: 200px;
    color: var(--muted);
  }
  .wrap {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-height: 0;
    overflow-y: auto;
    margin: 0 -22px -22px;
    padding: 0 22px 22px;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 16px;
  }
  .stats {
    flex: 1;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .stats > span {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .cats {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    width: 100%;
  }
  .install {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 220px;
    flex-shrink: 0;
  }
  .none {
    font-size: 12.5px;
    color: #fcd34d;
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin: -6px 0 0 -10px;
  }
  .links .btn {
    height: 30px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .gallery {
    display: flex;
    gap: 8px;
    overflow-x: auto;
    padding-bottom: 4px;
  }
  .shot {
    flex-shrink: 0;
    width: 180px;
    aspect-ratio: 16 / 9;
    border-radius: 10px;
    overflow: hidden;
    box-shadow: 0 0 0 1px var(--stroke) inset;
  }
  .shot img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 0.3s var(--ease);
  }
  .shot:hover img {
    transform: scale(1.05);
  }
  .body {
    padding-top: 4px;
    border-top: 1px solid var(--stroke);
  }
  .lightbox {
    position: fixed;
    inset: 0;
    z-index: 300;
    display: grid;
    place-items: center;
    padding: 40px;
    background: rgba(0, 0, 0, 0.8);
    backdrop-filter: blur(8px);
    cursor: zoom-out;
  }
  .lightbox img {
    max-width: 100%;
    max-height: 100%;
    border-radius: 12px;
  }
</style>
