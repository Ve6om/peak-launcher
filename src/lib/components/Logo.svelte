<script lang="ts">
  let { size = 22 }: { size?: number } = $props();
</script>

<img src="/logo.png" width={size} height={size} alt="" draggable="false" />

<style>
  img {
    display: block;
    flex-shrink: 0;
    filter: drop-shadow(0 2px 8px rgba(80, 120, 255, 0.35));
  }
</style>
