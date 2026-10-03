<script setup lang="ts">
import { useFloating, autoUpdate, offset, flip, shift, type Placement } from '@floating-ui/vue'

const props = withDefaults(defineProps<{ text?: string; shortcut?: string | string[]; placement?: Placement; delay?: number }>(), {
  placement: 'bottom',
  delay: 450,
})
const reference = ref<HTMLElement | null>(null)
const floating = ref<HTMLElement | null>(null)
const show = ref(false)
let timer: ReturnType<typeof setTimeout> | null = null

const { floatingStyles } = useFloating(reference, floating, {
  placement: computed(() => props.placement),
  whileElementsMounted: autoUpdate,
  transform: false,
  middleware: [offset(6), flip(), shift({ padding: 6 })],
})

function enter() {
  if (!props.text || window.matchMedia('(pointer: coarse)').matches) return
  timer = setTimeout(() => (show.value = true), props.delay)
}
function leave() {
  if (timer) clearTimeout(timer)
  show.value = false
}
</script>

<template>
  <span ref="reference" class="tt-ref" @mouseenter="enter" @mouseleave="leave" @mousedown="leave" @focusin="enter" @focusout="leave">
    <slot />
  </span>
  <Teleport to="body">
    <Transition name="fade">
      <div v-if="show" ref="floating" class="tooltip" :style="floatingStyles" role="tooltip">
        <span>{{ text }}</span>
        <UiKbd v-if="shortcut" :keys="shortcut" />
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.tt-ref { display: inline-flex; }
.tooltip {
  z-index: 3000;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 8px;
  font-size: var(--fs-sm);
  color: var(--text);
  background: var(--bg-elevated);
  border-radius: var(--r-sm);
  box-shadow: var(--shadow-md);
  pointer-events: none;
  white-space: nowrap;
}
</style>
