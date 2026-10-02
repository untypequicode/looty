<script setup lang="ts">
import type { Component } from 'vue'

const props = withDefaults(defineProps<{
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger' | 'subtle'
  size?: 'xs' | 'sm' | 'md' | 'lg'
  icon?: Component
  iconRight?: Component
  loading?: boolean
  disabled?: boolean
  block?: boolean
  type?: 'button' | 'submit'
  to?: string
  square?: boolean
}>(), { variant: 'secondary', size: 'md', type: 'button' })

const NuxtLink = resolveComponent('NuxtLink')
const tag = computed(() => (props.to ? NuxtLink : 'button'))
const iconSize = computed(() => ({ xs: 12, sm: 14, md: 15, lg: 16 })[props.size])
</script>

<template>
  <component
    :is="tag"
    :to="to"
    :type="to ? undefined : type"
    class="btn"
    :class="[`btn-${variant}`, `btn-${size}`, { block, square: square || (!$slots.default && icon), loading }]"
    :disabled="disabled || loading"
  >
    <span v-if="loading" class="btn-spinner" />
    <component :is="icon" v-else-if="icon" :size="iconSize" :stroke-width="2" class="btn-icon" />
    <span v-if="$slots.default" class="btn-label"><slot /></span>
    <component :is="iconRight" v-if="iconRight" :size="iconSize" :stroke-width="2" class="btn-icon" />
  </component>
</template>

<style scoped>
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  font-weight: 500;
  border-radius: var(--r-sm);
  border: 1px solid transparent;
  white-space: nowrap;
  user-select: none;
  transition: background var(--dur-fast) var(--ease), border-color var(--dur-fast) var(--ease),
    color var(--dur-fast) var(--ease), transform var(--dur-fast) var(--ease), box-shadow var(--dur-fast) var(--ease);
}
.btn:active:not(:disabled) { transform: scale(0.98); }
.btn:disabled { opacity: 0.5; cursor: not-allowed; }
.btn.block { width: 100%; }

.btn-xs { height: 22px; padding: 0 6px; font-size: var(--fs-xs); border-radius: var(--r-xs); }
.btn-sm { height: 26px; padding: 0 9px; font-size: var(--fs-sm); }
.btn-md { height: 30px; padding: 0 12px; font-size: var(--fs-md); }
.btn-lg { height: 36px; padding: 0 16px; font-size: var(--fs-lg); }
.btn.square.btn-xs { width: 22px; padding: 0; }
.btn.square.btn-sm { width: 26px; padding: 0; }
.btn.square.btn-md { width: 30px; padding: 0; }
.btn.square.btn-lg { width: 36px; padding: 0; }

.btn-primary {
  background: var(--accent);
  color: var(--text-on-accent);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.14), 0 1px 2px rgba(0, 0, 0, 0.2);
}
.btn-primary:hover:not(:disabled) { background: var(--accent-hover); }

.btn-secondary {
  background: var(--bg-elevated);
  border-color: var(--border-strong);
  color: var(--text);
  box-shadow: var(--shadow-sm);
}
.btn-secondary:hover:not(:disabled) { background: var(--bg-card-hover); border-color: var(--text-4); }

.btn-ghost { color: var(--text-2); }
.btn-ghost:hover:not(:disabled) { background: var(--bg-hover); color: var(--text); }

.btn-subtle { background: var(--bg-hover); color: var(--text); }
.btn-subtle:hover:not(:disabled) { background: var(--bg-active); }

.btn-danger { background: var(--red-soft); color: var(--red); border-color: color-mix(in srgb, var(--red) 25%, transparent); }
.btn-danger:hover:not(:disabled) { background: var(--red); color: #fff; }

.btn-icon { flex: none; }
.btn-label { overflow: hidden; text-overflow: ellipsis; }
.btn-spinner {
  width: 12px; height: 12px; border-radius: 50%;
  border: 1.5px solid currentColor; border-right-color: transparent;
  animation: spin 0.6s linear infinite;
}
</style>
