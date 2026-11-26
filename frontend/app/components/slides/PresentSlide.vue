<script setup lang="ts">
import type * as Y from 'yjs'
import { readSlide, slideSize } from '~/slides/model'
import { enteringAt, hiddenAt, prefersReducedMotion, readAnims, schedule } from '~/slides/present'
import SlideView from './SlideView.vue'

/**
 * Une diapositive en mode présentation : le rendu habituel, avec les éléments animés cachés ou révélés
 * selon l'étape. `animate` joue les entrées (sinon elles sont instantanées : miniatures, retour en
 * arrière, préférence « réduire les animations »).
 */
const props = defineProps<{ doc: Y.Doc; slideId: string; tick: number; step: number; animate?: boolean }>()
const root = ref<HTMLElement | null>(null)
const slide = computed(() => { void props.tick; return readSlide(props.doc, props.slideId) })
const size = computed(() => { void props.tick; return slideSize(props.doc) })

let shown = new Set<string>()
let started = false
let raf = 0

function setHidden(el: Element, hide: boolean) {
  el.classList.toggle('pa-hidden', hide)
  if (hide) el.setAttribute('aria-hidden', 'true')
  else el.removeAttribute('aria-hidden')
}

function apply() {
  const r = root.value
  if (!r) return
  const sched = schedule(readAnims(props.doc, props.slideId))
  const kinds = new Map<string, { kind: string; delay: number; ms: number }>()
  for (const a of sched) if (!kinds.has(a.el)) kinds.set(a.el, { kind: a.kind, delay: a.delay, ms: a.ms })
  const hidden = hiddenAt(sched, props.step)
  const animate = !!props.animate && !prefersReducedMotion()
  const arriving = !started
  // À l'arrivée, les entrées de l'étape 0 jouent aussi : on les cache d'abord, puis on les révèle.
  const initial = arriving && animate ? new Set([...hidden, ...enteringAt(sched, 0).map((a) => a.el)]) : hidden
  const nodes = r.querySelectorAll<HTMLElement>('[data-el]')
  nodes.forEach((n) => {
    const id = n.dataset.el!
    const k = kinds.get(id)
    n.classList.toggle('pa', !!k)
    n.classList.toggle('pa-move', k?.kind === 'move')
    if (!k) return
    const reveal = !initial.has(id) && !shown.has(id) && started
    const instant = !animate || !reveal
    n.classList.toggle('pa-instant', instant)
    n.style.setProperty('--pa-ms', `${k.ms}ms`)
    n.style.setProperty('--pa-d', instant ? '0ms' : `${k.delay}ms`)
    setHidden(n, initial.has(id))
  })
  shown = new Set([...nodes].map((n) => n.dataset.el!).filter((id) => !initial.has(id)))
  started = true
  if (initial !== hidden) {
    cancelAnimationFrame(raf)
    raf = requestAnimationFrame(() => { raf = requestAnimationFrame(() => {
      nodes.forEach((n) => {
        const id = n.dataset.el!
        const k = kinds.get(id)
        if (!k || hidden.has(id) || !initial.has(id)) return
        n.classList.remove('pa-instant')
        n.style.setProperty('--pa-d', `${k.delay}ms`)
        setHidden(n, false)
      })
      shown = new Set([...nodes].map((n) => n.dataset.el!).filter((id) => !hidden.has(id)))
    }) })
  }
}

onMounted(() => nextTick(apply))
watch(() => [props.step, props.tick, props.slideId], () => nextTick(apply), { flush: 'post' })
onBeforeUnmount(() => cancelAnimationFrame(raf))
</script>

<template>
  <div ref="root" class="pslide">
    <SlideView v-if="slide" :doc="doc" :slide="slide" :size="size" :tick="tick" />
  </div>
</template>

<style>
.pslide { width: 100%; }
.sv-el.pa { transition: opacity var(--pa-ms, 500ms) ease var(--pa-d, 0ms), translate var(--pa-ms, 500ms) ease var(--pa-d, 0ms); }
.sv-el.pa.pa-instant { transition: none; }
.sv-el.pa.pa-hidden { opacity: 0; pointer-events: none; }
.sv-el.pa.pa-hidden.pa-move { translate: 0 90px; }
</style>
