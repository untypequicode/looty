<script setup lang="ts">
import type { Component } from 'vue'
defineProps<{ icon?: Component; title: string; description?: string }>()
</script>

<template>
  <div class="empty">
    <div v-if="icon" class="empty-icon"><component :is="icon" :size="22" :stroke-width="1.6" /></div>
    <h3 class="empty-title">{{ title }}</h3>
    <p v-if="description" class="empty-desc">{{ description }}</p>
    <div v-if="$slots.default" class="empty-actions"><slot /></div>
  </div>
</template>

<style scoped>
.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  padding: 56px 24px;
  max-width: 380px;
  margin: 0 auto;
}
.empty-icon {
  width: 44px;
  height: 44px;
  display: grid;
  place-items: center;
  border-radius: var(--r-md);
  background: var(--bg-hover);
  color: var(--text-2);
  margin-bottom: 14px;
  box-shadow: inset 0 0 0 1px var(--border);
}
.empty-title { margin: 0; font-size: var(--fs-lg); font-weight: 600; }
.empty-desc { margin: 6px 0 0; color: var(--text-2); line-height: 1.55; }
.empty-actions { margin-top: 16px; display: flex; gap: 8px; }
</style>
