<script setup lang="ts">
import type { Component } from 'vue'

/**
 * Liste générique pour les suggestions de l'éditeur (/ commandes, @ mentions, : émojis).
 * Pilotée au clavier via `onKeyDown` exposé (appelé par le plugin Suggestion de TipTap).
 */
export interface SuggestionItem {
  id: string
  label: string
  hint?: string
  icon?: Component
  emoji?: string
  avatar?: { name: string; src?: string | null }
  group?: string
  shortcut?: string
  [k: string]: any
}

const props = defineProps<{ items: SuggestionItem[]; command: (item: SuggestionItem) => void; emptyLabel?: string }>()
const selected = ref(0)
const root = ref<HTMLElement | null>(null)

watch(() => props.items, () => (selected.value = 0))

function scroll() {
  nextTick(() => root.value?.querySelector('.sg-item.active')?.scrollIntoView({ block: 'nearest' }))
}

function onKeyDown({ event }: { event: KeyboardEvent }) {
  const n = props.items.length
  if (!n) return false
  if (event.key === 'ArrowDown') { selected.value = (selected.value + 1) % n; scroll(); return true }
  if (event.key === 'ArrowUp') { selected.value = (selected.value - 1 + n) % n; scroll(); return true }
  if (event.key === 'Enter' || event.key === 'Tab') { select(selected.value); return true }
  return false
}
function select(i: number) {
  const it = props.items[i]
  if (it) props.command(it)
}

const grouped = computed(() => {
  const out: { group?: string; items: (SuggestionItem & { index: number })[] }[] = []
  props.items.forEach((it, index) => {
    const last = out[out.length - 1]
    if (last && last.group === it.group) last.items.push({ ...it, index })
    else out.push({ group: it.group, items: [{ ...it, index }] })
  })
  return out
})

defineExpose({ onKeyDown })
</script>

<template>
  <div ref="root" class="sg" data-editor-suggestion-open>
    <template v-if="items.length">
      <template v-for="(g, gi) in grouped" :key="gi">
        <div v-if="g.group" class="sg-group">{{ g.group }}</div>
        <button
          v-for="it in g.items"
          :key="it.id"
          class="sg-item"
          :class="{ active: it.index === selected }"
          @mousemove="selected = it.index"
          @mousedown.prevent="select(it.index)"
        >
          <UiAvatar v-if="it.avatar" :name="it.avatar.name" :src="it.avatar.src" :size="20" />
          <span v-else-if="it.emoji" class="sg-emoji">{{ it.emoji }}</span>
          <span v-else-if="it.icon" class="sg-icon"><component :is="it.icon" :size="15" /></span>
          <span class="sg-text">
            <span class="sg-label truncate">{{ it.label }}</span>
            <span v-if="it.hint" class="sg-hint truncate">{{ it.hint }}</span>
          </span>
          <span v-if="it.shortcut" class="sg-shortcut">{{ it.shortcut }}</span>
        </button>
      </template>
    </template>
    <div v-else class="sg-empty">{{ emptyLabel || $t('common.noResults') }}</div>
  </div>
</template>

<style scoped>
.sg {
  width: 300px;
  max-height: 340px;
  overflow-y: auto;
  padding: 4px;
  background: var(--bg-elevated);
  border-radius: var(--r-md);
  box-shadow: var(--shadow-lg);
}
.sg-group { font-size: var(--fs-xs); color: var(--text-3); padding: 8px 8px 4px; font-weight: 500; }
.sg-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  min-height: 34px;
  padding: 4px 8px;
  border-radius: var(--r-sm);
  text-align: left;
}
.sg-item.active { background: var(--bg-active); }
.sg-icon {
  width: 26px; height: 26px; flex: none; display: grid; place-items: center;
  border-radius: var(--r-sm); background: var(--bg-hover); color: var(--text-2);
  box-shadow: inset 0 0 0 1px var(--border);
}
.sg-emoji { width: 22px; text-align: center; font-size: 17px; flex: none; }
.sg-text { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.sg-label { font-size: var(--fs-md); }
.sg-hint { font-size: var(--fs-xs); color: var(--text-3); }
.sg-shortcut { font-size: var(--fs-xs); color: var(--text-3); font-family: var(--font-mono); }
.sg-empty { padding: 12px; color: var(--text-3); font-size: var(--fs-sm); text-align: center; }
</style>
