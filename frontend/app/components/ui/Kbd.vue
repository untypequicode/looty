<script setup lang="ts">
const props = defineProps<{ keys: string | string[] }>()
const isMac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform)
const map: Record<string, string> = isMac
  ? { mod: '⌘', shift: '⇧', alt: '⌥', enter: '↵', ctrl: '⌃', esc: 'Esc' }
  : { mod: 'Ctrl', shift: 'Shift', alt: 'Alt', enter: '↵', ctrl: 'Ctrl', esc: 'Esc' }
const list = computed(() => (Array.isArray(props.keys) ? props.keys : [props.keys]).map((k) => map[k.toLowerCase()] ?? k.toUpperCase()))
</script>

<template>
  <span class="kbd-group"><kbd v-for="(k, i) in list" :key="i" class="kbd">{{ k }}</kbd></span>
</template>

<style scoped>
.kbd-group { display: inline-flex; gap: 3px; }
@media (pointer: coarse) { .kbd-group { display: none; } }
.kbd {
  font-family: var(--font);
  font-size: 10.5px;
  font-weight: 500;
  min-width: 17px;
  height: 17px;
  padding: 0 4px;
  display: inline-grid;
  place-items: center;
  border-radius: 4px;
  color: var(--text-2);
  background: var(--bg-hover);
  border: 1px solid var(--border);
  line-height: 1;
}
</style>
