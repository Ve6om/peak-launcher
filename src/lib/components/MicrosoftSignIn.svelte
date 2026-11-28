<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // The Microsoft sign-in window. Two ways in, and Peak finishes by itself
  // whichever one is used: sign in with this computer's browser, or use a
  // code (and QR code) from any other device. The code page is always opened
  // plain: Microsoft rejects microsoft.com/link links with the code filled in
  // (?otc=CODE) with a "first party application" error, so the user types it.
  import { onMount } from 'svelte';
  import { Copy, ExternalLink, Globe, Link2, LoaderCircle, Smartphone } from '@lucide/svelte';
  import { renderSVG } from 'uqr';
  import Modal from './Modal.svelte';
  import { api } from '$lib/api';
  import { toast } from '$lib/state.svelte';
  import type { LoginStart } from '$lib/types';

  let { start, oncancel }: { start: LoginStart; oncancel: () => void } = $props();

  const code = $derived(start.device);
  const page = $derived(code.verificationUri.replace(/^https?:\/\/(www\.)?/, ''));
  /** Microsoft's code page, where the user types the code. */
  const link = $derived(code.verificationUri);
  const qr = $derived(renderSVG(link, { ecc: 'M', border: 1, blackColor: '#101216', whiteColor: '#ffffff' }));

  let browserOpened = $state(false);

  // Time left before Microsoft stops accepting the code.
  let remaining = $state(0);
  onMount(() => {
    const end = Date.now() + start.device.expiresIn * 1000;
    const tick = () => (remaining = Math.max(0, Math.round((end - Date.now()) / 1000)));
    tick();
    const timer = setInterval(tick, 1000);
    return () => clearInterval(timer);
  });
  const clock = $derived(`${Math.floor(remaining / 60)}:${String(remaining % 60).padStart(2, '0')}`);

  async function copy(text: string, done: string) {
    try {
      await navigator.clipboard.writeText(text);
      toast(done);
    } catch {
      toast(t('Couldn\'t copy. Select the text and copy it yourself.'), 'error');
    }
  }

  function openBrowser() {
    if (!start.browserUrl) return;
    browserOpened = true;
    api.openUrl(start.browserUrl);
  }

  async function copyAndOpenPage() {
    await copy(code.userCode, 'Code copied. Finish signing in in your browser.');
    api.openUrl(link);
  }
</script>

<Modal title={t('Add your Microsoft account')} subtitle={t('Choose how you\'d like to sign in. Peak finishes by itself once you\'re done.')} width={700} onclose={oncancel}>
  <div class="cards" class:single={!start.browserUrl}>
    {#if start.browserUrl}
      <section class="card">
        <span class="label">{t('On this computer')}</span>
        <b>{t('Sign in with your browser')}</b>
        <p>{t('Peak opens Microsoft\'s sign-in page and picks up the result on its own. There\'s no code to type.')}</p>
        <button class="btn primary go" onclick={openBrowser}><Globe size={16} /> {t('Continue in browser')}</button>
        {#if browserOpened}
          <p class="note">{t('Opened in your browser. Finish signing in there, then come back here.')}</p>
          <button class="linkish" onclick={() => copy(start.browserUrl!, 'Sign-in link copied')}>{t('Browser didn\'t open? Copy the link')}</button>
        {/if}
      </section>
    {/if}

    <section class="card">
      <span class="label">{t('On another device')}</span>
      <b>{t('Use a code')}</b>
      <div class="qr-row">
        <div class="tile" role="img" aria-label={t('QR code for Microsoft\'s code page')}>{@html qr}</div>
        <div class="code-side">
          <span class="scan"><Smartphone size={13} /> {t('Scan the code or open this page on your phone, then enter the code below:')} <button class="linkish" onclick={() => api.openUrl(link)}>{page}</button></span>
          <div class="code">{code.userCode}</div>
        </div>
      </div>
      <button class="btn go" onclick={copyAndOpenPage}><ExternalLink size={15} /> {t('Copy code & open page')}</button>
      <div class="row">
        <button class="btn" onclick={() => copy(code.userCode, t('Code copied'))}><Copy size={15} /> {t('Copy code')}</button>
        <button class="btn" onclick={() => copy(link, t('Page link copied'))}><Link2 size={15} /> {t('Copy page link')}</button>
      </div>
    </section>
  </div>

  <footer>
    <span class="waiting"><LoaderCircle size={15} class="spin" /> {t('Waiting for you to finish signing in…')}</span>
    <span class="left">{t('Code expires in {clock}', { clock })}</span>
    <button class="btn ghost" onclick={oncancel}>{t('Cancel')}</button>
  </footer>
</Modal>

<style>
  .cards {
    display: grid;
    grid-template-columns: 1fr 1.25fr;
    gap: 14px;
    align-items: stretch;
  }
  .cards.single {
    grid-template-columns: 1fr;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px;
    border-radius: var(--radius);
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--stroke);
  }
  .card b {
    font-size: 15px;
  }
  .card p {
    margin: 0;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--muted);
  }
  .card .note {
    color: var(--accent-2);
  }
  .go {
    justify-content: center;
  }
  .card > .btn.primary.go {
    margin-top: auto;
    height: 40px;
  }
  .qr-row {
    display: flex;
    gap: 14px;
    align-items: center;
  }
  .tile {
    width: 132px;
    height: 132px;
    flex-shrink: 0;
    padding: 6px;
    border-radius: 12px;
    background: #fff;
    box-shadow: 0 6px 22px rgba(0, 0, 0, 0.4);
  }
  .tile :global(svg) {
    display: block;
    width: 100%;
    height: 100%;
  }
  .code-side {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .scan {
    font-size: 12px;
    line-height: 1.45;
    color: var(--muted);
  }
  .scan :global(svg) {
    vertical-align: -2px;
    margin-right: 3px;
  }
  .code {
    text-align: center;
    font-family: 'JetBrains Mono', Consolas, monospace;
    font-size: 22px;
    font-weight: 700;
    letter-spacing: 0.1em;
    padding: 8px 6px;
    border-radius: 12px;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid var(--stroke-strong);
    user-select: all;
  }
  .row {
    display: flex;
    gap: 8px;
  }
  .row .btn {
    flex: 1;
    justify-content: center;
  }
  .linkish {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    color: var(--accent);
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
    text-align: left;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-top: 18px;
    padding-top: 14px;
    border-top: 1px solid var(--stroke);
    font-size: 12.5px;
    color: var(--muted);
  }
  .waiting {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    flex: 1;
  }
  .left {
    font-variant-numeric: tabular-nums;
    color: var(--faint);
  }
</style>
