<script setup lang="ts">
import { shallowRef, watch } from 'vue'

/**
 * Icône Lucide d'un élément « icon » (ADR 0023) : la bibliothèque est chargée à la demande, une seule
 * fois (elle ne pèse que sur les présentations qui contiennent des icônes).
 */
const props = defineProps<{ name: string; color: string; strokeWidth: number }>()
const comp = shallowRef<unknown>(null)
let lib: Promise<{ icons?: Record<string, unknown> }> | null = null
const load = () => (lib ??= import('lucide-vue-next') as Promise<{ icons?: Record<string, unknown> }>)
watch(() => props.name, async (n) => {
  comp.value = /^[A-Za-z0-9]{1,64}$/.test(n) ? ((await load()).icons?.[n] ?? null) : null
}, { immediate: true })
</script>

<template>
  <component :is="comp" v-if="comp" size="100%" :color="color" :stroke-width="strokeWidth" class="si" aria-hidden="true" />
</template>

<style scoped>
.si { width: 100%; height: 100%; display: block; }
</style>
