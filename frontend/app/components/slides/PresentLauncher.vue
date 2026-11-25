<script setup lang="ts">
import type * as Y from 'yjs'
import type { HocuspocusProvider } from '@hocuspocus/provider'
import { MonitorPlay, Play, Printer, Radio } from 'lucide-vue-next'
import { slideIds } from '~/slides/model'
import { pickPresenter, type Presenter } from '~/slides/presentSync'
import { useI18n } from '~/composables/useI18n'
import PresentShow from './PresentShow.vue'
import PresentPrint from './PresentPrint.vue'

/**
 * Boutons de présentation du bandeau de l'éditeur (issue #303) : lancer le diaporama (depuis la
 * diapositive affichée), ouvrir la vue présentateur dans une seconde fenêtre, imprimer, et suivre une
 * personne qui présente déjà. Le diaporama réutilise le document ouvert (aucune seconde connexion).
 */
const props = defineProps<{ doc: Y.Doc; provider: HocuspocusProvider; pageId: string; currentId: string; canEdit: boolean; ready: boolean }>()
const { t } = useI18n()
const route = useRoute()
const open = ref(false)
const startId = ref<string | null>(null)
const follow = ref(false)
const presenter = ref<Presenter | null>(null)
const printing = ref(false)

const base = computed(() => `/w/${route.params.ws}/present/${props.pageId}`)
const presenterUrl = computed(() => `${base.value}?mode=presenter`)
const hasSlides = computed(() => props.ready && slideIds(props.doc).length > 0)

function readPresenter() {
  const aw = props.provider.awareness
  presenter.value = aw ? pickPresenter(aw.getStates().entries(), aw.clientID) : null
}
props.provider.on('awarenessUpdate', readPresenter)
onBeforeUnmount(() => props.provider.off('awarenessUpdate', readPresenter))

function start(withPresenter = false, following = false) {
  startId.value = props.currentId
  follow.value = following
  if (withPresenter) window.open(presenterUrl.value, 'looty-presenter', 'popup,width=1200,height=800')
  open.value = true
}

// Impression : toutes les diapositives, une par page (les raccourcis du navigateur la déclenchent aussi).
async function print() {
  printing.value = true
  await nextTick()
  await new Promise((r) => requestAnimationFrame(() => r(null)))
  window.print()
}
function beforePrint() { printing.value = true }
function afterPrint() { printing.value = false }
onMounted(() => { window.addEventListener('beforeprint', beforePrint); window.addEventListener('afterprint', afterPrint) })
onBeforeUnmount(() => { window.removeEventListener('beforeprint', beforePrint); window.removeEventListener('afterprint', afterPrint) })
</script>

<template>
  <div class="pl" role="group" :aria-label="t('present.panel')">
    <button type="button" class="pb primary" data-present-start :disabled="!hasSlides" :title="t('present.startFrom')" @click="start()"><Play :size="15" /> {{ t('present.start') }}</button>
    <button type="button" class="pb" data-present-presenter :disabled="!hasSlides" :title="t('present.presenterWindow')" :aria-label="t('present.presenterWindow')" @click="start(true)"><MonitorPlay :size="15" /><span>{{ t('present.presenterView') }}</span></button>
    <button type="button" class="pb" :disabled="!hasSlides" :title="t('present.print')" :aria-label="t('present.print')" @click="print"><Printer :size="15" /></button>
    <span v-if="presenter && !open" class="pf" role="status">
      <Radio :size="14" /> {{ t('present.presenting', { name: presenter.name }) }}
      <button type="button" class="pb" data-present-follow @click="start(false, true)">{{ t('present.follow') }}</button>
    </span>
    <Teleport to="body">
      <PresentShow v-if="open" :doc="doc" :page-id="pageId" :provider="provider" :can-share="canEdit" :start-slide-id="startId" :start-following="follow" :presenter-url="presenterUrl" @close="open = false" />
      <PresentPrint v-if="printing" :doc="doc" />
    </Teleport>
  </div>
</template>

<style scoped>
.pl { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; margin-left: auto; }
.pb { display: inline-flex; align-items: center; gap: 6px; height: 30px; padding: 0 10px; border-radius: var(--r-sm); border: 1px solid var(--border); color: var(--text); font-size: var(--fs-sm); }
.pb:hover:not(:disabled) { background: var(--bg-hover); }
.pb:disabled { opacity: 0.5; }
.pb.primary { background: var(--accent); border-color: var(--accent); color: var(--accent-contrast, #fff); }
.pb:focus-visible { outline: 2px solid var(--border-focus); outline-offset: 1px; }
.pf { display: inline-flex; align-items: center; gap: 6px; font-size: var(--fs-sm); color: var(--text-2); }
@media (max-width: 700px) { .pb span { display: none; } }
</style>
