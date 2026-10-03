<script setup lang="ts">
import type { Component } from 'vue'
const NuxtLink = resolveComponent('NuxtLink')

const props = defineProps<{
  to?: string
  icon?: Component
  emoji?: string | null
  label: string
  badge?: number | string
  active?: boolean
  depth?: number
  exact?: boolean
}>()
const route = useRoute()
const isActive = computed(() => {
  if (props.active !== undefined) return props.active
  if (!props.to) return false
  const path = props.to.split('?')[0]
  return props.exact ? route.path === path : route.path === path || route.path.startsWith(`${path}/`)
})
</script>

<template>
  <component
    :is="to ? NuxtLink : 'button'"
    :to="to"
    class="sb-item"
    :class="{ active: isActive }"
    :style="depth ? { paddingLeft: `${8 + depth * 14}px` } : undefined"
  >
    <slot name="lead" />
    <span v-if="emoji" class="sb-emoji">{{ emoji }}</span>
    <component :is="icon" v-else-if="icon" :size="15" :stroke-width="1.9" class="sb-icon" />
    <span class="sb-label truncate">{{ label }}</span>
    <span v-if="badge" class="sb-badge">{{ badge }}</span>
    <span v-if="$slots.actions" class="sb-actions" @click.prevent.stop><slot name="actions" /></span>
  </component>
</template>

<style scoped>
.sb-item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  height: 28px;
  padding: 0 8px;
  border-radius: var(--r-sm);
  color: var(--text-2);
  font-size: var(--fs-md);
  font-weight: 500;
  text-align: left;
  transition: background var(--dur-fast) var(--ease), color var(--dur-fast) var(--ease);
}
.sb-item:hover { background: var(--bg-hover); color: var(--text); }
.sb-item.active { background: var(--bg-active); color: var(--text); }
.sb-icon { flex: none; color: var(--text-3); }
.sb-item.active .sb-icon, .sb-item:hover .sb-icon { color: var(--text-2); }
.sb-emoji { width: 16px; text-align: center; font-size: 13px; flex: none; line-height: 1; }
.sb-label { flex: 1; min-width: 0; }
.sb-badge {
  min-width: 18px;
  height: 18px;
  padding: 0 5px;
  border-radius: var(--r-full);
  background: var(--accent);
  color: #fff;
  font-size: 10.5px;
  font-weight: 600;
  display: grid;
  place-items: center;
}
.sb-actions { display: none; align-items: center; gap: 2px; margin-right: -4px; }
.sb-item:hover .sb-actions, .sb-item:focus-within .sb-actions { display: flex; }
@media (pointer: coarse) {
  .sb-item { height: 38px; font-size: var(--fs-lg); }
  .sb-actions { display: flex; }
}
</style>
