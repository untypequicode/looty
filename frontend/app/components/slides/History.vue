<script setup lang="ts">
import * as Y from 'yjs'
import { History, RotateCcw, Bookmark, Pencil, Save } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api, errorMessage } from '~/composables/api'
import { useWorkspace } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog, promptDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'
import { decodeState, diffSlides, listSlides, slideSize, type SlideChange } from '~/slides/model'
import SlideView from './SlideView.vue'
import LazyThumb from './LazyThumb.vue'

/**
 * Historique des versions d'une présentation : mêmes versions que les pages (une par session d'édition,
 * versions nommées, « avant restauration »), aperçu diapositive par diapositive en lecture seule, et
 * restauration comme nouvel état : l'état actuel est d'abord gardé comme version, puis le contenu de la
 * version remplace le document collaboratif (les autres personnes le voient arriver ; Ctrl+Z l'annule).
 */
interface Revision { id: string; kind: 'auto' | 'manual' | 'restore'; label: string | null; title: string; created_by: string | null; author_name: string | null; created_at: string; updated_at: string; size: number }

const open = defineModel<boolean>('open', { default: false })
const props = defineProps<{ pageId: string; doc: Y.Doc; canEdit: boolean }>()
const emit = defineEmits<{ restore: [snapshot: Y.Doc] }>()
const { member } = useWorkspace()
const { t, relativeTime, formatDateTime } = useI18n()
const qc = useQueryClient()

const key = computed(() => ['page-revisions', props.pageId])
const { data: revisions, isLoading } = useQuery({
  queryKey: key,
  queryFn: () => api<Revision[]>(`/api/pages/${props.pageId}/revisions`),
  enabled: open,
  staleTime: 0,
})
const selected = ref<string | null>(null)
watch(revisions, (list) => { if (list?.length && !list.some((r) => r.id === selected.value)) selected.value = list[0]!.id })
watch(open, (o) => { if (!o) { selected.value = null; snap.value = null } })
const current = computed(() => revisions.value?.find((r) => r.id === selected.value))

// Aperçu : document Yjs reconstruit de la version (en mémoire seulement).
const snap = shallowRef<Y.Doc | null>(null)
const loading = ref(false)
const shown = ref('')
watch(selected, async (id) => {
  snap.value?.destroy()
  snap.value = null
  if (!id) return
  loading.value = true
  try {
    const r = await api<{ yjs_state: string }>(`/api/pages/${props.pageId}/revisions/${id}`)
    if (selected.value !== id) return
    snap.value = decodeState(r.yjs_state)
    shown.value = listSlides(snap.value)[0]?.id ?? ''
  } catch (e) {
    toast.error(errorMessage(e))
  } finally { loading.value = false }
})
onBeforeUnmount(() => snap.value?.destroy())

const slides = computed(() => (snap.value ? listSlides(snap.value) : []))
const size = computed(() => (snap.value ? slideSize(snap.value) : { w: 1920, h: 1080 }))
const slide = computed(() => slides.value.find((s) => s.id === shown.value) ?? slides.value[0] ?? null)
/** Par rapport au document actuel : diapositives modifiées, absentes aujourd'hui (added) ou présentes aujourd'hui seulement (removed). */
const changes = computed<Map<string, SlideChange>>(() => (snap.value ? diffSlides(props.doc, snap.value) : new Map()))
const removedNow = computed(() => [...changes.value.values()].filter((c) => c === 'removed').length)
const changedCount = computed(() => [...changes.value.values()].filter((c) => c !== 'removed').length)

const kindLabel = (r: Revision) => (r.label ? r.label : r.kind === 'restore' ? t('history.beforeRestore') : null)
const author = (r: Revision) => member(r.created_by ?? '')?.name ?? r.author_name ?? t('common.unknown')

const restoring = ref(false)
async function restore() {
  const r = current.value
  if (!r || !snap.value) return
  if (!(await confirmDialog({ title: t('history.restoreTitle'), message: t('slides.collab.restoreHint', { time: formatDateTime(r.updated_at) }), confirmLabel: t('history.restore') }))) return
  restoring.value = true
  try {
    // Filet de sécurité : l'état actuel devient une version « avant restauration ».
    await api(`/api/pages/${props.pageId}/revisions`, { method: 'POST', body: { kind: 'restore' } })
    emit('restore', snap.value)
    toast.success(t('history.restored'))
    qc.invalidateQueries({ queryKey: key.value })
    open.value = false
  } catch (e) { toast.error(errorMessage(e)) } finally { restoring.value = false }
}
async function saveVersion() {
  const label = await promptDialog({ title: t('history.namePrompt'), initial: '', confirmLabel: t('common.save') })
  if (label === null) return
  await api(`/api/pages/${props.pageId}/revisions`, { method: 'POST', body: { kind: 'manual', label: label || t('history.namedDefault') } })
  qc.invalidateQueries({ queryKey: key.value })
  toast.success(t('history.saved'))
}
async function rename(r: Revision) {
  const label = await promptDialog({ title: t('history.namePrompt'), initial: r.label ?? '', confirmLabel: t('common.save') })
  if (label === null) return
  await api(`/api/pages/${props.pageId}/revisions/${r.id}`, { method: 'PATCH', body: { label: label || null } })
  qc.invalidateQueries({ queryKey: key.value })
}
</script>

<template>
  <UiDialog v-model:open="open" :title="t('history.title')" :width="1120">
    <div class="hd" data-slides-history>
      <div class="hd-strip" :aria-label="t('slides.slides')">
        <button v-for="(s, i) in slides" :key="s.id" type="button" class="hd-thumb" :class="{ on: s.id === slide?.id }" :aria-label="t('slides.slideN', { n: i + 1 })" :aria-pressed="s.id === slide?.id" @click="shown = s.id">
          <span class="hd-n">{{ i + 1 }}</span>
          <LazyThumb v-if="snap" :doc="snap" :slide="s" :size="size" :tick="0" />
          <span v-if="changes.get(s.id)" class="hd-chg" :class="changes.get(s.id)">{{ changes.get(s.id) === 'added' ? t('slides.collab.notNow') : t('slides.collab.differs') }}</span>
        </button>
      </div>
      <div class="hd-preview">
        <div v-if="loading && !snap" class="hd-empty"><UiSpinner /></div>
        <template v-else-if="snap && slide">
          <div class="hd-slide"><SlideView :doc="snap" :slide="slide" :size="size" :tick="0" /></div>
          <p class="hd-sum">{{ t('slides.collab.compare', { changed: changedCount, removed: removedNow }) }}</p>
        </template>
        <div v-else-if="!isLoading" class="hd-empty"><History :size="22" /> {{ t('history.empty') }}</div>
      </div>
      <aside class="hd-list">
        <UiButton v-if="canEdit" size="sm" :icon="Save" class="hd-save" @click="saveVersion">{{ t('history.saveVersion') }}</UiButton>
        <div v-if="isLoading" class="hd-empty"><UiSpinner /></div>
        <div v-for="(r, i) in revisions ?? []" :key="r.id" class="hd-row">
          <button class="hd-item" :class="{ on: r.id === selected }" :aria-pressed="r.id === selected" data-revision @click="selected = r.id">
            <span class="hd-when" :title="formatDateTime(r.updated_at)">{{ i === 0 ? t('history.latest') : relativeTime(r.updated_at) }}</span>
            <span class="hd-meta">
              <UiAvatar :name="author(r)" :size="16" />
              <span class="truncate">{{ author(r) }}</span>
            </span>
            <span v-if="kindLabel(r)" class="hd-badge" :class="r.kind"><Bookmark :size="11" /> {{ kindLabel(r) }}</span>
          </button>
          <UiButton v-if="canEdit" class="hd-rename" size="xs" variant="ghost" :icon="Pencil" :aria-label="t('history.rename')" @click="rename(r)" />
        </div>
      </aside>
    </div>
    <template #footer>
      <span class="hd-hint">{{ t('slides.collab.historyHint') }}</span>
      <UiButton variant="ghost" @click="open = false">{{ t('common.close') }}</UiButton>
      <UiButton v-if="canEdit" variant="primary" :icon="RotateCcw" :loading="restoring" :disabled="!snap || !changes.size" data-restore @click="restore">{{ t('history.restore') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.hd { display: grid; grid-template-columns: 132px minmax(0, 1fr) 250px; gap: 14px; height: min(70vh, 640px); }
.hd-strip { overflow-y: auto; display: flex; flex-direction: column; gap: 8px; padding: 2px; }
.hd-thumb { position: relative; display: block; width: 100%; padding: 0; border-radius: var(--r-sm); box-shadow: 0 0 0 1px var(--border-strong); overflow: hidden; text-align: left; flex: none; }
.hd-thumb.on { box-shadow: 0 0 0 2px var(--accent); }
.hd-thumb:focus-visible { outline: 2px solid var(--border-focus); outline-offset: 2px; }
.hd-n { position: absolute; left: 3px; top: 2px; z-index: 2; font-size: var(--fs-xs); background: var(--bg-elevated); color: var(--text-2); padding: 0 4px; border-radius: var(--r-sm); }
.hd-chg { position: absolute; right: 3px; bottom: 3px; z-index: 2; font-size: 10px; font-weight: 700; padding: 0 5px; border-radius: var(--r-full); background: var(--accent); color: var(--text-on-accent); }
.hd-chg.added { background: var(--green); }
.hd-preview { display: flex; flex-direction: column; align-items: stretch; gap: 10px; overflow-y: auto; padding: 12px; border-radius: var(--r-md); background: var(--bg-column); box-shadow: inset 0 0 0 1px var(--border); }
.hd-slide { box-shadow: 0 0 0 1px var(--border-strong), 0 6px 22px rgb(0 0 0 / 0.2); }
.hd-sum { margin: 0; color: var(--text-3); font-size: var(--fs-sm); }
.hd-list { overflow-y: auto; display: flex; flex-direction: column; gap: 2px; }
.hd-save { margin-bottom: 8px; }
.hd-row { position: relative; }
.hd-item { width: 100%; display: flex; flex-direction: column; align-items: flex-start; gap: 3px; padding: 8px 10px; border-radius: var(--r-sm); text-align: left; }
.hd-item:hover { background: var(--bg-hover); }
.hd-item.on { background: var(--accent-soft); }
.hd-when { font-weight: 600; font-size: var(--fs-sm); }
.hd-meta { display: flex; align-items: center; gap: 6px; font-size: var(--fs-xs); color: var(--text-3); max-width: 100%; }
.hd-badge { display: inline-flex; align-items: center; gap: 4px; font-size: 11px; font-weight: 600; padding: 1px 6px; border-radius: var(--r-full); background: var(--bg-active); color: var(--text-2); }
.hd-badge.manual { background: color-mix(in srgb, var(--accent) 18%, transparent); color: var(--accent-text, var(--accent)); }
.hd-rename { position: absolute; top: 6px; right: 6px; opacity: 0; }
.hd-row:hover .hd-rename, .hd-row:focus-within .hd-rename { opacity: 1; }
@media (pointer: coarse) { .hd-rename { opacity: 1; } }
.hd-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; height: 100%; color: var(--text-3); padding: 24px; }
.hd-hint { margin-right: auto; font-size: var(--fs-sm); color: var(--text-3); }
@media (max-width: 860px) { .hd { grid-template-columns: 1fr; grid-template-rows: 90px 1fr 180px; } .hd-strip { flex-direction: row; } .hd-thumb { width: 120px; } }
</style>
