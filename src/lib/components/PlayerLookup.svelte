<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // Look up any player by name: their id, current skin in 3D and cape from
  // Mojang, plus past names and skins from Crafty.gg (Mojang no longer
  // shares those). NameMC is linked too, since its records go back further
  // for some players.
  import { Search, LoaderCircle, Copy, ExternalLink, UserSearch } from '@lucide/svelte';
  import SkinViewer from './SkinViewer.svelte';
  import SkinHead from './SkinHead.svelte';
  import { api } from '$lib/api';
  import { toast, errorText } from '$lib/state.svelte';
  import type { Player, Skin } from '$lib/types';

  let query = $state('');
  let busy = $state(false);
  let player = $state<Player | null>(null);
  let error = $state<string | null>(null);
  // A past skin picked to view on the model (null shows the current one).
  let viewing = $state<Skin | null>(null);
  // A cape picked to view on the model (null shows the one they wear).
  let viewingCape = $state<string | null>(null);
  const shown = $derived(viewing ?? player?.skin ?? null);
  const shownCape = $derived(viewingCape ?? (viewing ? null : (player?.cape ?? null)));

  const valid = $derived(/^\w{1,16}$/.test(query.trim()));

  async function search() {
    if (!valid || busy) return;
    busy = true;
    error = null;
    try {
      player = await api.playerLookup(query.trim());
      viewing = null;
      viewingCape = null;
    } catch (e) {
      player = null;
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  function since(iso: string): string {
    return new Date(iso).toLocaleDateString(undefined, { year: 'numeric', month: 'short' });
  }

  function nameNote(i: number, changedAt: string | null, count: number): string {
    const parts: string[] = [];
    if (i === 0) parts.push(t('Current'));
    if (changedAt) parts.push(`since ${since(changedAt)}`);
    else if (i > 0 && i === count - 1) parts.push(t('First name'));
    return parts.join(' · ');
  }

  async function copyId() {
    if (!player) return;
    try {
      await navigator.clipboard.writeText(player.id);
      toast(t('Player ID copied'), 'success');
    } catch {
      toast(t('Couldn\'t copy to the clipboard'), 'error');
    }
  }
</script>

<section class="lookup glass rise">
  <div class="head">
    <div>
      <span class="label">{t('Player lookup')}</span>
      <p>{t('See anyone\'s current skin, cape and player ID.')}</p>
    </div>
    <form
      class="form"
      onsubmit={(e) => {
        e.preventDefault();
        search();
      }}
    >
      <input class="input" bind:value={query} placeholder={t('Username')} maxlength="16" spellcheck="false" autocomplete="off" aria-label={t('Username to look up')} />
      <button class="btn" type="submit" disabled={!valid || busy}>
        {#if busy}<LoaderCircle size={15} class="spin" />{:else}<Search size={15} />{/if} {t('Look up')}
      </button>
    </form>
  </div>

  {#if error}
    <p class="error">{error}</p>
  {:else if player}
    <div class="result">
      <div class="stage">
        {#if shown}
          {#key shown.png}
            <SkinViewer png={shown.png} model={shown.model} cape={shownCape} width={170} height={220} />
          {/key}
        {:else}
          <p class="muted">{t('Uses Minecraft\'s default skin.')}</p>
        {/if}
        {#if viewing || viewingCape}
          <button class="back btn ghost" onclick={() => ((viewing = null), (viewingCape = null))}>{t('Back to current look')}</button>
        {/if}
      </div>
      <div class="info">
        <h3>{player.name}</h3>
        <div class="id">
          <code>{player.id}</code>
          <button class="btn ghost icon" title={t('Copy player ID')} aria-label={t('Copy player ID')} onclick={copyId}><Copy size={14} /></button>
        </div>
        <ul>
          <li>{player.skin ? (player.skin.model === 'slim' ? t('Slim arms (Alex style)') : t('Classic arms (Steve style)')) : t('Default skin')}</li>
          <li>{player.cape ? t('Wears a cape (shown on the model)') : t('No cape')}</li>
        </ul>

        {#if player.historyFound}
          <div class="section">
            <span class="label">{t('Names')}</span>
            <ol class="names">
              {#each player.names as n, i (n.name + i)}
                <li>
                  <b>{n.name}</b>
                  <span>{nameNote(i, n.changedAt, player.names.length)}</span>
                </li>
              {/each}
            </ol>
          </div>
          {#if player.pastSkins.length > 1}
            <div class="section">
              <span class="label">{t('Past skins · click one to see it')}</span>
              <div class="past">
                {#each player.pastSkins as s, i (i)}
                  {@const current = s.png === player.skin?.png}
                  <button
                    class="past-pick"
                    class:selected={viewing ? viewing === s : current}
                    title={current ? t('Current skin') : t('View this skin')}
                    onclick={() => (viewing = current ? null : s)}
                  >
                    <SkinHead accountId={null} name={player.name} png={s.png} size={34} />
                  </button>
                {/each}
              </div>
            </div>
          {/if}
          {#if player.capes.length}
            <div class="section">
              <span class="label">{t('Capes · click one to see it')}</span>
              <div class="capes">
                {#each player.capes as c, i (i)}
                  <button
                    class="cape"
                    class:selected={viewingCape ? viewingCape === c.png : c.current && !viewing}
                    title={c.current ? t('{name} (wearing now)', { name: c.name }) : c.name}
                    onclick={() => (viewingCape = c.current && !viewing ? null : c.png)}
                  >
                    <span class="cape-art" style:background-image="url({c.png})"></span>
                    <span class="cape-name">{c.name}</span>
                    {#if c.current}<span class="worn">{t('Wearing')}</span>{/if}
                  </button>
                {/each}
              </div>
            </div>
          {/if}
        {:else}
          <span class="hint">{t('Couldn\'t reach Crafty.gg for past names and skins. Try again later.')}</span>
        {/if}

        <div class="links">
          <button class="btn" onclick={() => api.openUrl(`https://namemc.com/profile/${player!.id}`)}>
            <ExternalLink size={14} /> {t('View on NameMC')}
          </button>
          <button class="btn ghost" onclick={() => api.openUrl(`https://crafty.gg/@${player!.name}`)}>
            <ExternalLink size={14} /> {t('Crafty.gg')}
          </button>
        </div>
        <span class="hint">{t('History from Crafty.gg. It can miss some older names; NameMC sometimes has more.')}</span>
      </div>
    </div>
  {:else}
    <div class="placeholder"><UserSearch size={16} /> {t('Type a username and press Enter.')}</div>
  {/if}
</section>

<style>
  .lookup {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 16px 18px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 20px;
    flex-wrap: wrap;
  }
  .head p {
    margin: 4px 0 0;
    font-size: 12.5px;
    color: var(--faint);
  }
  .form {
    display: flex;
    gap: 8px;
    width: 340px;
    max-width: 100%;
  }
  .error {
    margin: 0;
    color: #ff9a9d;
    font-size: 13px;
  }
  .placeholder {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    color: var(--faint);
  }
  .result {
    display: flex;
    align-items: center;
    gap: 24px;
  }
  .stage {
    position: relative;
    align-self: flex-start;
    width: 170px;
    height: 220px;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    border-radius: var(--radius);
    background: radial-gradient(circle at 50% 45%, color-mix(in srgb, var(--accent) 12%, transparent), transparent 65%), rgba(0, 0, 0, 0.18);
  }
  .back {
    position: absolute;
    bottom: 6px;
    left: 50%;
    transform: translateX(-50%);
    font-size: 11.5px;
    padding: 4px 9px;
    white-space: nowrap;
  }
  .section {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 4px;
  }
  .names {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 13px;
  }
  .names li {
    display: flex;
    gap: 10px;
    align-items: baseline;
  }
  .names b {
    font-weight: 600;
  }
  .names span {
    font-size: 12px;
    color: var(--faint);
  }
  .past {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .past-pick {
    padding: 4px;
    border-radius: 10px;
    border: 1px solid var(--stroke);
    background: rgba(255, 255, 255, 0.03);
    cursor: pointer;
    line-height: 0;
  }
  .past-pick.selected {
    border-color: color-mix(in srgb, var(--accent) 60%, transparent);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }
  .capes {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .cape {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    width: 70px;
    padding: 6px 4px;
    border-radius: 10px;
    border: 1px solid var(--stroke);
    background: rgba(255, 255, 255, 0.03);
    color: var(--muted);
    font-size: 11px;
    cursor: pointer;
  }
  .cape.selected {
    border-color: color-mix(in srgb, var(--accent) 60%, transparent);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    color: var(--text);
  }
  .cape-art {
    width: 26px;
    height: 42px;
    /* The cape's back panel sits at x 1-10, y 1-16 of its 64x32 texture. */
    background-size: 640% 300%;
    background-position: 1.85% 3.2%;
    image-rendering: pixelated;
  }
  .cape-name {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .worn {
    font-size: 10px;
    color: var(--accent);
  }
  .links {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 6px;
  }
  .muted {
    margin: 0;
    padding: 0 14px;
    text-align: center;
    font-size: 12.5px;
    color: var(--muted);
  }
  .info {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    min-width: 0;
  }
  h3 {
    margin: 0;
    font-size: 22px;
  }
  .id {
    display: flex;
    align-items: center;
    gap: 4px;
    max-width: 100%;
  }
  code {
    font-size: 12px;
    padding: 2px 7px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.06);
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  ul {
    margin: 0;
    padding-left: 18px;
    font-size: 13px;
    color: var(--muted);
    line-height: 1.6;
  }
  .hint {
    font-size: 11.5px;
    color: var(--faint);
  }
</style>
