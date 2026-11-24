<script lang="ts">
  // Asked once, the first time someone presses Play: which Java build should
  // Peak download for their games? Saved to Settings and remembered.
  import { Coffee } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import JavaVendorPicker from './JavaVendorPicker.svelte';
  import { api } from '$lib/api';
  import { app, toast, errorText } from '$lib/state.svelte';
  import type { JavaVendor } from '$lib/types';

  let pick = $state<JavaVendor>('graalvm');
  let saving = $state(false);

  function finish(go: boolean) {
    const resolve = app.javaPrompt;
    app.javaPrompt = null;
    resolve?.(go);
  }

  async function confirm() {
    if (!app.settings) return finish(true);
    saving = true;
    try {
      app.settings = await api.saveSettings({ ...$state.snapshot(app.settings), javaVendor: pick, javaVendorChosen: true });
      finish(true);
    } catch (e) {
      toast(errorText(e), 'error');
      saving = false;
    }
  }
</script>

<Modal title="Which Java should Peak use?" subtitle="Minecraft runs on Java. Peak downloads the right version for each game; pick whose." width={640} onclose={() => finish(false)}>
  <JavaVendorPicker bind:value={pick} />
  <p class="note">
    You can change this any time in Settings → Java & memory, or per instance. With GraalVM, Minecraft older than 1.17 still uses Mojang's Java, since GraalVM doesn't exist for it.
  </p>
  <div class="actions">
    <button class="btn primary" disabled={saving} onclick={confirm}><Coffee size={15} /> Use this and play</button>
  </div>
</Modal>

<style>
  .note {
    margin: 12px 0 0;
    font-size: 12px;
    line-height: 1.45;
    color: var(--muted);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    margin-top: 14px;
  }
</style>
