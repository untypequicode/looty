<script setup lang="ts">
/**
 * Cercle d'état façon Linear. `progress` (0..1) dessine un camembert partiel
 * selon la position de la colonne dans le flux ; `done` affiche la coche.
 */
const props = withDefaults(defineProps<{ progress?: number; done?: boolean; size?: number; partial?: boolean; backlog?: boolean }>(), {
  progress: 0,
  size: 16,
})
const r = 3.5
const circ = 2 * Math.PI * r
const dash = computed(() => `${Math.max(0, Math.min(1, props.progress)) * circ} ${circ}`)
</script>

<template>
  <svg :width="size" :height="size" viewBox="0 0 16 16" class="status" :class="{ done, partial }" aria-hidden="true">
    <template v-if="done">
      <circle cx="8" cy="8" r="7" fill="var(--accent)" />
      <path d="M5 8.2 7.1 10.2 11 6" fill="none" stroke="#fff" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" />
    </template>
    <template v-else>
      <circle cx="8" cy="8" r="6" fill="none" :stroke="partial ? 'var(--accent)' : 'currentColor'" stroke-width="1.5" :stroke-dasharray="backlog ? '1.6 1.9' : undefined" />
      <circle
        v-if="progress > 0"
        cx="8" cy="8" :r="r"
        fill="none"
        :stroke="partial ? 'var(--accent)' : 'currentColor'"
        :stroke-width="r * 2"
        :stroke-dasharray="dash"
        transform="rotate(-90 8 8)"
      />
    </template>
  </svg>
</template>

<style scoped>
.status { flex: none; color: var(--text-3); transition: color var(--dur-fast) var(--ease); }
</style>
