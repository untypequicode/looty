<script setup lang="ts">
defineProps<{ label?: string; hint?: string; stacked?: boolean }>()
</script>

<template>
  <div class="sr" :class="{ stacked }">
    <div class="sr-text">
      <div v-if="label" class="sr-label">{{ label }}</div>
      <div v-if="hint" class="sr-hint">{{ hint }}</div>
      <slot name="text" />
    </div>
    <div class="sr-control"><slot /></div>
  </div>
</template>

<style scoped>
.sr { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 14px 16px; min-height: 60px; }
.sr + .sr { border-top: 1px solid var(--border); }
.sr-text { min-width: 0; flex: 1; }
.sr-label { font-weight: 500; }
.sr-hint { color: var(--text-3); font-size: var(--fs-sm); margin-top: 2px; line-height: 1.45; }
.sr-control { flex: none; display: flex; align-items: center; gap: 8px; }
.sr.stacked { flex-direction: column; align-items: stretch; gap: 10px; }
.sr.stacked .sr-control { flex: 1; }
@media (max-width: 639px) {
  .sr { flex-direction: column; align-items: stretch; gap: 10px; }
  .sr-control { flex-wrap: wrap; }
}
</style>
