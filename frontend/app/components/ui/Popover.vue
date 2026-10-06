<script lang="ts">
let popoverSeq = 0
</script>

<script setup lang="ts">
import { useFloating, autoUpdate, offset, flip, shift, size as sizeMw, type Placement } from '@floating-ui/vue'
import { onClickOutside } from '@vueuse/core'
import { useLayer } from '~/composables/useLayer'
import { motion } from '~/composables/useMotion'
import { useBreakpoint } from '~/composables/useBreakpoint'

const open = defineModel<boolean>('open', { default: false })
const props = withDefaults(defineProps<{
  placement?: Placement
  offset?: number
  width?: number | 'trigger'
  sheetOnMobile?: boolean
  disabled?: boolean
  /** Élément de référence externe (ex. menu contextuel au clic droit). */
  virtualRect?: { x: number; y: number } | null
}>(), { placement: 'bottom-start', offset: 6, sheetOnMobile: true })
const emit = defineEmits<{ (e: 'close'): void }>()

const reference = ref<HTMLElement | null>(null)
const floating = ref<HTMLElement | null>(null)
const { isMobile } = useBreakpoint()
const asSheet = computed(() => props.sheetOnMobile && isMobile.value)

const virtualEl = computed(() => props.virtualRect
  ? { getBoundingClientRect: () => ({ x: props.virtualRect!.x, y: props.virtualRect!.y, width: 0, height: 0, top: props.virtualRect!.y, left: props.virtualRect!.x, right: props.virtualRect!.x, bottom: props.virtualRect!.y }) }
  : null)

const { floatingStyles } = useFloating(computed(() => (virtualEl.value as any) ?? reference.value), floating, {
  placement: computed(() => props.placement),
  open,
  // left/top plutôt que transform : les animations GSAP manipulent transform.
  transform: false,
  whileElementsMounted: autoUpdate,
  middleware: computed(() => [
    offset(props.offset),
    flip({ padding: 8 }),
    shift({ padding: 8 }),
    sizeMw({
      padding: 8,
      apply({ availableHeight, rects, elements }) {
        Object.assign(elements.floating.style, {
          maxHeight: `${Math.max(160, Math.min(availableHeight, 480))}px`,
          ...(props.width === 'trigger' ? { width: `${rects.reference.width}px` } : {}),
        })
      },
    }),
  ]),
})

function toggle() {
  if (props.disabled) return
  open.value = !open.value
}
function close() {
  if (!open.value) return
  open.value = false
  emit('close')
}

// Popovers imbriqués (liste déroulante ouverte depuis un popover) : un clic dans un calque
// ouvert après celui-ci ne le ferme pas.
const layer = ref(0)
watch(open, (o) => { if (o) layer.value = ++popoverSeq })
onClickOutside(floating, (e) => {
  if (asSheet.value) return
  if (reference.value?.contains(e.target as Node)) return
  const other = (e.target as HTMLElement | null)?.closest?.('[data-popover-layer]') as HTMLElement | null
  if (other && Number(other.dataset.popoverLayer) > layer.value) return
  close()
})
useLayer(open, close)

defineExpose({ close, toggle, reference })
</script>

<template>
  <span v-if="$slots.trigger" ref="reference" class="popover-ref" @click="toggle">
    <slot name="trigger" :open="open" :toggle="toggle" />
  </span>
  <Teleport to="body">
    <template v-if="asSheet">
      <Transition :css="false" @enter="motion.overlay.onEnter" @leave="motion.overlay.onLeave">
        <div v-if="open" class="sheet-overlay" @click="close" />
      </Transition>
      <Transition :css="false" @enter="motion.sheet.onEnter" @leave="motion.sheet.onLeave">
        <div v-if="open" ref="floating" class="sheet" role="dialog" :data-popover-layer="layer">
          <div class="sheet-handle" />
          <slot :close="close" />
        </div>
      </Transition>
    </template>
    <Transition v-else :css="false" @enter="motion.pop.onEnter" @leave="motion.pop.onLeave">
      <div
        v-if="open"
        ref="floating"
        class="popover"
        :data-popover-layer="layer"
        :style="[floatingStyles, typeof width === 'number' ? { width: `${width}px` } : {}]"
        role="dialog"
      >
        <slot :close="close" />
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.popover-ref { display: inline-flex; min-width: 0; }
.popover {
  z-index: 1000;
  min-width: 180px;
  background: var(--bg-elevated);
  border-radius: var(--r-md);
  box-shadow: var(--shadow-lg);
  overflow: auto;
  transform-origin: top left;
}
.sheet-overlay {
  position: fixed;
  inset: 0;
  z-index: 999;
  background: var(--bg-overlay);
}
.sheet {
  position: fixed;
  z-index: 1000;
  left: 0;
  right: 0;
  bottom: 0;
  max-height: 78vh;
  overflow: auto;
  background: var(--bg-elevated);
  border-radius: 14px 14px 0 0;
  box-shadow: var(--shadow-lg);
  padding-bottom: calc(8px + var(--safe-bottom));
}
.sheet-handle {
  width: 36px;
  height: 4px;
  border-radius: 4px;
  background: var(--border-strong);
  margin: 8px auto 4px;
}
</style>
