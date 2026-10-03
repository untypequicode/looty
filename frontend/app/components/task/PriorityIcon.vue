<script setup lang="ts">
/** Icône de priorité façon Linear : barres croissantes, « ! » pour urgent, tirets pour aucune. */
const props = withDefaults(defineProps<{ priority: number; size?: number }>(), { size: 16 })
const bars = computed(() => ({ 2: 3, 3: 2, 4: 1 } as Record<number, number>)[props.priority] ?? 0)
</script>

<template>
  <svg :width="size" :height="size" viewBox="0 0 16 16" class="prio" :class="`p${priority}`" aria-hidden="true">
    <template v-if="priority === 1">
      <rect x="1.5" y="1.5" width="13" height="13" rx="3.5" fill="var(--prio-urgent)" />
      <rect x="7.1" y="4" width="1.8" height="5.2" rx="0.9" fill="#fff" />
      <circle cx="8" cy="11.3" r="1" fill="#fff" />
    </template>
    <template v-else-if="priority === 0">
      <rect v-for="i in 3" :key="i" :x="1.5 + (i - 1) * 4.75" y="7.25" width="3.5" height="1.5" rx="0.75" fill="var(--text-3)" />
    </template>
    <template v-else>
      <rect x="1.5" y="9" width="3" height="5" rx="1" :fill="bars >= 1 ? 'currentColor' : 'var(--text-4)'" />
      <rect x="6.5" y="6" width="3" height="8" rx="1" :fill="bars >= 2 ? 'currentColor' : 'var(--text-4)'" />
      <rect x="11.5" y="3" width="3" height="11" rx="1" :fill="bars >= 3 ? 'currentColor' : 'var(--text-4)'" />
    </template>
  </svg>
</template>

<style scoped>
.prio { flex: none; color: var(--text-2); }
</style>
