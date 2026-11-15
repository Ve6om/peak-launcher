<script lang="ts">
  let { checked = $bindable(false), disabled = false, label = '' }: { checked?: boolean; disabled?: boolean; label?: string } = $props();
</script>

<label class="switch" class:disabled>
  <input type="checkbox" bind:checked {disabled} aria-label={label} />
  <span class="knob"></span>
</label>

<style>
  .switch {
    position: relative;
    display: inline-block;
    width: 42px;
    height: 24px;
    flex-shrink: 0;
  }
  .disabled {
    opacity: 0.45;
  }
  input {
    position: absolute;
    inset: 0;
    opacity: 0;
    margin: 0;
    cursor: pointer;
    z-index: 1;
  }
  .knob {
    position: absolute;
    inset: 0;
    border-radius: 99px;
    background: rgba(255, 255, 255, 0.12);
    transition: background 0.25s;
  }
  .knob::after {
    content: '';
    position: absolute;
    top: 3px;
    left: 3px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: white;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.3);
    transition: transform 0.25s var(--ease);
  }
  input:checked + .knob {
    background: var(--gradient);
  }
  input:checked + .knob::after {
    transform: translateX(18px);
  }
  input:focus-visible + .knob {
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 40%, transparent);
  }
</style>
