<script setup lang="ts">
import type { Component } from 'vue'
import { FileText, BookCheck, CheckSquare, Kanban, CalendarDays, FolderCode, Search, Paperclip, Clock, Star } from 'lucide-vue-next'
import { useQuery } from '@tanstack/vue-query'
import { refDebounced } from '@vueuse/core'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useWorkspaces } from '~/composables/useWorkspace'
import { usePages } from '~/composables/usePages'
import type { AiAttachKind, AiAttachment } from '~/composables/useAi'
import type { SearchResult } from '~/types'
import { isReadable } from '~/utils/chatFiles'
import { fileIcon } from '~/utils/fileIcons'
import { groupByKind, emptyQuerySections, flatten, hitPlace, pushRecent, parseRecents, type PickerEntry, type PickerSection, type RecentItem } from '~/utils/pickerSearch'

/**
 * Palette de recherche pour joindre un élément à un message : pages, entraînements, tâches, fichiers du Drive,
 * tableaux, événements et projets de tous vos espaces. Vide : vos derniers éléments joints et vos favoris.
 * Résultats groupés par type, aperçu de l'élément survolé, ↑ ↓ pour naviguer, Entrée ou Tab pour joindre.
 */
const open = defineModel<boolean>('open', { default: false })
const query = defineModel<string>('query', { default: '' })
const emit = defineEmits<{ pick: [AiAttachment] }>()
const { t } = useI18n()
const uid = useId()

const KINDS: Record<AiAttachKind, { icon: Component; types: SearchResult['type'][] }> = {
  page: { icon: FileText, types: ['page'] },
  study: { icon: BookCheck, types: ['study'] },
  task: { icon: CheckSquare, types: ['task'] },
  file: { icon: Paperclip, types: ['file'] },
  board: { icon: Kanban, types: ['board'] },
  event: { icon: CalendarDays, types: ['event'] },
  project: { icon: FolderCode, types: ['project'] },
}
const kindOf = (r: SearchResult): AiAttachKind | null => {
  const k = (Object.entries(KINDS).find(([, v]) => v.types.includes(r.type))?.[0] as AiAttachKind | undefined) ?? null
  // Un fichier que l'assistant ne sait pas lire (image, archive…) n'est pas proposé.
  return k === 'file' && !isReadable(r.title) ? null : k
}

const filter = ref<'all' | AiAttachKind>('all')
const filters = computed(() => [{ value: 'all' as const, label: t('ai.picker.all') }, ...(Object.keys(KINDS) as AiAttachKind[]).map((k) => ({ value: k, icon: KINDS[k].icon, title: t(`ai.picker.kind.${k}`) }))])

const { data: workspaces } = useWorkspaces()
const { favorites } = usePages()
const multiWorkspace = computed(() => (workspaces.value?.length ?? 0) > 1)
const debounced = refDebounced(query, 180)
const searching = computed(() => !!debounced.value.trim())
const input = ref<{ focus: () => void } | null>(null)

function toEntry(r: SearchResult, kind: AiAttachKind, ws: string, wsName: string): PickerEntry {
  const place = kind === 'file'
    ? [multiWorkspace.value ? wsName : '', ...(r.folder ? r.folder.split('/') : [])].filter(Boolean).join(' › ')
    : hitPlace({ kind, ws, wsName, r }, { multiWorkspace: multiWorkspace.value })
  return { key: `${kind}:${r.id}`, kind, id: r.id, ws, title: r.title, place, excerpt: (r.snippet ?? '').trim() }
}

const { data: found, isFetching } = useQuery({
  queryKey: computed(() => ['ai', 'attach-search', debounced.value.trim(), (workspaces.value ?? []).map((w) => w.id).join(',')]),
  queryFn: async (): Promise<PickerEntry[]> => {
    const q = debounced.value.trim()
    const out = await Promise.all((workspaces.value ?? []).map(async (w) => {
      try {
        const r = await api<{ results: SearchResult[] }>(`/api/workspaces/${w.id}/search?q=${encodeURIComponent(q)}`, { silent: true })
        return r.results.flatMap((x) => { const kind = kindOf(x); return kind ? [toEntry(x, kind, w.id, w.name)] : [] })
      } catch { return [] }
    }))
    return out.flat()
  },
  enabled: computed(() => open.value && searching.value && !!workspaces.value?.length),
})

// --- Récents (mémorisés dans ce navigateur) et favoris ---------------------------------------------
const RECENTS_KEY = 'looty.ai.recents'
const recents = ref<RecentItem[]>([])
const asEntry = (r: RecentItem, place = ''): PickerEntry => ({ key: `${r.kind}:${r.id}`, kind: r.kind, id: r.id, ws: r.ws, title: r.title, place, excerpt: '' })
function remember(a: AiAttachment) {
  if (a.kind === 'file' || !a.ws) return // un fichier expire : inutile de le proposer de nouveau
  recents.value = pushRecent(recents.value, { kind: a.kind, id: a.id, ws: a.ws, title: a.title })
  try { localStorage.setItem(RECENTS_KEY, JSON.stringify(recents.value)) } catch { /* stockage indisponible */ }
}

const sections = computed<PickerSection<PickerEntry>[]>(() => {
  if (searching.value) return groupByKind(found.value ?? [], filter.value)
  const fav = (favorites.value ?? []).map((p) => asEntry({ kind: 'page', id: p.id, ws: p.workspace_id, title: p.title }))
  const rec = recents.value.filter((r) => filter.value === 'all' || r.kind === filter.value).map((r) => asEntry(r))
  return emptyQuerySections(rec, filter.value === 'all' || filter.value === 'page' ? fav : [])
})
const flat = computed(() => flatten(sections.value))
const active = ref(0)
const current = computed(() => flat.value[active.value])
watch([flat, open], () => { active.value = 0 })
watch(open, (v) => {
  if (!v) return
  filter.value = 'all'
  try { recents.value = parseRecents(localStorage.getItem(RECENTS_KEY)) } catch { recents.value = [] }
  nextTick(() => setTimeout(() => input.value?.focus(), 30))
})

const iconOf = (e: PickerEntry): Component => (e.kind === 'file' ? fileIcon(e.title) : KINDS[e.kind as AiAttachKind]?.icon ?? FileText)
const optionId = (e: PickerEntry, section: string) => `${uid}-${section}-${e.key}`
const activeId = computed(() => {
  const e = current.value
  if (!e) return undefined
  const s = sections.value.find((x) => x.items.includes(e))
  return s ? optionId(e, s.id) : undefined
})
const sectionLabel = (id: string) => (id === 'recent' ? t('ai.picker.recent') : id === 'favorite' ? t('ai.picker.favorites') : t(`ai.picker.group.${id}`))
const sectionIcon = (id: string) => (id === 'recent' ? Clock : id === 'favorite' ? Star : null)
const showNone = computed(() => searching.value && !isFetching.value && !flat.value.length)
const status = computed(() => (showNone.value ? t('ai.picker.none') : flat.value.length ? t('ai.picker.count', { count: flat.value.length }) : ''))

function choose(e: PickerEntry | undefined) {
  if (!e) return
  const a: AiAttachment = { kind: e.kind as AiAttachKind, id: e.id, title: e.title, ws: e.ws }
  remember(a)
  emit('pick', a)
  query.value = ''
  open.value = false
}
function move(delta: number) {
  const n = flat.value.length
  if (!n) return
  active.value = (active.value + delta + n) % n
  nextTick(() => document.getElementById(activeId.value ?? '')?.scrollIntoView({ block: 'nearest' }))
}
function onKey(e: KeyboardEvent) {
  if (e.isComposing) return
  if (e.key === 'ArrowDown') { e.preventDefault(); move(1) }
  else if (e.key === 'ArrowUp') { e.preventDefault(); move(-1) }
  else if ((e.key === 'Enter' || (e.key === 'Tab' && !e.shiftKey)) && current.value) { e.preventDefault(); choose(current.value) }
  else if (e.key === 'Escape') open.value = false
}
</script>

<template>
  <UiPopover v-model:open="open" :width="560" placement="top-start">
    <template #trigger><slot /></template>
    <div class="pk" @keydown="onKey">
      <UiInput
        ref="input" v-model="query" :icon="Search" size="sm" :placeholder="t('ai.picker.search')"
        role="combobox" aria-autocomplete="list" aria-expanded="true" :aria-controls="`${uid}-list`" :aria-activedescendant="activeId" :aria-label="t('ai.picker.search')"
      />
      <UiSegmented v-model="filter" :options="filters" class="pk-filter" />
      <div class="pk-body">
        <div :id="`${uid}-list`" class="pk-list" role="listbox" :aria-label="t('ai.picker.results')">
          <p v-if="showNone" class="pk-hint" role="presentation">{{ t('ai.picker.none') }}</p>
          <p v-else-if="searching && isFetching && !flat.length" class="pk-hint" role="presentation"><UiSpinner /></p>
          <p v-else-if="!searching && !flat.length" class="pk-hint" role="presentation">{{ t('ai.picker.hint') }}</p>
          <div v-for="s in sections" :key="s.id" role="group" :aria-labelledby="`${uid}-h-${s.id}`">
            <div :id="`${uid}-h-${s.id}`" class="pk-head">
              <component :is="sectionIcon(s.id)" v-if="sectionIcon(s.id)" :size="11" />
              <component :is="KINDS[s.id as AiAttachKind].icon" v-else-if="KINDS[s.id as AiAttachKind]" :size="11" />
              {{ sectionLabel(s.id) }}
            </div>
            <button
              v-for="e in s.items" :id="optionId(e, s.id)" :key="e.key" type="button" tabindex="-1" class="pk-row" :class="{ on: e === current }"
              role="option" :aria-selected="e === current" @mouseenter="active = flat.indexOf(e)" @click="choose(e)"
            >
              <component :is="iconOf(e)" :size="14" class="pk-icon" />
              <span class="pk-text">
                <span class="pk-title truncate">{{ e.title || t('docs.untitled') }}</span>
                <span v-if="e.place" class="pk-path truncate">{{ e.place }}</span>
              </span>
            </button>
          </div>
        </div>
        <aside v-if="current" class="pk-preview" :aria-label="t('ai.picker.preview')">
          <component :is="iconOf(current)" :size="18" class="pk-icon" />
          <strong class="pk-p-title">{{ current.title || t('docs.untitled') }}</strong>
          <span class="pk-p-kind">{{ t(`ai.picker.kind.${current.kind}`) }}</span>
          <span v-if="current.place" class="pk-p-place">{{ current.place }}</span>
          <p v-if="current.excerpt" class="pk-p-excerpt">{{ current.excerpt }}</p>
          <p v-else class="pk-p-none">{{ t('ai.picker.noExcerpt') }}</p>
        </aside>
      </div>
      <div class="pk-foot"><span><kbd>↑</kbd><kbd>↓</kbd> {{ t('ai.picker.navigate') }}</span><span><kbd>Tab</kbd> / <kbd>↵</kbd> {{ t('ai.picker.attachAction') }}</span></div>
      <p class="sr-only" role="status" aria-live="polite">{{ status }}</p>
    </div>
  </UiPopover>
</template>

<style scoped>
.pk { display: flex; flex-direction: column; gap: 8px; padding: 8px; }
.pk-filter { align-self: flex-start; }
.pk-body { display: grid; grid-template-columns: minmax(0, 1fr) 190px; gap: 8px; min-height: 0; }
.pk-list { display: flex; flex-direction: column; max-height: 320px; overflow-y: auto; }
.pk-hint { color: var(--text-3); font-size: var(--fs-sm); padding: 8px; margin: 0; display: flex; justify-content: center; }
.pk-head { display: flex; align-items: center; gap: 5px; padding: 6px 8px 2px; font-size: var(--fs-xs); font-weight: 600; color: var(--text-4); text-transform: uppercase; letter-spacing: 0.04em; }
.pk-row { display: flex; align-items: center; gap: 8px; width: 100%; padding: 6px 8px; border-radius: var(--r-sm); text-align: left; color: var(--text); }
.pk-row.on { background: var(--bg-hover); }
.pk-icon { flex: none; color: var(--text-3); }
.pk-text { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.pk-title { font-size: var(--fs-sm); }
.pk-path { font-size: var(--fs-xs); color: var(--text-3); }
.pk-preview { display: flex; flex-direction: column; gap: 4px; padding: 10px; border-radius: var(--r-md); background: var(--bg-elevated); border: 1px solid var(--border); overflow: hidden; max-height: 320px; }
.pk-p-title { font-size: var(--fs-sm); line-height: 1.3; word-break: break-word; }
.pk-p-kind { font-size: var(--fs-xs); color: var(--text-4); }
.pk-p-place { font-size: var(--fs-xs); color: var(--text-3); word-break: break-word; }
.pk-p-excerpt { margin: 4px 0 0; font-size: var(--fs-xs); color: var(--text-2); line-height: 1.5; overflow: hidden; display: -webkit-box; -webkit-line-clamp: 8; line-clamp: 8; -webkit-box-orient: vertical; }
.pk-p-none { margin: 4px 0 0; font-size: var(--fs-xs); color: var(--text-4); }
.pk-foot { display: flex; gap: 12px; color: var(--text-4); font-size: var(--fs-xs); }
.pk-foot kbd { padding: 0 4px; margin-right: 2px; border: 1px solid var(--border); border-radius: var(--r-sm); font: inherit; }
@media (max-width: 640px) {
  .pk-body { grid-template-columns: minmax(0, 1fr); }
  .pk-preview, .pk-foot { display: none; }
}
</style>
