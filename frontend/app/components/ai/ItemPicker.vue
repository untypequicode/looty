<script setup lang="ts">
import type { Component } from 'vue'
import { FileText, BookCheck, CheckSquare, Kanban, CalendarDays, FolderCode, Search, Paperclip, Clock, Star, GraduationCap, ChevronRight, ArrowLeft } from 'lucide-vue-next'
import { useQuery } from '@tanstack/vue-query'
import { refDebounced } from '@vueuse/core'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useWorkspaces } from '~/composables/useWorkspace'
import { usePages } from '~/composables/usePages'
import { fileSize } from '~/composables/useProjects'
import type { MoodleCourse, MoodleCourseContent, MoodleOverview } from '~/composables/useMoodle'
import type { AiAttachKind, AiAttachment } from '~/composables/useAi'
import type { SearchResult } from '~/types'
import { isReadable } from '~/utils/chatFiles'
import { isMoodleKind, moodleCourseAttachment, moodleFileAttachment } from '~/utils/assistantDraft'
import { courseEntries, fileEntries } from '~/utils/moodlePicker'
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
  moodle_course: { icon: GraduationCap, types: [] },
  moodle_file: { icon: GraduationCap, types: [] },
}
const kindOf = (r: SearchResult): AiAttachKind | null => {
  const k = (Object.entries(KINDS).find(([, v]) => v.types.includes(r.type))?.[0] as AiAttachKind | undefined) ?? null
  // Un fichier que l'assistant ne sait pas lire (image, archive…) n'est pas proposé.
  return k === 'file' && !isReadable(r.title) ? null : k
}

const filter = ref<string>('all')
// Un seul filtre « Moodle » pour les cours et leurs fichiers.
const filters = computed(() => [
  { value: 'all', label: t('ai.picker.all') },
  ...(Object.keys(KINDS) as AiAttachKind[]).filter((k) => !isMoodleKind(k)).map((k) => ({ value: k as string, icon: KINDS[k].icon, title: t(`ai.picker.kind.${k}`) })),
  { value: 'moodle', icon: GraduationCap, title: t('ai.picker.kind.moodle') },
])

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

// --- Moodle : cours de la connexion de la personne (par espace), fichiers d'un cours --------------------
interface MoodleSource { ws: string; wsName: string; courses: MoodleCourse[] }
const { data: moodle } = useQuery({
  queryKey: computed(() => ['ai', 'moodle-courses', (workspaces.value ?? []).map((w) => w.id).join(',')]),
  queryFn: async (): Promise<MoodleSource[]> => {
    const out = await Promise.all((workspaces.value ?? []).map(async (w): Promise<MoodleSource | null> => {
      try {
        const o = await api<MoodleOverview>(`/api/workspaces/${w.id}/moodle`, { silent: true })
        if (!o.url || !o.connection) return null
        return { ws: w.id, wsName: w.name, courses: await api<MoodleCourse[]>(`/api/workspaces/${w.id}/moodle/courses`, { silent: true }) }
      } catch { return null }
    }))
    return out.filter((s): s is MoodleSource => !!s)
  },
  enabled: computed(() => open.value && !!workspaces.value?.length),
  staleTime: 60_000,
  retry: false,
})
const moodleEntries = computed(() => (moodle.value ?? []).flatMap((s) => courseEntries(s.ws, s.wsName, s.courses, debounced.value, multiWorkspace.value)))

/** Cours dont on parcourt les fichiers (flèche → ou bouton ›). */
const drill = ref<{ ws: string; id: number; name: string } | null>(null)
const { data: courseContent, isFetching: drillFetching, error: drillError } = useQuery({
  queryKey: computed(() => ['moodle', drill.value?.ws ?? '', 'course', drill.value?.id ?? 0]),
  queryFn: () => api<MoodleCourseContent>(`/api/workspaces/${drill.value!.ws}/moodle/courses/${drill.value!.id}`, { silent: true }),
  enabled: computed(() => open.value && !!drill.value),
  staleTime: 120_000,
  retry: false,
})
function enter(e: PickerEntry | undefined) {
  if (!e || e.kind !== 'moodle_course' || e.courseId == null) return
  drill.value = { ws: e.ws, id: e.courseId, name: e.title }
  query.value = ''
  active.value = 0
  nextTick(() => input.value?.focus())
}
function leave() {
  drill.value = null
  query.value = ''
  nextTick(() => input.value?.focus())
}
const wholeCourse = computed<PickerEntry | null>(() => {
  const d = drill.value
  return d ? { key: `moodle_course:${d.ws}:${d.id}`, kind: 'moodle_course', id: `${d.ws}:${d.id}`, ws: d.ws, title: d.name, place: t('ai.picker.wholeCourse'), excerpt: '', courseId: d.id, courseName: d.name } : null
})

// --- Récents (mémorisés dans ce navigateur) et favoris ---------------------------------------------
const RECENTS_KEY = 'looty.ai.recents'
const recents = ref<RecentItem[]>([])
const asEntry = (r: RecentItem, place = ''): PickerEntry => ({ key: `${r.kind}:${r.id}`, kind: r.kind, id: r.id, ws: r.ws, title: r.title, place, excerpt: '' })
function remember(a: AiAttachment) {
  if (a.kind === 'file' || isMoodleKind(a.kind) || !a.ws) return // un fichier expire ; un cours Moodle se retrouve dans sa propre liste
  recents.value = pushRecent(recents.value, { kind: a.kind, id: a.id, ws: a.ws, title: a.title })
  try { localStorage.setItem(RECENTS_KEY, JSON.stringify(recents.value)) } catch { /* stockage indisponible */ }
}

const sections = computed<PickerSection<PickerEntry>[]>(() => {
  // Dans un cours : le cours entier, puis ses fichiers lisibles (filtrés par la recherche).
  if (drill.value) {
    const d = drill.value
    const files = fileEntries(d.ws, { id: d.id, name: d.name }, courseContent.value?.sections ?? [], debounced.value)
    return [{ id: 'moodle_course', items: wholeCourse.value ? [wholeCourse.value] : [] }, { id: 'moodle_file', items: files }].filter((s) => s.items.length)
  }
  if (searching.value) return groupByKind([...(found.value ?? []), ...moodleEntries.value], filter.value)
  const fav = (favorites.value ?? []).map((p) => asEntry({ kind: 'page', id: p.id, ws: p.workspace_id, title: p.title }))
  const rec = recents.value.filter((r) => filter.value === 'all' || r.kind === filter.value).map((r) => asEntry(r))
  const base = filter.value === 'moodle' ? [] : emptyQuerySections(rec, filter.value === 'all' || filter.value === 'page' ? fav : [])
  // Liste vide : quelques cours Moodle (tous avec le filtre « Moodle »), après les récents et les favoris.
  const courses = filter.value === 'all' ? moodleEntries.value.slice(0, 6) : filter.value === 'moodle' ? moodleEntries.value.slice(0, 40) : []
  return courses.length ? [...base, { id: 'moodle_course', items: courses }] : base
})
const flat = computed(() => flatten(sections.value))
const active = ref(0)
const current = computed(() => flat.value[active.value])
watch([flat, open], () => { active.value = 0 })
watch(open, (v) => {
  if (!v) return
  filter.value = 'all'
  drill.value = null
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
watch(filter, () => { drill.value = null })
/** Chargement en cours : la recherche, ou le contenu du cours parcouru. */
const loading = computed(() => (drill.value ? drillFetching.value : searching.value && isFetching.value))
const showNone = computed(() => !loading.value && !flat.value.length && (!!drill.value || searching.value))
const showHint = computed(() => !drill.value && !searching.value && !flat.value.length)
const drillProblem = computed(() => (drill.value && drillError.value ? t('ai.picker.courseError') : ''))
const status = computed(() => (showNone.value ? t('ai.picker.none') : flat.value.length ? t('ai.picker.count', { count: flat.value.length }) : ''))
/** Aperçu d'un élément Moodle : cours, section, type et taille. */
const facts = (e: PickerEntry): { label: string; value: string }[] => {
  if (e.kind !== 'moodle_file') return []
  return [
    { label: t('ai.picker.fact.course'), value: e.courseName ?? '' },
    { label: t('ai.picker.fact.section'), value: e.section ?? '' },
    { label: t('ai.picker.fact.type'), value: e.type ?? '' },
    { label: t('ai.picker.fact.size'), value: e.size != null ? fileSize(e.size) : '' },
  ].filter((f) => f.value)
}

function choose(e: PickerEntry | undefined) {
  if (!e) return
  let a: AiAttachment
  if (e.kind === 'moodle_course' && e.courseId != null) a = moodleCourseAttachment(e.ws, { id: e.courseId, name: e.title })
  else if (e.kind === 'moodle_file' && e.courseId != null && e.url) a = moodleFileAttachment(e.ws, { id: e.courseId, name: e.courseName ?? '' }, { name: e.title, source: e.url, size: e.size })
  else a = { kind: e.kind as AiAttachKind, id: e.id, title: e.title, ws: e.ws }
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
  // → sur un cours Moodle (champ vide) : parcourir ses fichiers ; ← ou Retour arrière (champ vide) : revenir.
  else if (e.key === 'ArrowRight' && !query.value && !drill.value && current.value?.kind === 'moodle_course') { e.preventDefault(); enter(current.value) }
  else if ((e.key === 'ArrowLeft' || e.key === 'Backspace') && !query.value && drill.value) { e.preventDefault(); leave() }
  else if (e.key === 'Escape') { if (drill.value) { e.stopPropagation(); leave() } else open.value = false }
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
          <button v-if="drill" type="button" tabindex="-1" class="pk-back" @click="leave"><ArrowLeft :size="13" /><span class="truncate">{{ drill.name }}</span></button>
          <p v-if="drillProblem" class="pk-hint" role="presentation">{{ drillProblem }}</p>
          <p v-else-if="showNone" class="pk-hint" role="presentation">{{ t('ai.picker.none') }}</p>
          <p v-else-if="loading && (drill || !flat.length)" class="pk-hint" role="presentation"><UiSpinner /></p>
          <p v-else-if="showHint" class="pk-hint" role="presentation">{{ t('ai.picker.hint') }}</p>
          <div v-for="s in sections" :key="s.id" role="group" :aria-labelledby="`${uid}-h-${s.id}`">
            <div :id="`${uid}-h-${s.id}`" class="pk-head">
              <component :is="sectionIcon(s.id)" v-if="sectionIcon(s.id)" :size="11" />
              <component :is="KINDS[s.id as AiAttachKind].icon" v-else-if="KINDS[s.id as AiAttachKind]" :size="11" />
              {{ sectionLabel(s.id) }}
            </div>
            <div v-for="e in s.items" :key="e.key" class="pk-item" :class="{ on: e === current }" @mouseenter="active = flat.indexOf(e)">
              <button :id="optionId(e, s.id)" type="button" tabindex="-1" class="pk-row" role="option" :aria-selected="e === current" @click="choose(e)">
                <component :is="iconOf(e)" :size="14" class="pk-icon" />
                <span class="pk-text">
                  <span class="pk-title truncate">{{ e.title || t('docs.untitled') }}</span>
                  <span v-if="e.place" class="pk-path truncate">{{ e.place }}</span>
                </span>
              </button>
              <button v-if="e.kind === 'moodle_course' && !drill" type="button" tabindex="-1" class="pk-drill" aria-hidden="true" :title="t('ai.picker.browse')" @click.stop="enter(e)"><ChevronRight :size="14" /></button>
            </div>
          </div>
        </div>
        <aside v-if="current" class="pk-preview" :aria-label="t('ai.picker.preview')">
          <component :is="iconOf(current)" :size="18" class="pk-icon" />
          <strong class="pk-p-title">{{ current.title || t('docs.untitled') }}</strong>
          <span class="pk-p-kind">{{ t(`ai.picker.kind.${current.kind}`) }}</span>
          <span v-if="current.place" class="pk-p-place">{{ current.place }}</span>
          <dl v-if="facts(current).length" class="pk-p-facts">
            <template v-for="f in facts(current)" :key="f.label"><dt>{{ f.label }}</dt><dd>{{ f.value }}</dd></template>
          </dl>
          <p v-else-if="current.excerpt" class="pk-p-excerpt">{{ current.excerpt }}</p>
          <p v-else-if="current.kind === 'moodle_course'" class="pk-p-none">{{ t('ai.picker.courseHint') }}</p>
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
.pk-item { display: flex; align-items: center; border-radius: var(--r-sm); }
.pk-item.on { background: var(--bg-hover); }
.pk-row { display: flex; flex: 1; min-width: 0; align-items: center; gap: 8px; padding: 6px 8px; border-radius: var(--r-sm); text-align: left; color: var(--text); }
.pk-drill { display: grid; place-items: center; flex: none; width: 26px; height: 26px; margin-right: 4px; border-radius: var(--r-sm); color: var(--text-3); }
.pk-drill:hover { background: var(--bg-active, var(--bg-hover)); color: var(--text); }
.pk-back { display: flex; align-items: center; gap: 6px; padding: 6px 8px; border-radius: var(--r-sm); color: var(--text-2); font-size: var(--fs-sm); text-align: left; }
.pk-back:hover { background: var(--bg-hover); }
.pk-icon { flex: none; color: var(--text-3); }
.pk-text { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.pk-title { font-size: var(--fs-sm); }
.pk-path { font-size: var(--fs-xs); color: var(--text-3); }
.pk-preview { display: flex; flex-direction: column; gap: 4px; padding: 10px; border-radius: var(--r-md); background: var(--bg-elevated); border: 1px solid var(--border); overflow: hidden; max-height: 320px; }
.pk-p-title { font-size: var(--fs-sm); line-height: 1.3; word-break: break-word; }
.pk-p-kind { font-size: var(--fs-xs); color: var(--text-4); }
.pk-p-place { font-size: var(--fs-xs); color: var(--text-3); word-break: break-word; }
.pk-p-excerpt { margin: 4px 0 0; font-size: var(--fs-xs); color: var(--text-2); line-height: 1.5; overflow: hidden; display: -webkit-box; -webkit-line-clamp: 8; line-clamp: 8; -webkit-box-orient: vertical; }
.pk-p-facts { display: grid; grid-template-columns: auto 1fr; gap: 2px 8px; margin: 4px 0 0; font-size: var(--fs-xs); }
.pk-p-facts dt { color: var(--text-4); }
.pk-p-facts dd { margin: 0; color: var(--text-2); word-break: break-word; }
.pk-p-none { margin: 4px 0 0; font-size: var(--fs-xs); color: var(--text-4); }
.pk-foot { display: flex; gap: 12px; color: var(--text-4); font-size: var(--fs-xs); }
.pk-foot kbd { padding: 0 4px; margin-right: 2px; border: 1px solid var(--border); border-radius: var(--r-sm); font: inherit; }
@media (max-width: 640px) {
  .pk-body { grid-template-columns: minmax(0, 1fr); }
  .pk-preview, .pk-foot { display: none; }
}
</style>
