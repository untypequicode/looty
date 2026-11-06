<script setup lang="ts">
import { X, ZoomIn, ZoomOut, Maximize } from 'lucide-vue-next'
import { useLayer } from '~/composables/useLayer'
import { motion } from '~/composables/useMotion'

/** Diagramme en plein écran : molette pour zoomer, glisser pour se déplacer. */
const open = defineModel<boolean>('open', { default: false })
defineProps<{ svg: string; title?: string; background?: string }>()

const stage = ref<HTMLElement | null>(null)
const content = ref<HTMLElement | null>(null)
const scale = ref(1)
const x = ref(0)
const y = ref(0)

function close() { open.value = false }
useLayer(open, close, { modal: true })

/** Ajuste le diagramme à l'écran (sans l'agrandir au-delà de 4×). */
function fit() {
  const s = stage.value, c = content.value?.querySelector('svg')
  if (!s || !c) return
  const box = c.viewBox?.baseVal
  const w = box?.width || c.getBoundingClientRect().width / scale.value
  const h = box?.height || c.getBoundingClientRect().height / scale.value
  c.setAttribute('width', String(w)); c.setAttribute('height', String(h))
  c.style.maxWidth = 'none'
  const k = Math.min(4, (s.clientWidth - 48) / w, (s.clientHeight - 48) / h)
  scale.value = k
  x.value = (s.clientWidth - w * k) / 2
  y.value = (s.clientHeight - h * k) / 2
}
watch(open, (v) => { if (v) nextTick(() => requestAnimationFrame(fit)) })

function zoomAt(f: number, cx: number, cy: number) {
  const k = Math.min(20, Math.max(0.1, scale.value * f))
  x.value = cx - ((cx - x.value) * k) / scale.value
  y.value = cy - ((cy - y.value) * k) / scale.value
  scale.value = k
}
function zoomCenter(f: number) {
  const s = stage.value
  if (s) zoomAt(f, s.clientWidth / 2, s.clientHeight / 2)
}
function onWheel(e: WheelEvent) {
  const r = stage.value!.getBoundingClientRect()
  zoomAt(Math.exp(-e.deltaY * 0.0015), e.clientX - r.left, e.clientY - r.top)
}

let drag: { px: number; py: number; x: number; y: number } | null = null
function onDown(e: PointerEvent) {
  drag = { px: e.clientX, py: e.clientY, x: x.value, y: y.value }
  ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
}
function onMove(e: PointerEvent) {
  if (!drag) return
  x.value = drag.x + e.clientX - drag.px
  y.value = drag.y + e.clientY - drag.py
}
function onUp() { drag = null }
</script>

<template>
  <Teleport to="body">
    <Transition :css="false" @enter="motion.dialog.onEnter" @leave="motion.dialog.onLeave">
      <div v-if="open" class="dv" role="dialog" aria-modal="true">
        <div class="dv-bar">
          <span class="dv-title">{{ title }}</span>
          <span class="spacer" />
          <UiButton size="sm" variant="ghost" :icon="ZoomOut" :title="$t('viewer.zoomOut')" @click="zoomCenter(1 / 1.25)" />
          <span class="dv-pct">{{ Math.round(scale * 100) }} %</span>
          <UiButton size="sm" variant="ghost" :icon="ZoomIn" :title="$t('viewer.zoomIn')" @click="zoomCenter(1.25)" />
          <UiButton size="sm" variant="ghost" :icon="Maximize" :title="$t('diagrams.fit')" @click="fit" />
          <UiButton size="sm" variant="ghost" :icon="X" :title="$t('common.close')" @click="close" />
        </div>
        <div ref="stage" class="dv-stage" :style="background ? { background } : undefined" @wheel.prevent="onWheel" @pointerdown="onDown" @pointermove="onMove" @pointerup="onUp" @pointercancel="onUp" @dblclick="fit">
          <div ref="content" class="dv-content" :style="{ transform: `translate(${x}px, ${y}px) scale(${scale})` }" v-html="svg" />
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.dv { position: fixed; inset: 0; z-index: 2500; display: flex; flex-direction: column; background: var(--bg); }
.dv-bar { display: flex; align-items: center; gap: 4px; height: 44px; padding: 0 10px 0 16px; border-bottom: 1px solid var(--border); background: var(--bg-panel); }
.dv-title { font-size: var(--fs-sm); font-weight: 600; color: var(--text-2); }
.dv-pct { min-width: 48px; text-align: center; font-size: var(--fs-xs); color: var(--text-3); font-variant-numeric: tabular-nums; }
.dv-stage { position: relative; flex: 1; overflow: hidden; cursor: grab; touch-action: none; }
.dv-stage:active { cursor: grabbing; }
.dv-content { position: absolute; left: 0; top: 0; transform-origin: 0 0; }
.dv-content :deep(svg) { display: block; }
</style>
