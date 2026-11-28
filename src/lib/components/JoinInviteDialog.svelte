<script lang="ts">
  import { t, th } from '$lib/i18n.svelte';
  // A friend's invite (a peak://join link): which server, what Peak will do
  // (use an instance you have, switch its pack version, or install the pack),
  // and Play once you agree.
  import { Play, Download, ArrowDownUp, Server } from '@lucide/svelte';
  import { instanceIcon } from '$lib/instanceImages';
  import Select from './Select.svelte';
  import { shownAddress } from '$lib/privacy.svelte';
  import Modal from './Modal.svelte';
  import { invite, inviteListing } from '$lib/invite.svelte';
  import { packMatch, planFor, vanillaChoices, playListing, switchAndJoin } from '$lib/discover.svelte';

  const i = $derived(invite.current!);
  const l = $derived(inviteListing(i));
  const match = $derived(packMatch(l));
  const plan = $derived(planFor(l));
  const choices = $derived(l.pack ? [] : vanillaChoices(l));
  let chosen = $state<string>('');
  $effect(() => {
    if (!chosen) chosen = choices[0]?.id ?? (i.gameVersion ? 'new' : '');
  });
  let busy = $state(false);

  function close() {
    invite.current = null;
  }

  async function play() {
    busy = true;
    const listing = l;
    const p = plan;
    close();
    if (p.kind === 'switch') await switchAndJoin(listing, p.inst);
    else await playListing(listing, listing.pack ? null : chosen || null);
    busy = false;
  }
</script>

<Modal title={t('Join {name}', { name: i.name })} subtitle={t('A friend invited you to play on {address}', { address: shownAddress(i.address) })} width={500} onclose={close}>
  <div class="what">
    <Server size={16} />
    {#if l.pack}
      <span>
        {t('Modpack:')} <b>{i.packName || t('the server’s pack')}</b>{#if i.versionName} · {i.versionName}{/if}
      </span>
    {:else}
      <span>{t('Minecraft {x} (no modpack)', { x: i.gameVersion ?? '' })}</span>
    {/if}
  </div>

  {#if l.pack}
    <p class="plan">
      {#if !match}
        {t('Peak installs the pack first (you can watch it on the instance), then joins.')}
      {:else if plan.kind === 'switch'}
        {@html th('Your <b>{instance}</b> is on {from}; Peak switches it to {to} (your old mods are backed up), then joins.', { instance: plan.inst.name, from: plan.from, to: plan.to })}
      {:else}
        {t('Peak joins with your')} <b>{match.inst.name}</b>.
      {/if}
    </p>
  {:else}
    <label class="pick">
      <span>{t('Play with')}</span>
      <Select
        bind:value={() => chosen, (v) => (chosen = v ?? '')}
        ariaLabel={t('Instance to play with')}
        style="flex: 1;"
        options={[
          ...choices.map((c) => ({ value: c.id, label: c.name, hint: c.gameVersion, icon: instanceIcon(c), loader: c.loader })),
          ...(i.gameVersion ? [{ value: 'new', label: t('A new {gameVersion} instance', { gameVersion: i.gameVersion }) }] : [])
        ]}
      />
    </label>
  {/if}

  <div class="actions">
    <button class="btn ghost" onclick={close}>{t('Not now')}</button>
    <button class="btn primary" disabled={busy || (!l.pack && !chosen)} onclick={play}>
      {#if l.pack && !match}<Download size={15} /> {t('Install and join')}{:else if plan.kind === 'switch'}<ArrowDownUp size={15} /> {t('Switch and join')}{:else}<Play size={15} /> {t('Join')}{/if}
    </button>
  </div>
</Modal>

<style>
  .what {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: 10px;
    background: rgba(0, 0, 0, 0.18);
    border: 1px solid var(--stroke);
    font-size: 13px;
  }
  .plan {
    margin: 12px 0 0;
    font-size: 13px;
    line-height: 1.5;
    color: var(--muted);
  }
  .pick {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 12px;
    font-size: 13px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }
</style>
