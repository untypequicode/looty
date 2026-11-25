<script setup lang="ts">
import type * as Y from 'yjs'
import { fragmentHtml, slideColor } from '~/slides/render'
import { textOf, type Slide, type SlideElement } from '~/slides/model'

/**
 * Une diapositive rendue en DOM et SVG (ADR 0022) : une boîte de taille logique (1920 x 1080) mise
 * à l'échelle de son conteneur. Sert au canevas, aux miniatures et plus tard au mode présentation.
 * Le slot par défaut est dessiné dans le repère logique (sélection, guides) ; `editor` remplace le
 * texte d'un élément en cours d'édition.
 */
const props = defineProps<{ doc: Y.Doc; slide: Slide; size: { w: number; h: number }; tick: number; editingId?: string | null }>()
const root = ref<HTMLElement | null>(null)
const scale = ref(1)
let ro: ResizeObserver | null = null
onMounted(() => {
  const measure = () => { if (root.value && props.size.w) scale.value = root.value.clientWidth / props.size.w }
  measure()
  ro = new ResizeObserver(measure)
  if (root.value) ro.observe(root.value)
})
onBeforeUnmount(() => ro?.disconnect())
defineExpose({ root, scale })

function html(el: SlideElement): string {
  void props.tick
  const f = textOf(props.doc, props.slide.id, el.id)
  return f ? fragmentHtml(f) : ''
}
const num = (v: unknown, d: number) => (typeof v === 'number' && Number.isFinite(v) ? v : d)
function boxStyle(el: SlideElement) {
  return { left: `${el.x}px`, top: `${el.y}px`, width: `${el.w}px`, height: `${el.h}px`, transform: el.rotation ? `rotate(${el.rotation}deg)` : undefined, zIndex: el.z }
}
function textStyle(el: SlideElement) {
  return {
    fontSize: `${num(el.props.fontSize, 48)}px`,
    color: slideColor(el.props.color, 'var(--text)'),
    textAlign: (typeof el.props.align === 'string' ? el.props.align : 'left') as 'left',
  }
}
const stroke = (el: SlideElement) => slideColor(el.props.stroke, 'none')
const fill = (el: SlideElement) => slideColor(el.props.fill, 'none')
const sw = (el: SlideElement) => num(el.props.strokeWidth, 0)
const label = (el: SlideElement) => el.alt || (el.type === 'text' ? '' : el.type)
</script>

<template>
  <div ref="root" class="sv" :style="{ aspectRatio: `${size.w} / ${size.h}` }">
    <div class="sv-stage" :style="{ width: `${size.w}px`, height: `${size.h}px`, transform: `scale(${scale})`, background: slideColor(slide.background, 'var(--bg-elevated)') }">
      <div v-for="el in slide.elements" :key="el.id" class="sv-el" :data-el="el.id" :data-type="el.type" :style="boxStyle(el)">
        <template v-if="el.type === 'text'">
          <slot v-if="editingId === el.id" name="editor" :el="el" :text-style="textStyle(el)" />
          <div v-else class="sv-text slide-text" :style="textStyle(el)" :aria-label="el.alt || undefined" v-html="html(el)" />
        </template>
        <svg v-else-if="el.type === 'rect'" class="sv-svg" :viewBox="`0 0 ${el.w} ${el.h}`" role="img" :aria-label="label(el)">
          <rect :x="sw(el) / 2" :y="sw(el) / 2" :width="Math.max(0, el.w - sw(el))" :height="Math.max(0, el.h - sw(el))" :rx="num(el.props.radius, 0)" :fill="fill(el)" :stroke="stroke(el)" :stroke-width="sw(el)" />
        </svg>
        <svg v-else-if="el.type === 'ellipse'" class="sv-svg" :viewBox="`0 0 ${el.w} ${el.h}`" role="img" :aria-label="label(el)">
          <ellipse :cx="el.w / 2" :cy="el.h / 2" :rx="Math.max(0, (el.w - sw(el)) / 2)" :ry="Math.max(0, (el.h - sw(el)) / 2)" :fill="fill(el)" :stroke="stroke(el)" :stroke-width="sw(el)" />
        </svg>
        <svg v-else-if="el.type === 'line'" class="sv-svg" :viewBox="`0 0 ${el.w} ${el.h}`" role="img" :aria-label="label(el)">
          <line :x1="0" :y1="el.h / 2" :x2="el.w" :y2="el.h / 2" :stroke="slideColor(el.props.stroke, 'var(--text)')" :stroke-width="num(el.props.strokeWidth, 6)" stroke-linecap="round" />
        </svg>
        <img v-else-if="el.type === 'image' && typeof el.props.src === 'string' && /^(\/|https?:)/.test(el.props.src)" class="sv-img" :src="el.props.src" :alt="el.alt" draggable="false" />
      </div>
      <slot />
    </div>
  </div>
</template>

<style scoped>
.sv { position: relative; width: 100%; overflow: hidden; container-type: normal; }
.sv-stage { position: absolute; left: 0; top: 0; transform-origin: 0 0; overflow: hidden; color: var(--text); }
.sv-el { position: absolute; }
.sv-text { width: 100%; height: 100%; overflow: hidden; line-height: 1.2; overflow-wrap: anywhere; }
.sv-svg { width: 100%; height: 100%; display: block; overflow: visible; }
.sv-img { width: 100%; height: 100%; object-fit: contain; display: block; user-select: none; }
.slide-text :deep(p), .slide-text :deep(h1), .slide-text :deep(h2), .slide-text :deep(h3), .slide-text :deep(h4), .slide-text :deep(ul), .slide-text :deep(ol), .slide-text :deep(blockquote), .slide-text :deep(pre) { margin: 0 0 0.35em; }
.slide-text :deep(h1) { font-size: 1.6em; } .slide-text :deep(h2) { font-size: 1.35em; } .slide-text :deep(h3), .slide-text :deep(h4) { font-size: 1.15em; }
.slide-text :deep(ul), .slide-text :deep(ol) { padding-left: 1.4em; }
.slide-text :deep(blockquote) { border-left: 0.12em solid var(--border-strong); padding-left: 0.6em; color: var(--text-2); }
.slide-text :deep(code) { font-family: var(--font-mono, monospace); background: var(--bg-code); padding: 0 0.2em; border-radius: 0.15em; }
.slide-text :deep(a) { color: var(--accent-text); text-decoration: underline; }
</style>
