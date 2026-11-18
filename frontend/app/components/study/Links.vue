<script setup lang="ts">
import { FileText, Kanban, CircleDot, CalendarDays, FolderCode, Link2, Plus, X, Search } from 'lucide-vue-next'
import { useQuery } from '@tanstack/vue-query'
import { refDebounced } from '@vueuse/core'
import { api } from '~/composables/api'
import { useStudyActions, useStudyLinks, STUDY_LINK_KINDS, type StudyLinkKind } from '~/composables/useStudy'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { hitPlace } from '~/utils/pickerSearch'
import type { SearchResult } from '~/types'

/**
 * Liens de l'entraînement vers les éléments de l'espace : pages, tableaux, tâches, événements et
 * projets. Chaque lien n'apparaît qu'à qui voit sa cible.
 */
const props = defineProps<{ setId: string; ws: string; canEdit: boolean }>()
const { t } = useI18n()
const actions = useStudyActions()
const id = computed(() => props.setId)
const { data } = useStudyLinks(id)
const { pages, boards } = useWorkspace()

const ICONS: Record<StudyLinkKind, any> = { page: FileText, board: Kanban, task: CircleDot, event: CalendarDays, project: FolderCode }
function path(kind: StudyLinkKind, target: string) {
  switch (kind) {
    case 'page': return links.doc(props.ws, target)
    case 'board': return links.board(props.ws, target)
    case 'task': return links.taskById(props.ws, target)
    case 'event': return `${links.calendar(props.ws)}?event=${target}`
    default: return `/w/${props.ws}/projects/${target}`
  }
}
const linked = computed(() => new Set((data.value ?? []).map((l) => `${l.target_kind}:${l.target_id}`)))

// --- Sélecteur : pages et tableaux à l'ouverture, recherche de l'espace dès qu'on tape ---
const pickOpen = ref(false)
const query = ref('')
const debounced = refDebounced(query, 180)
const input = ref<{ focus: () => void } | null>(null)
interface Option { kind: StudyLinkKind; id: string; title: string; place: string }
const { data: found, isFetching } = useQuery({
  queryKey: computed(() => ['study', 'link-search', props.ws, debounced.value.trim()]),
  queryFn: async () => (await api<{ results: SearchResult[] }>(`/api/workspaces/${props.ws}/search?q=${encodeURIComponent(debounced.value.trim())}`, { silent: true })).results,
  enabled: computed(() => pickOpen.value && !!debounced.value.trim()),
})
const options = computed<Option[]>(() => {
  const free = (o: Option) => !linked.value.has(`${o.kind}:${o.id}`)
  if (!debounced.value.trim()) {
    return [
      ...(pages.data.value ?? []).map((p) => ({ kind: 'page' as const, id: p.id, title: p.title || t('docs.untitled'), place: '' })),
      ...(boards.data.value ?? []).map((b) => ({ kind: 'board' as const, id: b.id, title: b.name, place: '' })),
    ].filter(free).slice(0, 30)
  }
  return (found.value ?? [])
    .filter((r): r is SearchResult & { type: StudyLinkKind } => (STUDY_LINK_KINDS as readonly string[]).includes(r.type))
    .map((r) => ({ kind: r.type, id: r.id, title: r.title || t('docs.untitled'), place: hitPlace({ kind: r.type, ws: props.ws, wsName: '', r }, { multiWorkspace: false }) }))
    .filter(free)
    .slice(0, 40)
})
const KIND_LABEL = computed<Record<StudyLinkKind, string>>(() => ({ page: t('study.linkPages'), board: t('study.linkBoards'), task: t('study.linkTasks'), event: t('study.linkEvents'), project: t('study.linkProjects') }))
const active = ref(0)
watch([options, pickOpen], () => { active.value = 0 })
watch(pickOpen, (v) => { if (v) { query.value = ''; nextTick(() => setTimeout(() => input.value?.focus(), 30)) } })

async function add(o: Option | undefined) {
  if (!o) return
  pickOpen.value = false
  await actions.link(props.setId, o.kind, o.id)
}
function onKey(e: KeyboardEvent) {
  const n = Math.max(1, options.value.length)
  if (e.key === 'ArrowDown') { e.preventDefault(); active.value = (active.value + 1) % n }
  else if (e.key === 'ArrowUp') { e.preventDefault(); active.value = (active.value - 1 + n) % n }
  else if (e.key === 'Enter' && !e.isComposing) { e.preventDefault(); void add(options.value[active.value]) }
  else if (e.key === 'Escape') pickOpen.value = false
}
</script>

<template>
  <section v-if="canEdit || data?.length" class="sl">
    <header>
      <h2><Link2 :size="13" /> {{ t('study.links') }}</h2>
      <UiPopover v-if="canEdit" v-model:open="pickOpen" :width="340" placement="bottom-end">
        <template #trigger><UiButton size="sm" variant="ghost" :icon="Plus">{{ t('study.addLink') }}</UiButton></template>
        <div class="lp" @keydown="onKey">
          <UiInput ref="input" v-model="query" :icon="Search" size="sm" :placeholder="t('study.linkSearch')" />
          <div class="lp-list" role="listbox">
            <p v-if="!debounced.trim()" class="lp-hint">{{ t('study.linkHint') }}</p>
            <div v-if="debounced.trim() && isFetching && !options.length" class="lp-hint"><UiSpinner /></div>
            <p v-else-if="debounced.trim() && !options.length" class="lp-hint">{{ t('ai.picker.none') }}</p>
            <button v-for="(o, i) in options" :key="`${o.kind}:${o.id}`" class="lp-row" :class="{ on: i === active }" role="option" :aria-selected="i === active" @mouseenter="active = i" @click="add(o)">
              <component :is="ICONS[o.kind]" :size="14" class="lp-icon" />
              <span class="lp-text">
                <span class="lp-title truncate">{{ o.title }}</span>
                <span v-if="o.place" class="lp-place truncate">{{ o.place }}</span>
              </span>
              <span class="lp-kind">{{ KIND_LABEL[o.kind] }}</span>
            </button>
          </div>
        </div>
      </UiPopover>
    </header>
    <p v-if="!data?.length" class="faint">{{ t('study.noLinks') }}</p>
    <ul v-else>
      <li v-for="l in data" :key="l.id">
        <NuxtLink :to="path(l.target_kind, l.target_id)" class="sl-item"><component :is="ICONS[l.target_kind]" :size="14" class="faint" /><span class="truncate">{{ l.title || t('docs.untitled') }}</span></NuxtLink>
        <UiButton v-if="canEdit" size="sm" variant="ghost" :icon="X" :aria-label="t('study.removeLink')" @click="actions.unlink(setId, l.id)" />
      </li>
    </ul>
  </section>
</template>

<style scoped>
.sl { border-top: 1px solid var(--border); padding-top: 12px; }
.sl header { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.sl h2 { display: flex; align-items: center; gap: 6px; margin: 0; font-size: var(--fs-sm); text-transform: uppercase; letter-spacing: .04em; color: var(--text-3); }
.sl ul { list-style: none; margin: 6px 0 0; padding: 0; }
.sl li { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.sl-item { display: flex; align-items: center; gap: 8px; flex: 1; min-width: 0; padding: 6px 8px; border-radius: var(--r-sm); }
.sl-item:hover { background: var(--bg-hover); }
.lp { display: flex; flex-direction: column; gap: 8px; padding: 8px; }
.lp-list { display: flex; flex-direction: column; max-height: 300px; overflow-y: auto; }
.lp-hint { color: var(--text-3); font-size: var(--fs-sm); padding: 4px 8px; margin: 0; display: flex; justify-content: center; }
.lp-row { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: var(--r-sm); text-align: left; color: var(--text); }
.lp-row.on { background: var(--bg-hover); }
.lp-icon { flex: none; color: var(--text-3); }
.lp-text { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.lp-title { font-size: var(--fs-sm); }
.lp-place { font-size: var(--fs-xs); color: var(--text-3); }
.lp-kind { flex: none; font-size: var(--fs-xs); color: var(--text-4); }
</style>
