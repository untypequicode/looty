<script setup lang="ts">
import { X } from 'lucide-vue-next'
import { motion } from '~/composables/useMotion'
import { useLayer } from '~/composables/useLayer'
import { useBreakpoint } from '~/composables/useBreakpoint'

const open = defineModel<boolean>('open', { default: false })
const props = withDefaults(defineProps<{
  title?: string
  description?: string
  width?: number
  closable?: boolean
  /** Position haute (palette de commandes) plutôt que centrée. */
  top?: boolean
}>(), { width: 460, closable: true })
const emit = defineEmits<{ (e: 'close'): void }>()

function close() {
  open.value = false
  emit('close')
}
useLayer(open, close, { modal: true })
const { isMobile } = useBreakpoint()
const panelMotion = computed(() => (isMobile.value && !props.top ? motion.sheet : motion.dialog))

// Bloque le défilement de la page derrière la modale.
watch(open, (v) => { document.body.style.overflow = v ? 'hidden' : '' })
onBeforeUnmount(() => { document.body.style.overflow = '' })
</script>

<template>
  <Teleport to="body">
    <Transition :css="false" @enter="motion.overlay.onEnter" @leave="motion.overlay.onLeave">
      <div v-if="open" class="dialog-overlay" @mousedown.self="closable && close()" />
    </Transition>
    <div v-if="open" class="dialog-wrap" :class="{ top, mobile: isMobile && !top }" @mousedown.self="closable && close()">
      <Transition appear :css="false" @enter="panelMotion.onEnter" @leave="panelMotion.onLeave">
        <div class="dialog" :style="{ width: `${width}px` }" role="dialog" aria-modal="true">
          <header v-if="title || closable" class="dialog-header">
            <div>
              <h2 v-if="title" class="dialog-title">{{ title }}</h2>
              <p v-if="description" class="dialog-desc">{{ description }}</p>
            </div>
            <UiButton v-if="closable" variant="ghost" size="sm" :icon="X" @click="close" />
          </header>
          <div class="dialog-body"><slot :close="close" /></div>
          <footer v-if="$slots.footer" class="dialog-footer"><slot name="footer" :close="close" /></footer>
        </div>
      </Transition>
    </div>
  </Teleport>
</template>

<style scoped>
.dialog-overlay {
  position: fixed;
  inset: 0;
  z-index: 900;
  background: var(--bg-overlay);
  backdrop-filter: blur(2px);
}
.dialog-wrap {
  position: fixed;
  inset: 0;
  z-index: 901;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
}
.dialog-wrap.top { align-items: flex-start; padding-top: 12vh; }
.dialog {
  max-width: 100%;
  max-height: calc(100vh - 32px);
  display: flex;
  flex-direction: column;
  background: var(--bg-elevated);
  border-radius: var(--r-lg);
  box-shadow: var(--shadow-lg);
  overflow: hidden;
}
.dialog-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 16px 16px 4px 20px;
}
.dialog-title { margin: 0; font-size: var(--fs-lg); font-weight: 600; letter-spacing: -0.01em; }
.dialog-desc { margin: 4px 0 0; color: var(--text-2); font-size: var(--fs-md); }
.dialog-body { padding: 12px 20px 20px; overflow: auto; }
.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid var(--border);
  background: var(--bg-panel);
}
.dialog-wrap.mobile { align-items: flex-end; padding: 0; }
.dialog-wrap.mobile .dialog {
  width: 100% !important;
  border-radius: 14px 14px 0 0;
  max-height: 92vh;
  padding-bottom: var(--safe-bottom);
}
</style>
