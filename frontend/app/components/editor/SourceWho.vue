<script setup lang="ts">
/** Qui est dans ce bloc (formule, diagramme) : pastilles nommées aux couleurs des curseurs. */
defineProps<{ people: { key: string; name: string; color: string }[] }>()
</script>

<template>
  <span v-if="people.length" class="who" contenteditable="false">
    <i v-for="p in people" :key="p.key" class="who-chip" :style="{ background: p.color }">{{ p.name }}</i>
  </span>
</template>

<style scoped>
.who { position: absolute; right: 4px; top: -9px; z-index: 4; display: inline-flex; gap: 3px; pointer-events: none; }
.who-chip { padding: 0 6px; border-radius: 4px; color: #fff; font: 600 10px/15px var(--font-sans, system-ui, sans-serif); font-style: normal; white-space: nowrap; box-shadow: 0 1px 3px rgba(0, 0, 0, .25); }
</style>
