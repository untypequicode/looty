<script setup lang="ts">
import type { Completion } from '~/editor/latexInput'

defineProps<{ items: Completion[]; active: number }>()
const emit = defineEmits<{ pick: [c: Completion] }>()
</script>

<template>
  <ul v-if="items.length" class="ls" role="listbox" contenteditable="false">
    <li v-for="(c, i) in items" :key="c.insert" role="option" :aria-selected="i === active" :class="{ on: i === active }" @mousedown.prevent="emit('pick', c)">{{ c.label }}</li>
    <li class="ls-hint">Tab ↹</li>
  </ul>
</template>

<style scoped>
.ls { position: absolute; z-index: 40; top: calc(100% + 4px); left: 0; min-width: 180px; max-width: 320px; margin: 0; padding: 4px; list-style: none; background: var(--bg-elevated); border: 1px solid var(--border); border-radius: var(--r-md); box-shadow: var(--shadow-lg); font-family: var(--font-mono); font-size: 12.5px; text-align: left; }
.ls li { padding: 4px 8px; border-radius: var(--r-sm); cursor: pointer; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: var(--text); }
.ls li.on { background: var(--accent-soft); color: var(--accent-text, var(--text)); }
.ls .ls-hint { cursor: default; color: var(--text-4); font-size: 11px; padding-top: 2px; }
</style>
