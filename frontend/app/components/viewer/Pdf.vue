<script setup lang="ts">
import { ZoomIn, ZoomOut, Maximize, Minimize, Download, ChevronUp, ChevronDown, FileWarning, ScanLine } from 'lucide-vue-next'
import type { PDFDocumentProxy } from 'pdfjs-dist'

/**
 * Lecteur PDF intégré (pdf.js) : rendu paresseux des pages visibles,
 * zoom (ajusté à la largeur par défaut), navigation, plein écran, téléchargement.
 */
const props = withDefaults(defineProps<{ src: string; name?: string; height?: string }>(), { height: '560px' })

const root = ref<HTMLElement | null>(null)
const scroller = ref<HTMLElement | null>(null)
const pagesEl = ref<HTMLElement | null>(null)
const doc = shallowRef<PDFDocumentProxy | null>(null)
const numPages = ref(0)
const current = ref(1)
const scale = ref(1)
const fitWidth = ref(true)
const error = ref(false)
const loading = ref(true)
const fullscreen = ref(false)
const rendered = new Map<number, number>() // page -> scale rendue
let observer: IntersectionObserver | null = null
let baseWidth = 612
const ratio = ref(1.294)

async function load() {
  loading.value = true
  error.value = false
  try {
    const pdfjs = await import('pdfjs-dist')
    pdfjs.GlobalWorkerOptions.workerSrc = new URL('pdfjs-dist/build/pdf.worker.min.mjs', import.meta.url).toString()
    // Pas d'exécution de JavaScript embarqué, pas d'évaluation dynamique.
    doc.value = await pdfjs.getDocument({ url: props.src, isEvalSupported: false, enableXfa: false }).promise
    numPages.value = doc.value.numPages
    const first = await doc.value.getPage(1)
    const vp = first.getViewport({ scale: 1 })
    baseWidth = vp.width
    ratio.value = vp.height / vp.width
    await nextTick()
    applyFit()
    setupPages()
  } catch (e) {
    console.error('PDF load failed', e)
    error.value = true
  } finally {
    loading.value = false
  }
}

function applyFit() {
  if (fitWidth.value && scroller.value) scale.value = Math.max(0.3, (scroller.value.clientWidth - 32) / baseWidth)
}

function setupPages() {
  observer?.disconnect()
  rendered.clear()
  observer = new IntersectionObserver((entries) => {
    for (const e of entries) {
      const n = Number((e.target as HTMLElement).dataset.page)
      if (e.isIntersecting) renderPage(n)
      if (e.intersectionRatio > 0.5) current.value = n
    }
  }, { root: scroller.value, rootMargin: '400px 0px', threshold: [0, 0.5] })
  pagesEl.value?.querySelectorAll('.pdf-page').forEach((el) => observer!.observe(el))
}

async function renderPage(n: number) {
  if (!doc.value || rendered.get(n) === scale.value) return
  rendered.set(n, scale.value)
  const page = await doc.value.getPage(n)
  const dpr = Math.min(window.devicePixelRatio || 1, 2)
  const viewport = page.getViewport({ scale: scale.value * dpr })
  const wrap = pagesEl.value?.querySelector(`[data-page="${n}"]`) as HTMLElement | null
  const canvas = wrap?.querySelector('canvas')
  if (!canvas) return
  canvas.width = viewport.width
  canvas.height = viewport.height
  await page.render({ canvasContext: canvas.getContext('2d')!, viewport }).promise
}

const pageStyle = computed(() => ({ width: `${baseWidth * scale.value}px`, height: `${baseWidth * scale.value * ratio.value}px` }))

function zoom(f: number) {
  fitWidth.value = false
  scale.value = Math.min(4, Math.max(0.3, scale.value * f))
}
function fit() { fitWidth.value = true; applyFit() }
watch(scale, () => nextTick(() => {
  rendered.clear()
  pagesEl.value?.querySelectorAll('.pdf-page').forEach((el) => {
    const r = el.getBoundingClientRect()
    const s = scroller.value!.getBoundingClientRect()
    if (r.bottom > s.top - 400 && r.top < s.bottom + 400) renderPage(Number((el as HTMLElement).dataset.page))
  })
}))
function goto(n: number) {
  const el = pagesEl.value?.querySelector(`[data-page="${Math.min(numPages.value, Math.max(1, n))}"]`)
  el?.scrollIntoView({ block: 'start', behavior: 'smooth' })
}
async function toggleFullscreen() {
  if (document.fullscreenElement) await document.exitFullscreen()
  else await root.value?.requestFullscreen()
}
function onFs() { fullscreen.value = document.fullscreenElement === root.value; nextTick(() => { applyFit() }) }

let ro: ResizeObserver | null = null
onMounted(() => {
  load()
  document.addEventListener('fullscreenchange', onFs)
  ro = new ResizeObserver(() => applyFit())
  if (scroller.value) ro.observe(scroller.value)
})
onBeforeUnmount(() => {
  observer?.disconnect()
  ro?.disconnect()
  document.removeEventListener('fullscreenchange', onFs)
  doc.value?.destroy()
})
watch(() => props.src, load)
</script>

<template>
  <div ref="root" class="pdf" :class="{ fullscreen }" :style="{ height: fullscreen ? '100vh' : height }" contenteditable="false">
    <div class="pdf-bar">
      <span class="pdf-name truncate">{{ name || src.split('/').pop() }}</span>
      <span class="spacer" />
      <template v-if="numPages">
        <button class="pdf-btn" :disabled="current <= 1" @click="goto(current - 1)"><ChevronUp :size="14" /></button>
        <span class="pdf-page-n tabular">{{ current }} / {{ numPages }}</span>
        <button class="pdf-btn" :disabled="current >= numPages" @click="goto(current + 1)"><ChevronDown :size="14" /></button>
        <span class="sep" />
        <button class="pdf-btn" :title="$t('viewer.zoomOut')" @click="zoom(1 / 1.2)"><ZoomOut :size="14" /></button>
        <span class="pdf-zoom tabular">{{ Math.round(scale * 100) }}%</span>
        <button class="pdf-btn" :title="$t('viewer.zoomIn')" @click="zoom(1.2)"><ZoomIn :size="14" /></button>
        <button class="pdf-btn" :class="{ on: fitWidth }" :title="$t('viewer.fitWidth')" @click="fit"><ScanLine :size="14" /></button>
        <span class="sep" />
      </template>
      <a class="pdf-btn" :href="src" download :title="$t('common.download')"><Download :size="14" /></a>
      <button class="pdf-btn" :title="$t('viewer.fullscreen')" @click="toggleFullscreen"><Minimize v-if="fullscreen" :size="14" /><Maximize v-else :size="14" /></button>
    </div>
    <div ref="scroller" class="pdf-scroll">
      <div v-if="loading" class="pdf-state"><UiSpinner /></div>
      <div v-else-if="error" class="pdf-state"><FileWarning :size="22" /> <span>{{ $t('viewer.pdfError') }}</span> <a :href="src" target="_blank" rel="noopener">{{ $t('common.open') }}</a></div>
      <div ref="pagesEl" class="pdf-pages">
        <div v-for="n in numPages" :key="n" class="pdf-page" :data-page="n" :style="pageStyle"><canvas /></div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.pdf { display: flex; flex-direction: column; border-radius: var(--r-lg); overflow: hidden; background: var(--bg-column); box-shadow: var(--shadow-card); margin: 1em 0; }
.pdf.fullscreen { border-radius: 0; margin: 0; background: var(--bg); }
.pdf-bar { display: flex; align-items: center; gap: 2px; height: 38px; padding: 0 6px 0 12px; background: var(--bg-panel); border-bottom: 1px solid var(--border); flex: none; font-size: var(--fs-sm); }
.pdf-name { font-weight: 500; min-width: 0; color: var(--text-2); }
.pdf-btn { width: 28px; height: 28px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-2); }
.pdf-btn:hover:not(:disabled) { background: var(--bg-hover); color: var(--text); }
.pdf-btn:disabled { opacity: 0.35; }
.pdf-btn.on { color: var(--accent-text); }
.pdf-page-n, .pdf-zoom { color: var(--text-3); min-width: 48px; text-align: center; }
.sep { width: 1px; height: 16px; background: var(--border-strong); margin: 0 4px; }
.pdf-scroll { flex: 1; overflow: auto; position: relative; }
.pdf-pages { display: flex; flex-direction: column; align-items: center; gap: 12px; padding: 16px; min-width: max-content; }
.pdf-page { background: #fff; box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3); border-radius: 2px; flex: none; }
.pdf-page canvas { width: 100%; height: 100%; display: block; }
.pdf-state { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; gap: 10px; color: var(--text-3); }
.pdf-state a { color: var(--accent-text); }
</style>
