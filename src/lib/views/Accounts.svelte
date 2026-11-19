<script lang="ts">
  import { UserPlus, Check, LogOut, Info } from '@lucide/svelte';
  import MicrosoftSignIn from '$lib/components/MicrosoftSignIn.svelte';
  import { api } from '$lib/api';
  import { app, toast, errorText } from '$lib/state.svelte';
  import type { LoginStart } from '$lib/types';

  let login = $state<LoginStart | null>(null);
  let signingIn = $state(false);
  let offlineName = $state('');
  // Each attempt gets a number, so a cancelled one can't report back later.
  let attempt = 0;

  async function signIn() {
    if (!app.info?.msConfigured) {
      toast('Microsoft sign-in needs a client ID first (see the note below).', 'info');
      return;
    }
    const mine = ++attempt;
    signingIn = true;
    try {
      const started = await api.msLoginStart();
      if (mine !== attempt) return;
      login = started;
      const accounts = await api.msLoginFinish(started);
      if (mine !== attempt) return;
      app.accounts = accounts;
      toast('Signed in', 'success');
    } catch (e) {
      if (mine === attempt) toast(errorText(e), 'error');
    } finally {
      if (mine === attempt) {
        signingIn = false;
        login = null;
      }
    }
  }

  /** Close the sign-in window and stop waiting for Microsoft. */
  function cancelSignIn() {
    attempt++;
    signingIn = false;
    login = null;
    api.msLoginCancel().catch(() => {});
  }

  async function addOffline() {
    try {
      app.accounts = await api.addOffline(offlineName);
      offlineName = '';
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  async function activate(id: string) {
    app.accounts = await api.setActiveAccount(id);
  }

  async function remove(id: string) {
    app.accounts = await api.removeAccount(id);
  }
</script>

<div class="page">
  <header>
    <div>
      <h1>Accounts</h1>
      <p>Switch between accounts any time. The active one is used when you press Play.</p>
    </div>
    <button class="btn primary" onclick={signIn} disabled={signingIn}><UserPlus size={17} /> Add Microsoft account</button>
  </header>

  <div class="list">
    {#each app.accounts as a, i (a.id)}
      <div class="account glass rise" class:active={a.active} style:animation-delay="{i * 40}ms">
        <span class="avatar">{a.username[0].toUpperCase()}</span>
        <div class="text">
          <b>{a.username}</b>
          <span>{a.kind === 'microsoft' ? 'Microsoft account' : 'Offline (dev only)'}</span>
        </div>
        {#if a.active}
          <span class="chip active-chip"><Check size={12} /> Active</span>
        {:else}
          <button class="btn" onclick={() => activate(a.id)}>Use this account</button>
        {/if}
        <button class="btn ghost icon" title="Sign out" aria-label="Sign out" onclick={() => remove(a.id)}><LogOut size={16} /></button>
      </div>
    {:else}
      <div class="empty glass">
        <UserPlus size={28} />
        <h3>No accounts yet</h3>
        <p>Sign in with the Microsoft account that owns Minecraft: Java Edition.</p>
      </div>
    {/each}
  </div>

  {#if app.info && !app.info.msConfigured}
    <div class="note glass">
      <Info size={18} />
      <div>
        <b>Microsoft sign-in isn't configured in this build.</b>
        <p>
          Register an app in Azure, request Minecraft API access from Mojang, then build with the
          <code>PEAK_MS_CLIENT_ID</code> environment variable set to your client ID.
        </p>
      </div>
    </div>
  {/if}

  {#if app.info && !app.info.tokensEncrypted && app.accounts.some((a) => a.kind === 'microsoft')}
    <div class="note glass">
      <Info size={18} />
      <div>
        <b>Sign-in details aren't encrypted on this computer.</b>
        <p>
          Peak couldn't reach your system's password store (on Linux, GNOME Keyring or KWallet), so your sign-in is
          saved in Peak's data folder without encryption. Installing or unlocking a password store fixes this the next
          time Peak starts.
        </p>
      </div>
    </div>
  {/if}

  {#if app.info?.devBuild}
    <div class="dev glass">
      <div>
        <span class="label">Developer</span>
        <p>Offline accounts are for testing and only exist in development builds.</p>
      </div>
      <div class="dev-form">
        <input class="input" bind:value={offlineName} placeholder="Username" maxlength="16" onkeydown={(e) => e.key === 'Enter' && addOffline()} />
        <button class="btn" onclick={addOffline} disabled={offlineName.trim().length < 3}>Add offline</button>
      </div>
    </div>
  {/if}
</div>

{#if login}
  <MicrosoftSignIn start={login} oncancel={cancelSignIn} />
{/if}

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 20px;
    max-width: 860px;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 16px;
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
  .list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .account {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 16px;
  }
  .account.active {
    border-color: color-mix(in srgb, var(--accent) 40%, transparent);
    background: linear-gradient(90deg, color-mix(in srgb, var(--accent) 12%, transparent), var(--glass));
  }
  .avatar {
    width: 44px;
    height: 44px;
    border-radius: 13px;
    display: grid;
    place-items: center;
    font-weight: 720;
    font-size: 17px;
    background: var(--gradient);
    color: var(--on-accent);
  }
  .text {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .text b {
    font-size: 15px;
    font-weight: 620;
  }
  .text span {
    font-size: 12.5px;
    color: var(--faint);
  }
  .active-chip {
    color: #a7f3d0;
    background: rgba(16, 185, 129, 0.15);
    border-color: rgba(52, 211, 153, 0.3);
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 48px;
    color: var(--faint);
    text-align: center;
  }
  .empty h3 {
    color: var(--text);
    font-size: 16px;
    margin-top: 4px;
  }
  .empty p {
    margin: 0;
    font-size: 13px;
  }
  .note {
    display: flex;
    gap: 14px;
    padding: 16px 18px;
    color: var(--accent-2);
    background: color-mix(in srgb, var(--accent-2) 6%, transparent);
    border-color: color-mix(in srgb, var(--accent-2) 22%, transparent);
  }
  .note b {
    color: var(--text);
    font-size: 13.5px;
  }
  .note p {
    margin: 4px 0 0;
    color: var(--muted);
    font-size: 13px;
    line-height: 1.5;
  }
  code {
    font-size: 12px;
    padding: 1px 6px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.08);
    color: var(--text);
  }
  .dev {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 20px;
    padding: 16px 18px;
    border-style: dashed;
  }
  .dev p {
    margin: 4px 0 0;
    font-size: 12.5px;
    color: var(--faint);
  }
  .dev-form {
    display: flex;
    gap: 8px;
    width: 340px;
  }
</style>
