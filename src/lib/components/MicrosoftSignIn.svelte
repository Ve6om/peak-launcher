<script lang="ts">
  // The Microsoft sign-in window: scan a QR code, open the link, or copy the
  // code and link, whichever is easiest. Microsoft's "microsoft.com/link" page
  // fills the code in by itself when it's given as ?otc=CODE.
  import { onMount } from 'svelte';
  import { Copy, ExternalLink, Link2, LoaderCircle, Smartphone } from '@lucide/svelte';
  import { renderSVG } from 'uqr';
  import Modal from './Modal.svelte';
  import { api } from '$lib/api';
  import { toast } from '$lib/state.svelte';
  import type { DeviceCode } from '$lib/types';

  let { code, oncancel }: { code: DeviceCode; oncancel: () => void } = $props();

  const page = $derived(code.verificationUri.replace(/^https?:\/\/(www\.)?/, ''));
  /** The link with the code already filled in. */
  const link = $derived(`${code.verificationUri}?otc=${encodeURIComponent(code.userCode)}`);
  const qr = $derived(renderSVG(link, { ecc: 'M', border: 1, blackColor: '#101216', whiteColor: '#ffffff' }));

  // Time left before Microsoft stops accepting the code.
  let remaining = $state(0);
  onMount(() => {
    const end = Date.now() + code.expiresIn * 1000;
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
      toast("Couldn't copy. Select the text and copy it yourself.", 'error');
    }
  }

  async function openAndCopy() {
    await copy(code.userCode, 'Code copied. Finish signing in in your browser.');
    api.openUrl(link);
  }
</script>

<Modal title="Sign in with Microsoft" subtitle="Pick whichever way is easiest. Peak finishes by itself once you're signed in." width={580} onclose={oncancel}>
  <div class="layout">
    <div class="qr">
      <div class="tile" role="img" aria-label="QR code for the sign-in link">{@html qr}</div>
      <span><Smartphone size={13} /> Scan with your phone</span>
    </div>

    <div class="side">
      <span class="label">Your code</span>
      <div class="code">{code.userCode}</div>

      <button class="btn primary open" onclick={openAndCopy}><ExternalLink size={16} /> Open link &amp; copy code</button>

      <div class="row">
        <button class="btn" onclick={() => copy(code.userCode, 'Code copied')}><Copy size={15} /> Copy code</button>
        <button class="btn" onclick={() => copy(link, 'Link copied')}><Link2 size={15} /> Copy link</button>
      </div>

      <p class="manual">
        Or go to <button class="linkish" onclick={() => api.openUrl(link)}>{page}</button> yourself and enter the code.
      </p>
    </div>
  </div>

  <footer>
    <span class="waiting"><LoaderCircle size={15} class="spin" /> Waiting for you to finish signing in…</span>
    <span class="left">Code expires in {clock}</span>
    <button class="btn ghost" onclick={oncancel}>Cancel</button>
  </footer>
</Modal>

<style>
  .layout {
    display: flex;
    gap: 24px;
    align-items: stretch;
  }
  .qr {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }
  .tile {
    width: 176px;
    height: 176px;
    padding: 8px;
    border-radius: 14px;
    background: #fff;
    box-shadow: 0 8px 28px rgba(0, 0, 0, 0.4);
  }
  .tile :global(svg) {
    display: block;
    width: 100%;
    height: 100%;
  }
  .qr span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--muted);
  }
  .side {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
    justify-content: center;
  }
  .code {
    text-align: center;
    font-family: 'JetBrains Mono', Consolas, monospace;
    font-size: 26px;
    font-weight: 700;
    letter-spacing: 0.12em;
    padding: 10px 12px;
    border-radius: 14px;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid var(--stroke-strong);
    user-select: all;
  }
  .open {
    justify-content: center;
  }
  .row {
    display: flex;
    gap: 8px;
  }
  .row .btn {
    flex: 1;
    justify-content: center;
  }
  .manual {
    margin: 0;
    font-size: 12.5px;
    color: var(--muted);
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
  }
  footer {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-top: 20px;
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
