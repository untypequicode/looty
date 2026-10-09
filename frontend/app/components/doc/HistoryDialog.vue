<script setup lang="ts">
import { EditorContent, Editor as VueEditor, type Editor } from '@tiptap/vue-3'
import * as Y from 'yjs'
import { History, RotateCcw, Bookmark, Pencil, Save } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api, errorMessage } from '~/composables/api'
import { useWorkspace } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'
import { revisionExtensions } from '~/editor/extensions'
import { upgradeSources } from '~/editor/source'

/**
 * Historique des versions d'une page : liste des sessions d'édition et versions nommées,
 * aperçu fidèle (reconstruit depuis l'état Yjs) et restauration dans l'éditeur collaboratif.
 */
interface Revision { id: string; kind: 'auto' | 'manual' | 'restore'; label: string | null; title: string; created_by: string | null; author_name: string | null; created_at: string; updated_at: string; size: number }

const open = defineModel<boolean>('open', { default: false })
const props = defineProps<{ pageId: string; editor: Editor | null; canEdit: boolean; currentTitle: string }>()
const emit = defineEmits<{ restoreTitle: [title: string] }>()
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
watch(open, (o) => { if (!o) selected.value = null })
const current = computed(() => revisions.value?.find((r) => r.id === selected.value))

// Aperçu : document Yjs reconstruit, éditeur en lecture seule recréé à chaque version.
const preview = shallowRef<Editor | null>(null)
const loadingPreview = ref(false)
watch(selected, async (id) => {
  preview.value?.destroy()
  preview.value = null
  if (!id) return
  loadingPreview.value = true
  try {
    const r = await api<{ yjs_state: string; title: string }>(`/api/pages/${props.pageId}/revisions/${id}`)
    if (selected.value !== id) return
    const doc = new Y.Doc()
    Y.applyUpdate(doc, Uint8Array.from(atob(r.yjs_state), (c) => c.charCodeAt(0)))
    preview.value = new VueEditor({ extensions: revisionExtensions(doc), editable: false, editorProps: { attributes: { class: 'prose doc-prose' } } })
  } catch (e) {
    toast.error(errorMessage(e))
  } finally { loadingPreview.value = false }
})
onBeforeUnmount(() => preview.value?.destroy())

const kindLabel = (r: Revision) => (r.label ? r.label : r.kind === 'restore' ? t('history.beforeRestore') : null)
const author = (r: Revision) => member(r.created_by ?? '')?.name ?? r.author_name ?? t('common.unknown')

const restoring = ref(false)
async function restore() {
  const r = current.value
  if (!r || !preview.value || !props.editor) return
  if (!(await confirmDialog({ title: t('history.restoreTitle'), message: t('history.restoreHint', { time: formatDateTime(r.updated_at) }), confirmLabel: t('history.restore') }))) return
  restoring.value = true
  try {
    // Filet de sécurité : l'état actuel devient une version « avant restauration ».
    await api(`/api/pages/${props.pageId}/revisions`, { method: 'POST', body: { kind: 'restore' } })
    // Remplacement dans l'éditeur collaboratif : propagé à tous les participants.
    props.editor.commands.setContent(upgradeSources(preview.value.getJSON()), { emitUpdate: true })
    if (r.title && r.title !== props.currentTitle) emit('restoreTitle', r.title)
    toast.success(t('history.restored'))
    qc.invalidateQueries({ queryKey: key.value })
    open.value = false
  } catch (e) { toast.error(errorMessage(e)) } finally { restoring.value = false }
}

async function saveVersion() {
  const label = window.prompt(t('history.namePrompt'), '')
  if (label === null) return
  await api(`/api/pages/${props.pageId}/revisions`, { method: 'POST', body: { kind: 'manual', label: label || t('history.namedDefault') } })
  qc.invalidateQueries({ queryKey: key.value })
  toast.success(t('history.saved'))
}
async function rename(r: Revision) {
  const label = window.prompt(t('history.namePrompt'), r.label ?? '')
  if (label === null) return
  await api(`/api/pages/${props.pageId}/revisions/${r.id}`, { method: 'PATCH', body: { label: label || null } })
  qc.invalidateQueries({ queryKey: key.value })
}
</script>

<template>
  <UiDialog v-model:open="open" :title="t('history.title')" :width="1080">
    <div class="hd">
      <div class="hd-preview">
        <div v-if="loadingPreview && !preview" class="hd-empty"><UiSpinner /></div>
        <template v-else-if="preview && current">
          <h1 class="hd-title">{{ current.title || t('docs.untitled') }}</h1>
          <EditorContent :editor="preview" />
        </template>
        <div v-else-if="!isLoading" class="hd-empty"><History :size="22" /> {{ t('history.empty') }}</div>
      </div>
      <aside class="hd-list">
        <UiButton v-if="canEdit" size="sm" :icon="Save" class="hd-save" @click="saveVersion">{{ t('history.saveVersion') }}</UiButton>
        <div v-if="isLoading" class="hd-empty"><UiSpinner /></div>
        <button v-for="(r, i) in revisions ?? []" :key="r.id" class="hd-item" :class="{ on: r.id === selected }" @click="selected = r.id">
          <span class="hd-when" :title="formatDateTime(r.updated_at)">{{ i === 0 ? t('history.latest') : relativeTime(r.updated_at) }}</span>
          <span class="hd-meta">
            <UiAvatar :name="author(r)" :size="16" />
            <span class="truncate">{{ author(r) }}</span>
          </span>
          <span v-if="kindLabel(r)" class="hd-badge" :class="r.kind"><Bookmark :size="11" /> {{ kindLabel(r) }}</span>
          <span v-if="canEdit" class="hd-rename" role="button" :title="t('history.rename')" @click.stop="rename(r)"><Pencil :size="12" /></span>
        </button>
      </aside>
    </div>
    <template #footer>
      <span class="hd-hint">{{ t('history.hint') }}</span>
      <UiButton variant="ghost" @click="open = false">{{ t('common.close') }}</UiButton>
      <UiButton v-if="canEdit" variant="primary" :icon="RotateCcw" :loading="restoring" :disabled="!preview || !editor || revisions?.[0]?.id === selected" @click="restore">{{ t('history.restore') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.hd { display: grid; grid-template-columns: minmax(0, 1fr) 260px; gap: 16px; height: min(70vh, 720px); }
.hd-preview { overflow-y: auto; padding: 8px 24px 24px; border-radius: var(--r-md); background: var(--bg); box-shadow: inset 0 0 0 1px var(--border); }
.hd-title { font-family: var(--font-display); font-size: 30px; letter-spacing: -0.02em; margin: 16px 0 12px; }
.hd-list { overflow-y: auto; display: flex; flex-direction: column; gap: 2px; }
.hd-save { margin-bottom: 8px; }
.hd-item { position: relative; display: flex; flex-direction: column; align-items: flex-start; gap: 3px; padding: 8px 10px; border-radius: var(--r-sm); text-align: left; }
.hd-item:hover { background: var(--bg-hover); }
.hd-item.on { background: var(--accent-soft); }
.hd-when { font-weight: 600; font-size: var(--fs-sm); }
.hd-meta { display: flex; align-items: center; gap: 6px; font-size: var(--fs-xs); color: var(--text-3); max-width: 100%; }
.hd-badge { display: inline-flex; align-items: center; gap: 4px; font-size: 11px; font-weight: 600; padding: 1px 6px; border-radius: var(--r-full); background: var(--bg-active); color: var(--text-2); }
.hd-badge.manual { background: color-mix(in srgb, var(--accent) 18%, transparent); color: var(--accent-text, var(--accent)); }
.hd-rename { position: absolute; top: 8px; right: 8px; width: 22px; height: 22px; display: grid; place-items: center; border-radius: var(--r-xs); color: var(--text-3); opacity: 0; }
.hd-item:hover .hd-rename { opacity: 1; }
.hd-rename:hover { background: var(--bg-active); color: var(--text); }
.hd-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; height: 100%; color: var(--text-3); padding: 24px; }
.hd-hint { margin-right: auto; font-size: var(--fs-sm); color: var(--text-3); }
@media (max-width: 760px) { .hd { grid-template-columns: 1fr; grid-template-rows: 1fr 200px; } }
</style>
