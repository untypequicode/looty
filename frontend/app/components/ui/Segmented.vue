<script setup lang="ts" generic="T extends string">
import type { Component } from 'vue'

const model = defineModel<T>({ required: true })
defineProps<{ options: { value: T; label?: string; icon?: Component; title?: string }[]; size?: 'sm' | 'md' }>()
</script>

<template>
  <div class="seg" :class="size ?? 'sm'" role="tablist">
    <button
      v-for="o in options"
      :key="o.value"
      role="tab"
      class="seg-btn"
      :class="{ active: model === o.value }"
      :aria-selected="model === o.value"
      :title="o.title ?? o.label"
      @click="model = o.value"
    >
      <component :is="o.icon" v-if="o.icon" :size="14" />
      <span v-if="o.label">{{ o.label }}</span>
    </button>
  </div>
</template>

<style scoped>
.seg {
  display: inline-flex;
  padding: 2px;
  gap: 2px;
  background: var(--bg-hover);
  border-radius: var(--r-sm);
  border: 1px solid var(--border);
}
.seg-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 22px;
  padding: 0 8px;
  border-radius: 4px;
  color: var(--text-2);
  font-size: var(--fs-sm);
  font-weight: 500;
  transition: background var(--dur-fast) var(--ease), color var(--dur-fast) var(--ease);
}
.seg.md .seg-btn { height: 26px; padding: 0 10px; }
.seg-btn:hover { color: var(--text); }
.seg-btn.active { background: var(--bg-elevated); color: var(--text); box-shadow: var(--shadow-sm), 0 0 0 1px var(--border); }
</style>
