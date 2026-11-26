<script setup lang="ts">
import * as Y from 'yjs'
import { ChevronLeft, ChevronRight, Play, Printer, Repeat } from 'lucide-vue-next'
import { slideIds, slideSize } from '~/slides/model'
import { stepsOf } from '~/slides/present'
import { useI18n } from '~/composables/useI18n'
import PresentShow from './PresentShow.vue'
import PresentPrint from './PresentPrint.vue'
import PresentSlide from './PresentSlide.vue'

/**
 * Présentation lue par lien public, sans compte (issue #303) : état Yjs reçu du serveur (base64),
 * lecture seule, diaporama plein écran, boucle automatique (kiosque) et impression.
 */
const props = defineProps<{ token: string; title: string; state: string | null }>()
const { t } = useI18n()
const route = useRoute()
const doc = new Y.Doc()
if (props.state) {
  try { Y.applyUpdate(doc, Uint8Array.from(atob(props.state), (c) => c.charCodeAt(0))) } catch { /* état illisible : présentation vide */ }
}
const ids = slideIds(doc)
const steps = stepsOf(doc)
const size = slideSize(doc)
const cur = ref(0)
const open = ref(false)
const loop = ref(Number(route.query.kiosk) > 0)
const every = ref(Math.max(2, Number(route.query.kiosk) || 8))
const printing = ref(false)
const frameStyle = { aspectRatio: `${size.w} / ${size.h}` }
const announce = computed(() => t('present.slideOf', { n: cur.value + 1, total: ids.length }))
onMounted(() => { if (Number(route.query.kiosk) > 0 && ids.length) open.value = true })
async function print() {
  printing.value = true
  await nextTick()
  await new Promise((r) => requestAnimationFrame(() => r(null)))
  window.print()
}
const afterPrint = () => { printing.value = false }
onMounted(() => window.addEventListener('afterprint', afterPrint))
onBeforeUnmount(() => { window.removeEventListener('afterprint', afterPrint); doc.destroy() })
</script>

<template>
  <main class="pub">
    <h1>{{ title || t('slides.untitled') }}</h1>
    <p v-if="!ids.length" class="empty">{{ t('present.empty') }}</p>
    <template v-else>
      <div class="pub-frame" :style="frameStyle">
        <PresentSlide :key="ids[cur]" :doc="doc" :slide-id="ids[cur]!" :tick="0" :step="steps[cur] ?? 0" />
      </div>
      <div class="pub-bar" role="toolbar" :aria-label="t('present.controls')">
        <button type="button" class="b" :aria-label="t('present.prev')" :disabled="cur === 0" @click="cur--"><ChevronLeft :size="18" /></button>
        <span class="n" aria-live="polite">{{ announce }}</span>
        <button type="button" class="b" :aria-label="t('present.next')" :disabled="cur >= ids.length - 1" @click="cur++"><ChevronRight :size="18" /></button>
        <span class="sp" />
        <label class="loop"><input v-model="loop" type="checkbox" /> <Repeat :size="14" /> {{ t('present.kioskOn') }}</label>
        <label v-if="loop" class="loop">{{ t('present.kioskEvery') }} <input v-model.number="every" type="number" min="2" max="600" /></label>
        <button type="button" class="b wide primary" data-public-start @click="open = true"><Play :size="15" /> {{ t('present.startShow') }}</button>
        <button type="button" class="b" :title="t('present.print')" :aria-label="t('present.print')" @click="print"><Printer :size="16" /></button>
      </div>
    </template>
    <PresentShow v-if="open" :doc="doc" :page-id="token" :kiosk="loop ? every : 0" :start-slide-id="ids[cur]" @close="open = false" />
    <Teleport to="body"><PresentPrint v-if="printing" :doc="doc" /></Teleport>
  </main>
</template>

<style scoped>
.pub { max-width: 1100px; margin: 0 auto; padding: 24px 16px 48px; color: var(--text); }
h1 { font-size: var(--fs-xl, 24px); margin: 0 0 16px; }
.pub-frame { width: 100%; border: 1px solid var(--border); border-radius: 8px; overflow: hidden; background: var(--bg-elevated); }
.pub-bar { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; margin-top: 12px; }
.b { display: inline-flex; align-items: center; gap: 6px; height: 34px; padding: 0 10px; border: 1px solid var(--border); border-radius: var(--r-sm); color: var(--text); }
.b:hover:not(:disabled) { background: var(--bg-hover); }
.b:disabled { opacity: 0.4; }
.b.primary { background: var(--accent); border-color: var(--accent); color: var(--accent-contrast, #fff); }
.b:focus-visible, input:focus-visible { outline: 2px solid var(--border-focus); outline-offset: 1px; }
.sp { flex: 1; }
.n { font-variant-numeric: tabular-nums; color: var(--text-2); }
.loop { display: inline-flex; align-items: center; gap: 6px; font-size: var(--fs-sm); color: var(--text-2); }
.loop input[type='number'] { width: 60px; height: 28px; background: var(--bg-input); border: 1px solid var(--border); border-radius: var(--r-sm); color: var(--text); padding: 0 6px; }
.empty { color: var(--text-3); }
</style>
