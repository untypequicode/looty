<script setup lang="ts">
import type * as Y from 'yjs'
import type { HocuspocusProvider } from '@hocuspocus/provider'
import {
  ChevronLeft, ChevronRight, Crosshair, Grid3x3, Maximize, Moon, Radio, Sun, X, ZoomIn, ZoomOut, MonitorPlay,
} from 'lucide-vue-next'
import { fragmentText, listSlides, slideSize, textOf } from '~/slides/model'
import { effectiveTransition, prefersReducedMotion } from '~/slides/present'
import { usePresentation } from '~/composables/usePresentation'
import { useI18n } from '~/composables/useI18n'
import PresentSlide from './PresentSlide.vue'

/**
 * Diaporama plein écran (issue #303) : clavier, télécommande, balayage, barre discrète, grille des
 * diapositives, écran noir ou blanc, pointeur laser, zoom, transitions et animations, suivi d'une
 * personne qui présente. `embedded` : dans une page dédiée (lien, kiosque) plutôt qu'en surimpression.
 */
const props = defineProps<{
  doc: Y.Doc
  pageId: string
  provider?: HocuspocusProvider | null
  canShare?: boolean
  startSlideId?: string | null
  startFollowing?: boolean
  /** Défilement automatique en boucle, en secondes par diapositive (0 : désactivé). */
  kiosk?: number
  /** Adresse de la vue présentateur (absente : bouton masqué). */
  presenterUrl?: string
  embedded?: boolean
}>()
const emit = defineEmits<{ (e: 'close'): void }>()
const { t } = useI18n()

const P = usePresentation({ doc: props.doc, pageId: props.pageId, provider: props.provider, canShare: props.canShare, startSlideId: props.startSlideId, startFollowing: props.startFollowing })
const { ids, steps, pos, screen, slideId, tick, presenter, following, detached, sharing } = P
const size = computed(() => { void tick.value; return slideSize(props.doc) })
const total = computed(() => ids.value.length)
const reduced = prefersReducedMotion()

const root = ref<HTMLElement | null>(null)
const controlsOn = ref(true)
const gridOpen = ref(false)
const laserOn = ref(false)
const laser = reactive({ x: -100, y: -100 })
const zoom = ref(1)
const origin = reactive({ x: 50, y: 50 })
const jumpBuf = ref('')
let hideTimer: ReturnType<typeof setTimeout> | undefined
let jumpTimer: ReturnType<typeof setTimeout> | undefined
let opener: Element | null = null
let enteredFs = false

// --- Transition et diapositive affichée ---
const back = ref(false)
watch(() => pos.value.slide, (n, o) => { back.value = n < o })
const transName = computed(() => {
  void tick.value
  const k = effectiveTransition(props.doc, slideId.value)
  return k === 'none' || reduced ? 'pt-none' : k === 'slide' && back.value ? 'pt-slide-back' : `pt-${k}`
})
const frameStyle = computed(() => ({ aspectRatio: `${size.value.w} / ${size.value.h}`, width: `min(100vw, calc(100dvh * ${size.value.w / size.value.h}))` }))
const zoomStyle = computed(() => (zoom.value > 1 ? { transform: `scale(${zoom.value})`, transformOrigin: `${origin.x}% ${origin.y}%` } : undefined))

// --- Annonce pour les lecteurs d'écran ---
const announcement = computed(() => {
  void tick.value
  const s = listSlides(props.doc)[pos.value.slide]
  if (!s) return ''
  const text = s.elements.filter((e) => e.type === 'text').map((e) => { const f = textOf(props.doc, s.id, e.id); return f ? fragmentText(f) : '' }).filter(Boolean).join('. ').slice(0, 160)
  return t('present.announce', { n: pos.value.slide + 1, total: total.value, text: text || t('present.noText') })
})

// --- Barre discrète ---
function showControls() {
  controlsOn.value = true
  clearTimeout(hideTimer)
  hideTimer = setTimeout(() => { if (!root.value?.querySelector('.ps-bar:focus-within') && !gridOpen.value) controlsOn.value = false }, 2600)
}

// --- Plein écran ---
async function enterFs() {
  if (props.embedded && !props.kiosk) return
  try { await root.value?.requestFullscreen?.(); enteredFs = !!document.fullscreenElement } catch { /* plein écran refusé : le diaporama reste en surimpression */ }
}
function toggleFs() {
  if (document.fullscreenElement) void document.exitFullscreen()
  else void enterFs()
}
function onFsChange() {
  if (enteredFs && !document.fullscreenElement) { enteredFs = false; close() }
}
function close() { emit('close') }

// --- Clavier ---
const NEXT = new Set(['ArrowRight', 'ArrowDown', 'PageDown', ' ', 'Enter', 'n', 'N'])
const PREV = new Set(['ArrowLeft', 'ArrowUp', 'PageUp', 'Backspace', 'p', 'P'])
function onKey(e: KeyboardEvent) {
  if (e.ctrlKey || e.metaKey || e.altKey) return
  showControls()
  const k = e.key
  if (gridOpen.value) {
    if (k === 'Escape') { e.preventDefault(); e.stopPropagation(); closeGrid() }
    else if (k.startsWith('Arrow')) { e.preventDefault(); moveGridFocus(k) }
    return
  }
  const onControl = e.target instanceof HTMLElement && e.target.closest('button, a, input, select')
  if (onControl && (k === ' ' || k === 'Enter')) return
  const done = () => e.preventDefault()
  if (k === 'Escape') { done(); close() }
  else if (/^[0-9]$/.test(k)) { done(); jumpBuf.value = (jumpBuf.value + k).slice(0, 3); clearTimeout(jumpTimer); jumpTimer = setTimeout(() => { jumpBuf.value = '' }, 2500) }
  else if (k === 'Enter' && jumpBuf.value) { done(); P.jump(Number(jumpBuf.value) - 1); jumpBuf.value = '' }
  else if (NEXT.has(k)) { done(); P.next(); restartKiosk() }
  else if (PREV.has(k)) { done(); P.prev(); restartKiosk() }
  else if (k === 'Home') { done(); P.first() }
  else if (k === 'End') { done(); P.last() }
  else if (k === 'b' || k === 'B' || k === '.') { done(); P.blank('black') }
  else if (k === 'w' || k === 'W' || k === ',') { done(); P.blank('white') }
  else if (k === 'g' || k === 'G') { done(); openGrid() }
  else if (k === 'l' || k === 'L') { done(); laserOn.value = !laserOn.value }
  else if (k === '+' || k === '=') { done(); setZoom(zoom.value + 0.5) }
  else if (k === '-' || k === '_') { done(); setZoom(zoom.value - 0.5) }
  else if (k === '0') { done(); setZoom(1) }
  else if (k === 'f' || k === 'F') { done(); toggleFs() }
  else if (screen.value && k !== 'Tab') P.setScreen('')
}
function setZoom(z: number) { zoom.value = Math.max(1, Math.min(3, z)) }

// --- Pointeur, laser, balayage ---
let down: { x: number; y: number; t: number } | null = null
let swiped = false
function onMove(e: PointerEvent) {
  showControls()
  laser.x = e.clientX; laser.y = e.clientY
  if (zoom.value > 1) { origin.x = (e.clientX / window.innerWidth) * 100; origin.y = (e.clientY / window.innerHeight) * 100 }
}
function onDown(e: PointerEvent) { down = { x: e.clientX, y: e.clientY, t: Date.now() }; swiped = false }
function onUp(e: PointerEvent) {
  if (!down) return
  const dx = e.clientX - down.x
  const dy = e.clientY - down.y
  if (Math.abs(dx) > 50 && Math.abs(dy) < Math.abs(dx) * 0.6 && Date.now() - down.t < 900) {
    swiped = true
    if (dx < 0) P.next(); else P.prev()
    restartKiosk()
  }
  down = null
}
function onClick(e: MouseEvent) {
  if (swiped || laserOn.value || screen.value) { swiped = false; if (screen.value) P.setScreen(''); return }
  if ((e.target as HTMLElement).closest('a, button, input, .ps-bar, .ps-grid, .ps-follow')) return
  P.next()
  restartKiosk()
}

// --- Grille des diapositives ---
const gridEl = ref<HTMLElement | null>(null)
const seen = reactive(new Set<number>())
let io: IntersectionObserver | null = null
function openGrid() {
  gridOpen.value = true
  controlsOn.value = true
  nextTick(() => {
    io?.disconnect()
    io = typeof IntersectionObserver === 'function' ? new IntersectionObserver((es) => es.forEach((en) => { if (en.isIntersecting) seen.add(Number((en.target as HTMLElement).dataset.gi)) }), { root: gridEl.value }) : null
    gridEl.value?.querySelectorAll<HTMLElement>('[data-gi]').forEach((n) => { if (io) io.observe(n); else seen.add(Number(n.dataset.gi)) })
    ;(gridEl.value?.querySelector<HTMLElement>(`[data-gi="${pos.value.slide}"]`) ?? gridEl.value?.querySelector<HTMLElement>('button'))?.focus()
  })
}
function closeGrid() { gridOpen.value = false; io?.disconnect(); nextTick(() => root.value?.focus()) }
function pick(i: number) { P.jump(i); closeGrid() }
function moveGridFocus(k: string) {
  const items = [...(gridEl.value?.querySelectorAll<HTMLElement>('button[data-gi]') ?? [])]
  const at = items.indexOf(document.activeElement as HTMLElement)
  const cols = Math.max(1, Math.round((gridEl.value?.clientWidth ?? 0) / ((items[0]?.offsetWidth ?? 1) + 12)))
  const delta = k === 'ArrowLeft' ? -1 : k === 'ArrowRight' ? 1 : k === 'ArrowUp' ? -cols : cols
  items[Math.max(0, Math.min(items.length - 1, at + delta))]?.focus()
}

// --- Kiosque : défilement automatique en boucle ---
let kioskTimer: ReturnType<typeof setInterval> | undefined
function restartKiosk() {
  if (!props.kiosk) return
  clearInterval(kioskTimer)
  kioskTimer = setInterval(() => {
    if (document.hidden || gridOpen.value || screen.value) return
    const last = pos.value.slide >= total.value - 1 && pos.value.step >= (steps.value[pos.value.slide] ?? 0)
    if (last) P.set({ slide: 0, step: 0 }); else P.set(nextOf())
  }, Math.max(2, props.kiosk) * 1000)
}
function nextOf() {
  const c = pos.value
  return c.step < (steps.value[c.slide] ?? 0) ? { slide: c.slide, step: c.step + 1 } : { slide: c.slide + 1, step: 0 }
}

// --- Suivi ---
const showFollowBar = computed(() => following.value || (!!presenter.value && !sharing.value))

onMounted(() => {
  opener = document.activeElement
  root.value?.focus()
  document.addEventListener('fullscreenchange', onFsChange)
  void enterFs()
  showControls()
  restartKiosk()
  if (props.canShare && props.provider && !props.embedded) P.share(true)
})
onBeforeUnmount(() => {
  document.removeEventListener('fullscreenchange', onFsChange)
  clearTimeout(hideTimer); clearTimeout(jumpTimer); clearInterval(kioskTimer); io?.disconnect()
  enteredFs = false
  if (document.fullscreenElement === root.value) void document.exitFullscreen()
  if (opener instanceof HTMLElement) opener.focus()
})
</script>

<template>
  <div
    ref="root" class="ps" :class="{ laser: laserOn, idle: !controlsOn }" role="dialog" aria-modal="true" :aria-label="t('present.stage')" tabindex="-1"
    @keydown="onKey" @pointermove="onMove" @pointerdown="onDown" @pointerup="onUp" @click="onClick" @focusin="showControls"
  >
    <div class="ps-stage" :style="zoomStyle">
      <div class="ps-frame" :style="frameStyle">
        <Transition :name="transName" :css="transName !== 'pt-none'">
          <PresentSlide v-if="slideId" :key="slideId" class="ps-slide" :doc="doc" :slide-id="slideId" :tick="tick" :step="pos.step" animate />
        </Transition>
      </div>
    </div>
    <div v-if="screen" class="ps-screen" :class="screen" aria-hidden="true" />
    <div v-if="laserOn" class="ps-laser" aria-hidden="true" :style="{ left: `${laser.x}px`, top: `${laser.y}px` }" />
    <div class="ps-sr" aria-live="polite" aria-atomic="true">{{ announcement }}</div>
    <p v-if="jumpBuf" class="ps-jump" role="status">{{ t('present.jump') }} {{ jumpBuf }}</p>

    <div v-if="showFollowBar" class="ps-follow" role="status">
      <template v-if="following">
        <span v-if="!presenter" class="ps-msg">{{ t('present.presenterEnded') }}</span>
        <span v-else-if="detached" class="ps-msg">{{ t('present.detached') }}</span>
        <span v-else class="ps-msg"><Radio :size="14" /> {{ t('present.following', { name: presenter.name }) }}</span>
        <button v-if="detached && presenter" type="button" class="ps-link" @click="P.backToPresenter()">{{ t('present.backToPresenter') }}</button>
        <button type="button" class="ps-link" @click="P.stopFollowing()">{{ t('present.leave') }}</button>
      </template>
      <template v-else-if="presenter">
        <span class="ps-msg"><Radio :size="14" /> {{ t('present.presenting', { name: presenter.name }) }}</span>
        <button type="button" class="ps-link" @click="P.follow()">{{ t('present.follow') }}</button>
      </template>
    </div>

    <div class="ps-bar" role="toolbar" :aria-label="t('present.controls')">
      <button type="button" class="ps-btn" :title="t('present.prev')" :aria-label="t('present.prev')" @click="P.prev(); restartKiosk()"><ChevronLeft :size="18" /></button>
      <button type="button" class="ps-btn num" :title="t('present.grid')" :aria-label="`${t('present.slideOf', { n: pos.slide + 1, total })}. ${t('present.grid')}`" @click="openGrid"><Grid3x3 :size="15" /> {{ pos.slide + 1 }} / {{ total }}</button>
      <button type="button" class="ps-btn" :title="t('present.next')" :aria-label="t('present.next')" @click="P.next(); restartKiosk()"><ChevronRight :size="18" /></button>
      <span class="sep" />
      <button type="button" class="ps-btn" :class="{ on: screen === 'black' }" :aria-pressed="screen === 'black'" :title="t('present.black')" :aria-label="t('present.black')" @click="P.blank('black')"><Moon :size="16" /></button>
      <button type="button" class="ps-btn" :class="{ on: screen === 'white' }" :aria-pressed="screen === 'white'" :title="t('present.white')" :aria-label="t('present.white')" @click="P.blank('white')"><Sun :size="16" /></button>
      <button type="button" class="ps-btn" :class="{ on: laserOn }" :aria-pressed="laserOn" :title="t('present.laser')" :aria-label="t('present.laser')" @click="laserOn = !laserOn"><Crosshair :size="16" /></button>
      <button type="button" class="ps-btn" :title="t('present.zoomOut')" :aria-label="t('present.zoomOut')" :disabled="zoom <= 1" @click="setZoom(zoom - 0.5)"><ZoomOut :size="16" /></button>
      <button type="button" class="ps-btn" :title="t('present.zoomIn')" :aria-label="t('present.zoomIn')" :disabled="zoom >= 3" @click="setZoom(zoom + 0.5)"><ZoomIn :size="16" /></button>
      <button type="button" class="ps-btn" :title="t('present.fullscreen')" :aria-label="t('present.fullscreen')" @click="toggleFs"><Maximize :size="16" /></button>
      <template v-if="canShare && provider && !embedded">
        <span class="sep" />
        <button type="button" class="ps-btn" :class="{ on: sharing }" :aria-pressed="sharing" :title="sharing ? t('present.sharing') : t('present.notSharing')" :aria-label="t('present.share')" @click="P.share(!sharing)"><Radio :size="16" /></button>
      </template>
      <a v-if="presenterUrl" class="ps-btn" :href="presenterUrl" target="_blank" rel="noopener" :title="t('present.presenterWindow')" :aria-label="t('present.presenterWindow')"><MonitorPlay :size="16" /></a>
      <span class="sep" />
      <button type="button" class="ps-btn" :title="t('present.exit')" :aria-label="t('present.exit')" @click="close"><X :size="18" /></button>
    </div>

    <div v-if="gridOpen" class="ps-grid" role="dialog" aria-modal="true" :aria-label="t('present.grid')">
      <div class="ps-grid-head">
        <h2>{{ t('present.gridHint') }}</h2>
        <button type="button" class="ps-btn" :title="t('present.closeGrid')" :aria-label="t('present.closeGrid')" @click="closeGrid"><X :size="18" /></button>
      </div>
      <div ref="gridEl" class="ps-cells">
        <button v-for="(id, i) in ids" :key="id" type="button" class="ps-cell" :class="{ on: i === pos.slide }" :data-gi="i" :aria-current="i === pos.slide ? 'true' : undefined" :aria-label="t('slides.slideN', { n: i + 1 })" @click="pick(i)">
          <span class="ps-cell-n">{{ i + 1 }}</span>
          <span class="ps-cell-v"><PresentSlide v-if="seen.has(i)" :doc="doc" :slide-id="id" :tick="tick" :step="steps[i] ?? 0" /></span>
        </button>
      </div>
    </div>
  </div>
</template>

<style>
.ps { position: fixed; inset: 0; z-index: 400; background: #000; color: #fff; display: grid; place-items: center; overflow: hidden; outline: none; user-select: none; touch-action: pan-y; }
.ps.laser, .ps.laser * { cursor: none; }
.ps.idle:not(.laser) { cursor: none; }
.ps-stage { width: 100%; height: 100%; display: grid; place-items: center; transition: transform 0.15s ease-out; }
.ps-frame { position: relative; overflow: hidden; max-height: 100dvh; }
.ps-slide { position: absolute; inset: 0; }
.ps-screen { position: absolute; inset: 0; z-index: 20; }
.ps-screen.black { background: #000; } .ps-screen.white { background: #fff; }
.ps-laser { position: fixed; z-index: 30; width: 22px; height: 22px; margin: -11px 0 0 -11px; border-radius: 50%; background: radial-gradient(circle, #ff3b30 30%, rgba(255, 59, 48, 0.35) 65%, transparent 70%); pointer-events: none; }
.ps-sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }
.ps-jump { position: absolute; top: 16px; right: 16px; z-index: 25; margin: 0; padding: 6px 12px; border-radius: 8px; background: rgba(0, 0, 0, 0.7); font-size: 18px; }
.ps-bar { position: absolute; left: 50%; bottom: 14px; transform: translateX(-50%); z-index: 40; display: flex; align-items: center; gap: 2px; padding: 5px 8px; border-radius: 12px; background: rgba(20, 20, 24, 0.82); backdrop-filter: blur(6px); max-width: calc(100vw - 16px); overflow-x: auto; transition: opacity 0.25s; }
.ps.idle .ps-bar:not(:focus-within) { opacity: 0; pointer-events: none; }
.ps-bar .sep { width: 1px; height: 18px; margin: 0 4px; background: rgba(255, 255, 255, 0.25); }
.ps-btn { display: inline-flex; align-items: center; justify-content: center; gap: 4px; min-width: 34px; height: 34px; padding: 0 8px; border: 0; border-radius: 8px; background: transparent; color: #fff; font: inherit; font-size: 13px; cursor: pointer; text-decoration: none; }
.ps-btn:hover:not(:disabled) { background: rgba(255, 255, 255, 0.16); }
.ps-btn:focus-visible { outline: 2px solid #fff; outline-offset: 1px; }
.ps-btn:disabled { opacity: 0.4; cursor: default; }
.ps-btn.on { background: rgba(255, 255, 255, 0.28); }
.ps-btn.num { font-variant-numeric: tabular-nums; }
.ps-follow { position: absolute; top: 12px; left: 12px; z-index: 40; display: flex; align-items: center; gap: 10px; padding: 6px 12px; border-radius: 10px; background: rgba(20, 20, 24, 0.85); font-size: 13px; max-width: calc(100vw - 24px); flex-wrap: wrap; }
.ps-msg { display: inline-flex; align-items: center; gap: 6px; }
.ps-link { border: 0; background: transparent; color: #9ecbff; font: inherit; text-decoration: underline; cursor: pointer; padding: 2px; }
.ps-link:focus-visible { outline: 2px solid #fff; }
.ps-grid { position: absolute; inset: 0; z-index: 50; display: flex; flex-direction: column; background: rgba(10, 10, 14, 0.96); }
.ps-grid-head { display: flex; align-items: center; justify-content: space-between; padding: 12px 20px; }
.ps-grid-head h2 { margin: 0; font-size: 16px; font-weight: 600; }
.ps-cells { flex: 1; overflow-y: auto; padding: 4px 20px 24px; display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 12px; align-content: start; }
.ps-cell { position: relative; padding: 0; border: 2px solid transparent; border-radius: 8px; background: #222; cursor: pointer; overflow: hidden; aspect-ratio: 16 / 9; color: #fff; }
.ps-cell.on { border-color: #4da3ff; }
.ps-cell:focus-visible { outline: 3px solid #fff; outline-offset: 2px; }
.ps-cell-v { display: block; pointer-events: none; }
.ps-cell-n { position: absolute; left: 6px; bottom: 6px; z-index: 2; padding: 1px 7px; border-radius: 6px; background: rgba(0, 0, 0, 0.7); font-size: 12px; }
/* Transitions entre diapositives */
.pt-fade-enter-active, .pt-fade-leave-active, .pt-slide-enter-active, .pt-slide-leave-active, .pt-slide-back-enter-active, .pt-slide-back-leave-active, .pt-zoom-enter-active, .pt-zoom-leave-active { transition: opacity 0.4s ease, transform 0.4s ease; }
.pt-fade-enter-from, .pt-fade-leave-to { opacity: 0; }
.pt-slide-enter-from { transform: translateX(100%); } .pt-slide-leave-to { transform: translateX(-100%); }
.pt-slide-back-enter-from { transform: translateX(-100%); } .pt-slide-back-leave-to { transform: translateX(100%); }
.pt-zoom-enter-from { opacity: 0; transform: scale(0.85); } .pt-zoom-leave-to { opacity: 0; transform: scale(1.15); }
@media (prefers-reduced-motion: reduce) {
  .ps-stage, .ps-bar { transition: none; }
}
</style>
