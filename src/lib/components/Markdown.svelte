<script lang="ts">
  // Renders a project's description (Markdown + HTML) safely. Links open in
  // the system browser instead of inside the launcher.
  import { marked } from 'marked';
  import DOMPurify from 'dompurify';
  import { api } from '$lib/api';

  let { source }: { source: string } = $props();
  const html = $derived(DOMPurify.sanitize(marked.parse(source ?? '', { async: false, gfm: true }) as string));

  function onclick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest('a');
    if (!a) return;
    e.preventDefault();
    const href = a.getAttribute('href');
    if (href && /^https?:\/\//.test(href)) api.openUrl(href);
  }
</script>

<!-- Link clicks bubble up here; links stay keyboard-focusable on their own. -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="md" role="presentation" {onclick}>{@html html}</div>

<style>
  .md {
    font-size: 13.5px;
    line-height: 1.65;
    color: #c9cee0;
    user-select: text;
    overflow-wrap: anywhere;
  }
  .md :global(h1),
  .md :global(h2),
  .md :global(h3) {
    color: var(--text);
    margin: 1.2em 0 0.5em;
    font-size: 1.15em;
  }
  .md :global(h1) {
    font-size: 1.35em;
  }
  .md :global(p) {
    margin: 0.6em 0;
  }
  .md :global(a) {
    color: var(--accent-2);
    text-decoration: none;
  }
  .md :global(a:hover) {
    text-decoration: underline;
  }
  .md :global(img) {
    max-width: 100%;
    height: auto;
    border-radius: 10px;
  }
  .md :global(code) {
    font-family: 'JetBrains Mono', Consolas, monospace;
    font-size: 0.9em;
    padding: 1px 5px;
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.07);
  }
  .md :global(pre) {
    padding: 12px;
    border-radius: 10px;
    background: rgba(0, 0, 0, 0.35);
    overflow-x: auto;
  }
  .md :global(blockquote) {
    margin: 0.8em 0;
    padding: 2px 14px;
    border-left: 3px solid color-mix(in srgb, var(--accent) 60%, transparent);
    color: var(--muted);
  }
  .md :global(ul),
  .md :global(ol) {
    padding-left: 1.4em;
  }
  .md :global(hr) {
    border: 0;
    height: 1px;
    background: var(--stroke);
  }
  .md :global(table) {
    border-collapse: collapse;
  }
  .md :global(td),
  .md :global(th) {
    border: 1px solid var(--stroke);
    padding: 4px 8px;
  }
</style>
