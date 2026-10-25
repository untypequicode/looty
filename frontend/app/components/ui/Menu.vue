<script setup lang="ts">
import type { Component } from 'vue'
import type { Placement } from '@floating-ui/vue'
import { Check } from 'lucide-vue-next'

export interface MenuItem {
  label?: string
  icon?: Component
  shortcut?: string | string[]
  danger?: boolean
  checked?: boolean
  disabled?: boolean
  divider?: boolean
  heading?: string
  onSelect?: () => void
}

const open = defineModel<boolean>('open', { default: false })
const props = withDefaults(defineProps<{
  items: MenuItem[]
  placement?: Placement
  width?: number
  virtualRect?: { x: number; y: number } | null
}>(), { placement: 'bottom-start', width: 220 })

const active = ref(-1)
const selectable = computed(() => props.items.map((it, i) => (!it.divider && !it.heading && !it.disabled ? i : -1)).filter((i) => i >= 0))

watch(open, (v) => { if (v) active.value = -1 })

function onKey(e: KeyboardEvent) {
  if (!open.value) return
  const list = selectable.value
  if (!list.length) return
  const pos = list.indexOf(active.value)
  if (e.key === 'ArrowDown') { e.preventDefault(); active.value = list[(pos + 1) % list.length]! }
  else if (e.key === 'ArrowUp') { e.preventDefault(); active.value = list[(pos - 1 + list.length) % list.length]! }
  else if (e.key === 'Enter' && active.value >= 0) { e.preventDefault(); select(props.items[active.value]!) }
}
useEventListener('keydown', onKey)

function select(it: MenuItem) {
  if (it.disabled) return
  open.value = false
  nextTick(() => it.onSelect?.())
}
</script>

<template>
  <UiPopover v-model:open="open" :placement="placement" :width="width" :virtual-rect="virtualRect">
    <template v-if="$slots.default" #trigger="{ open: o }"><slot :open="o" /></template>
    <div class="menu" role="menu">
      <template v-for="(it, i) in items" :key="i">
        <div v-if="it.divider" class="menu-divider" />
        <div v-else-if="it.heading" class="menu-heading">{{ it.heading }}</div>
        <button
          v-else
          class="menu-item"
          :class="{ danger: it.danger, active: active === i }"
          :disabled="it.disabled"
          role="menuitem"
          @mouseenter="active = i"
          @click="select(it)"
        >
          <component :is="it.icon" v-if="it.icon" :size="15" class="menu-icon" />
          <span class="menu-label truncate">{{ it.label }}</span>
          <Check v-if="it.checked" :size="14" class="menu-check" />
          <UiKbd v-else-if="it.shortcut" :keys="it.shortcut" />
        </button>
      </template>
    </div>
  </UiPopover>
</template>

<style scoped>
.menu { padding: 4px; display: flex; flex-direction: column; }
.menu-item {
  display: flex;
  align-items: center;
  gap: 9px;
  height: 30px;
  padding: 0 8px;
  border-radius: var(--r-sm);
  color: var(--text);
  text-align: left;
  font-size: var(--fs-md);
}
.menu-item.active { background: var(--bg-active); }
.menu-item:disabled { opacity: 0.45; cursor: default; }
.menu-item.danger { color: var(--red); }
.menu-icon { color: var(--text-2); flex: none; }
.menu-item.danger .menu-icon { color: var(--red); }
.menu-label { flex: 1; }
.menu-check { color: var(--accent); }
.menu-divider { height: 1px; background: var(--border); margin: 4px -4px; }
.menu-heading { font-size: var(--fs-xs); color: var(--text-3); padding: 6px 8px 2px; font-weight: 500; }
@media (pointer: coarse) {
  .menu-item { height: 42px; font-size: var(--fs-lg); }
}
</style>
