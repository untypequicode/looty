<script setup lang="ts">
import type * as Y from 'yjs'
import type { HocuspocusProvider } from '@hocuspocus/provider'
import { ChevronLeft, ChevronRight, Minus, Moon, Pause, Play, Plus, Radio, RotateCcw, Sun } from 'lucide-vue-next'
import { slideSize } from '~/slides/model'
import { getNotes } from '~/slides/present'
import { elapsed, formatClock } from '~/slides/presentSync'
import { usePresentation } from '~/composables/usePresentation'
import { useI18n } from '~/composables/useI18n'
import PresentSlide from './PresentSlide.vue'

/**
 * Vue présentateur (issue #303), dans une seconde fenêtre : diapositive en cours, suivante, notes de
 * l'orateur, minuteur, horloge et progression. Synchronisée avec le diaporama par BroadcastChannel
 * (repli sur localStorage) ; elle peut aussi piloter la présentation et la partager pour être suivie.
 */
const props = defineProps<{ doc: Y.Doc; pageId: string; provider?: HocuspocusProvider | null; canShare?: boolean }>()
const { t } = useI18n()
const P = usePresentation({ doc: props.doc, pageId: props.pageId, provider: props.provider, canShare: props.canShare })
const { ids, steps, pos, screen, slideId, tick, timer, sharing } = P
const size = computed(() => { void tick.value; return slideSize(props.doc) })
const total = computed(() => ids.value.length)
const notes = computed(() => { void tick.value; return getNotes(props.doc, slideId.value) })
const nextPos = computed(() => {
  const c = pos.value
  if (c.step < (steps.value[c.slide] ?? 0)) return { slide: c.slide, step: c.step + 1 }
  return c.slide < total.value - 1 ? { slide: c.slide + 1, step: 0 } : null
})
const nextId = computed(() => (nextPos.value ? ids.value[nextPos.value.slide] ?? '' : ''))

const now = ref(Date.now())
let clockTimer: ReturnType<typeof setInterval> | undefined
const fontSize = ref(22)
const clock = computed(() => new Date(now.value).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }))
const timeText = computed(() => formatClock(elapsed(timer.startedAt, timer.pausedAt, now.value)))
const percent = computed(() => (total.value ? Math.round(((pos.value.slide + 1) / total.value) * 100) : 0))
const ratio = computed(() => size.value.w / size.value.h)
const frameStyle = computed(() => ({ aspectRatio: `${size.value.w} / ${size.value.h}`, width: `min(100%, calc((100dvh - 240px) * ${ratio.value}))` }))
const smallStyle = computed(() => ({ aspectRatio: `${size.value.w} / ${size.value.h}`, width: `min(100%, calc(30dvh * ${ratio.value}))` }))
const timerLabel = computed(() => (timer.startedAt === null ? t('present.timerStart') : timer.pausedAt === null ? t('present.timerPause') : t('present.timerResume')))

function onKey(e: KeyboardEvent) {
  if (e.ctrlKey || e.metaKey || e.altKey) return
  const k = e.key
  const typing = e.target instanceof HTMLElement && e.target.closest('textarea, input, select')
  if (typing) return
  const onControl = e.target instanceof HTMLElement && e.target.closest('button, a')
  if (onControl && (k === ' ' || k === 'Enter')) return
  if (['ArrowRight', 'ArrowDown', 'PageDown', ' ', 'Enter'].includes(k)) { e.preventDefault(); P.next() }
  else if (['ArrowLeft', 'ArrowUp', 'PageUp', 'Backspace'].includes(k)) { e.preventDefault(); P.prev() }
  else if (k === 'Home') { e.preventDefault(); P.first() }
  else if (k === 'End') { e.preventDefault(); P.last() }
  else if (k === 'b' || k === 'B' || k === '.') P.blank('black')
  else if (k === 'w' || k === 'W' || k === ',') P.blank('white')
}
onMounted(() => {
  clockTimer = setInterval(() => { now.value = Date.now() }, 1000)
  P.startTimer()
  if (props.canShare && props.provider) P.share(true)
})
onBeforeUnmount(() => clearInterval(clockTimer))
</script>

<template>
  <div class="pv" tabindex="-1" @keydown="onKey">
    <header class="pv-top">
      <div class="pv-time">
        <span class="pv-label">{{ t('present.elapsed') }}</span>
        <strong class="pv-big" role="timer" :aria-label="`${t('present.elapsed')} ${timeText}`">{{ timeText }}</strong>
        <button type="button" class="pv-btn" :title="timerLabel" :aria-label="timerLabel" @click="P.toggleTimer()"><Pause v-if="timer.startedAt !== null && timer.pausedAt === null" :size="16" /><Play v-else :size="16" /></button>
        <button type="button" class="pv-btn" :title="t('present.timerReset')" :aria-label="t('present.timerReset')" @click="P.resetTimer()"><RotateCcw :size="16" /></button>
      </div>
      <div class="pv-prog" role="progressbar" :aria-label="t('present.progress')" aria-valuemin="0" aria-valuemax="100" :aria-valuenow="percent" :aria-valuetext="t('present.slideOf', { n: pos.slide + 1, total })">
        <span class="pv-bar"><span :style="{ width: `${percent}%` }" /></span>
        <span class="pv-count">{{ t('present.slideOf', { n: pos.slide + 1, total }) }}<template v-if="(steps[pos.slide] ?? 0) > 0"> · {{ t('present.step', { n: pos.step, total: steps[pos.slide] }) }}</template></span>
      </div>
      <div class="pv-time">
        <span class="pv-label">{{ t('present.clock') }}</span>
        <strong class="pv-big">{{ clock }}</strong>
      </div>
    </header>

    <main class="pv-main">
      <section class="pv-cur" :aria-label="t('present.current')">
        <h2 class="pv-h">{{ t('present.current') }}</h2>
        <div class="pv-frame" :style="frameStyle">
          <PresentSlide v-if="slideId" :key="slideId" :doc="doc" :slide-id="slideId" :tick="tick" :step="pos.step" />
          <div v-if="screen" class="pv-screen" :class="screen" />
        </div>
        <div class="pv-nav" role="toolbar" :aria-label="t('present.controls')">
          <button type="button" class="pv-btn wide" :aria-label="t('present.prev')" @click="P.prev()"><ChevronLeft :size="20" /> {{ t('present.prev') }}</button>
          <button type="button" class="pv-btn wide" :aria-label="t('present.next')" @click="P.next()">{{ t('present.next') }} <ChevronRight :size="20" /></button>
          <button type="button" class="pv-btn" :class="{ on: screen === 'black' }" :aria-pressed="screen === 'black'" :title="t('present.black')" :aria-label="t('present.black')" @click="P.blank('black')"><Moon :size="16" /></button>
          <button type="button" class="pv-btn" :class="{ on: screen === 'white' }" :aria-pressed="screen === 'white'" :title="t('present.white')" :aria-label="t('present.white')" @click="P.blank('white')"><Sun :size="16" /></button>
          <button v-if="canShare && provider" type="button" class="pv-btn" :class="{ on: sharing }" :aria-pressed="sharing" :title="sharing ? t('present.sharing') : t('present.notSharing')" :aria-label="t('present.share')" @click="P.share(!sharing)"><Radio :size="16" /></button>
        </div>
      </section>

      <aside class="pv-side">
        <section :aria-label="t('present.upNext')">
          <h2 class="pv-h">{{ t('present.upNext') }}</h2>
          <div v-if="nextPos && nextId" class="pv-frame small" :style="smallStyle">
            <PresentSlide :key="`${nextId}-${nextPos.step}`" :doc="doc" :slide-id="nextId" :tick="tick" :step="nextPos.step" />
          </div>
          <p v-else class="pv-end">{{ t('present.endOfShow') }}</p>
        </section>
        <section class="pv-notes" :aria-label="t('present.notes')">
          <div class="pv-notes-head">
            <h2 class="pv-h">{{ t('present.notes') }}</h2>
            <button type="button" class="pv-btn" :title="t('present.notesSmaller')" :aria-label="t('present.notesSmaller')" @click="fontSize = Math.max(14, fontSize - 2)"><Minus :size="14" /></button>
            <button type="button" class="pv-btn" :title="t('present.notesBigger')" :aria-label="t('present.notesBigger')" @click="fontSize = Math.min(48, fontSize + 2)"><Plus :size="14" /></button>
          </div>
          <div class="pv-notes-body" tabindex="0" :style="{ fontSize: `${fontSize}px` }">{{ notes || t('present.noNotes') }}</div>
        </section>
      </aside>
    </main>
    <p class="pv-hint">{{ t('present.syncHint') }}</p>
  </div>
</template>

<style scoped>
.pv { position: fixed; inset: 0; display: flex; flex-direction: column; background: #111318; color: #f2f3f5; outline: none; font-family: inherit; }
.pv-top { display: flex; align-items: center; gap: 24px; padding: 10px 20px; border-bottom: 1px solid rgba(255, 255, 255, 0.12); }
.pv-time { display: flex; align-items: center; gap: 8px; }
.pv-label { font-size: 12px; opacity: 0.7; text-transform: uppercase; letter-spacing: 0.04em; }
.pv-big { font-size: 28px; font-variant-numeric: tabular-nums; }
.pv-prog { flex: 1; display: flex; flex-direction: column; gap: 4px; min-width: 120px; }
.pv-bar { display: block; height: 6px; border-radius: 3px; background: rgba(255, 255, 255, 0.15); overflow: hidden; }
.pv-bar > span { display: block; height: 100%; background: #4da3ff; transition: width 0.2s; }
.pv-count { font-size: 12px; opacity: 0.8; }
.pv-main { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(0, 3fr) minmax(260px, 2fr); gap: 20px; padding: 16px 20px; }
.pv-h { margin: 0 0 6px; font-size: 12px; font-weight: 600; opacity: 0.7; text-transform: uppercase; letter-spacing: 0.04em; }
.pv-frame { position: relative; width: 100%; overflow: hidden; border-radius: 6px; background: #000; }

.pv-screen { position: absolute; inset: 0; opacity: 0.85; }
.pv-screen.black { background: #000; } .pv-screen.white { background: #fff; }
.pv-nav { display: flex; gap: 8px; margin-top: 12px; flex-wrap: wrap; }
.pv-btn { display: inline-flex; align-items: center; justify-content: center; gap: 4px; min-width: 34px; height: 34px; padding: 0 8px; border: 1px solid rgba(255, 255, 255, 0.2); border-radius: 8px; background: rgba(255, 255, 255, 0.06); color: inherit; font: inherit; font-size: 14px; cursor: pointer; }
.pv-btn.wide { padding: 0 16px; height: 42px; }
.pv-btn:hover { background: rgba(255, 255, 255, 0.16); }
.pv-btn.on { background: rgba(77, 163, 255, 0.4); }
.pv-btn:focus-visible, .pv-notes-body:focus-visible { outline: 2px solid #fff; outline-offset: 2px; }
.pv-side { display: flex; flex-direction: column; gap: 16px; min-height: 0; }
.pv-end { margin: 0; opacity: 0.7; }
.pv-notes { flex: 1; min-height: 0; display: flex; flex-direction: column; }
.pv-notes-head { display: flex; align-items: center; gap: 6px; }
.pv-notes-head .pv-h { flex: 1; margin: 0; }
.pv-notes-body { flex: 1; min-height: 0; overflow-y: auto; margin-top: 6px; padding: 12px 14px; border-radius: 8px; background: rgba(255, 255, 255, 0.07); white-space: pre-wrap; overflow-wrap: anywhere; line-height: 1.45; }
.pv-hint { margin: 0; padding: 6px 20px 10px; font-size: 12px; opacity: 0.6; }
@media (max-width: 800px) { .pv-main { grid-template-columns: 1fr; overflow-y: auto; } .pv-top { flex-wrap: wrap; gap: 10px; } }
</style>
