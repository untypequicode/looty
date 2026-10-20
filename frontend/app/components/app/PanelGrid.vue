<script setup lang="ts" generic="T extends GridBox">
import { GridLayout, GridItem } from 'grid-layout-plus'
import { useElementSize } from '@vueuse/core'
import { GRID_COLS, type GridBox } from '~/composables/usePanelGrid'

/**
 * Grille de panneaux (12 colonnes) : on déplace un panneau par son en-tête (`.pg-handle`),
 * on le redimensionne par le coin ; les autres se poussent. Chaque changement terminé est
 * signalé par `change` (enregistrement automatique côté parent). Quand la place manque,
 * les panneaux s'empilent dans l'ordre de lecture, sans modifier la disposition.
 */
const props = withDefaults(defineProps<{
  items: T[]
  rowHeight?: number
  gap?: number
  minW?: number
  minH?: number
  /** Largeur (px) en dessous de laquelle les panneaux s'empilent. */
  stackBelow?: number
}>(), { rowHeight: 28, gap: 14, minW: 3, minH: 4, stackBelow: 720 })
const emit = defineEmits<{ (e: 'change', boxes: GridBox[]): void }>()
defineSlots<{ default(p: { item: T; stacked: boolean }): any }>()

const el = ref<HTMLElement | null>(null)
const { width } = useElementSize(el)
const stacked = computed(() => width.value > 0 && width.value < props.stackBelow)

// Copie locale : la grille modifie les positions en place pendant le glissement.
const layout = ref<GridBox[]>([])
watch(() => props.items, (v) => { layout.value = v.map(({ i, x, y, w, h }) => ({ i, x, y, w, h })) }, { immediate: true, deep: true })
const byId = computed(() => new Map(props.items.map((x) => [x.i, x])))

const key = (b: GridBox[]) => b.map(({ i, x, y, w, h }) => `${i}:${x},${y},${w},${h}`).sort().join('|')
function onUpdated() {
  const boxes = layout.value.map(({ i, x, y, w, h }) => ({ i, x, y, w, h }))
  if (key(boxes) !== key(props.items)) emit('change', boxes)
}

const reading = computed(() => [...props.items].sort((a, b) => a.y - b.y || a.x - b.x))
const px = (h: number) => `${h * props.rowHeight + (h - 1) * props.gap}px`
</script>

<template>
  <div ref="el" class="pg" :style="{ '--pg-gap': `${gap}px` }">
    <div v-if="stacked" class="pg-stack">
      <div v-for="it in reading" :key="it.i" class="pg-cell" :style="{ maxHeight: px(it.h) }">
        <slot :item="it" :stacked="true" />
      </div>
    </div>
    <GridLayout
      v-else-if="width > 0"
      v-model:layout="layout"
      :col-num="GRID_COLS"
      :row-height="rowHeight"
      :margin="[gap, gap]"
      :is-draggable="true"
      :is-resizable="true"
      :vertical-compact="true"
      :use-css-transforms="true"
      class="pg-grid"
      @layout-updated="onUpdated"
    >
      <GridItem
        v-for="b in layout"
        :key="b.i"
        :i="b.i" :x="b.x" :y="b.y" :w="b.w" :h="b.h"
        :min-w="minW" :min-h="minH"
        drag-allow-from=".pg-handle"
        drag-ignore-from="button, a, input, textarea, select, [contenteditable]"
        class="pg-cell"
      >
        <slot v-if="byId.get(b.i)" :item="byId.get(b.i)!" :stacked="false" />
      </GridItem>
    </GridLayout>
  </div>
</template>

<style scoped>
.pg { min-width: 0; }
.pg-stack { display: flex; flex-direction: column; gap: var(--pg-gap); }
.pg-cell { min-width: 0; display: flex; }
.pg-cell > :deep(*) { flex: 1; min-width: 0; }
.pg-grid {
  margin: calc(var(--pg-gap) * -1);
  --vgl-placeholder-bg: var(--accent);
  --vgl-placeholder-opacity: 14%;
  --vgl-resizer-size: 14px;
  --vgl-resizer-border-color: var(--text-3);
  --vgl-resizer-border-width: 2px;
  --vgl-item-resizing-opacity: 85%;
}
.pg-grid :deep(.vgl-item--placeholder) { border-radius: var(--r-lg); }
.pg-grid :deep(.vgl-item__resizer) { opacity: 0; transition: opacity var(--dur-fast); right: 3px; bottom: 3px; }
.pg-grid :deep(.vgl-item:hover .vgl-item__resizer), .pg-grid :deep(.vgl-item--resizing .vgl-item__resizer) { opacity: 0.7; }
.pg-grid :deep(.vgl-item--dragging) { box-shadow: var(--shadow-pop, 0 12px 32px rgb(0 0 0 / 18%)); border-radius: var(--r-lg); }
@media (pointer: coarse) { .pg-grid :deep(.vgl-item__resizer) { opacity: 0.6; } }
</style>
