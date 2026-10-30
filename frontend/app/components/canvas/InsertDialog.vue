<script setup lang="ts">
import { FileText, CheckSquare, Kanban, CalendarDays, FolderCode, PenTool, Search, Sigma, Globe, NotebookText } from 'lucide-vue-next'
import type { Component } from 'vue'
import { useQuery } from '@tanstack/vue-query'
import { refDebounced } from '@vueuse/core'
import type { CalEvent } from '~/composables/useCalendar'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useWorkspace, useMyTasks } from '~/composables/useWorkspace'
import { useProjectList } from '~/composables/useProjects'
import { fold } from '~/utils/fold'
import { lootyUrl, type LootyRef } from '~/utils/lootyLinks'

/**
 * « Insérer depuis Looty » : documents, canevas, tâches, tableaux, événements et projets
 * de l'espace, ajoutés au canevas comme cartes vivantes.
 */
const open = defineModel<boolean>('open', { default: false })
const emit = defineEmits<{ (e: 'pick', urls: string[]): void }>()
const { t, formatDate } = useI18n()
const { wsId, pages, boards } = useWorkspace()
type Tab = 'page' | 'task' | 'board' | 'event' | 'project'
const tab = ref<Tab>('page')
const q = ref('')
const query = refDebounced(computed(() => q.value.trim()), 250)
const picked = ref<string[]>([])
watch(open, (v) => { if (v) { q.value = ''; picked.value = [] } })

const scope = ref<'all'>('all')
const myTasks = useMyTasks(scope)
const searched = useQuery({
  queryKey: computed(() => ['ws', wsId.value, 'canvas-search', query.value]),
  queryFn: () => api<{ results: { id: string; title: string; type: string; parent_id: string | null }[] }>(`/api/workspaces/${wsId.value}/search?q=${encodeURIComponent(query.value)}`),
  enabled: computed(() => open.value && tab.value === 'task' && !!query.value),
})
const from = new Date()
const to = new Date(Date.now() + 60 * 86400_000)
const events = useQuery({
  queryKey: computed(() => ['ws', wsId.value, 'canvas-events']),
  queryFn: () => api<{ events: CalEvent[] }>(`/api/workspaces/${wsId.value}/calendars/events?from=${encodeURIComponent(from.toISOString())}&to=${encodeURIComponent(to.toISOString())}`),
  enabled: computed(() => open.value && tab.value === 'event'),
})
const projects = useProjectList(wsId)

interface Row { ref: LootyRef; title: string; hint: string; icon: Component | string }
const match = (s: string) => !query.value || fold(s).includes(fold(query.value))
const rows = computed<Row[]>(() => {
  const ws = wsId.value
  switch (tab.value) {
    case 'page':
      return (pages.data.value ?? []).filter((p) => match(p.title || '')).slice(0, 60).map((p) => ({
        ref: { kind: p.default_view === 'canvas' ? 'canvas' : 'page', ws, id: p.id },
        title: p.title || t('docs.untitled'),
        hint: p.default_view === 'canvas' ? t('canvas.title') : t('canvas.kinds.page'),
        icon: p.icon || (p.default_view === 'canvas' ? PenTool : FileText),
      }))
    case 'task':
      if (query.value) {
        return (searched.data.value?.results ?? []).filter((r) => r.type === 'task').map((r) => ({
          ref: { kind: 'task', ws, id: r.id, boardId: r.parent_id ?? undefined }, title: r.title,
          hint: boards.data.value?.find((b) => b.id === r.parent_id)?.name ?? '', icon: CheckSquare,
        }))
      }
      return (myTasks.data.value ?? []).slice(0, 60).map((x) => ({ ref: { kind: 'task', ws, id: x.id, boardId: x.board_id }, title: x.title, hint: `${x.board_name} · ${x.column_name}`, icon: CheckSquare }))
    case 'board':
      return (boards.data.value ?? []).filter((b) => match(b.name)).map((b) => ({ ref: { kind: 'board', ws, id: b.id }, title: b.name, hint: t('canvas.kinds.board'), icon: Kanban }))
    case 'event':
      return (events.data.value?.events ?? []).filter((e) => e.looty && e.event_id && match(e.summary)).slice(0, 60).map((e) => ({
        ref: { kind: 'event', ws, id: e.event_id! }, title: e.summary, hint: formatDate(e.starts_at, { weekday: 'short', day: 'numeric', month: 'short' }), icon: CalendarDays,
      }))
    case 'project':
      return (projects.data.value ?? []).filter((p) => match(p.name)).map((p) => ({
        ref: { kind: 'project', ws, id: p.id }, title: p.name, hint: p.kind === 'site' ? t('projects.site') : p.kind === 'notebook' ? 'Notebook' : 'LaTeX',
        icon: p.kind === 'site' ? Globe : p.kind === 'notebook' ? NotebookText : Sigma,
      }))
  }
  return []
})
const key = (r: LootyRef) => `${r.kind}:${r.id}`
function toggle(r: Row) {
  const k = lootyUrl(r.ref)
  picked.value = picked.value.includes(k) ? picked.value.filter((x) => x !== k) : [...picked.value, k]
}
function insert(urls = picked.value) {
  if (!urls.length) return
  emit('pick', urls)
  open.value = false
}
const tabs = computed(() => [
  { value: 'page', label: t('canvas.kinds.pages'), icon: FileText },
  { value: 'task', label: t('canvas.kinds.tasks'), icon: CheckSquare },
  { value: 'board', label: t('canvas.kinds.boards'), icon: Kanban },
  { value: 'event', label: t('canvas.kinds.events'), icon: CalendarDays },
  { value: 'project', label: t('canvas.kinds.projects'), icon: FolderCode },
])
</script>

<template>
  <UiDialog v-model:open="open" :title="t('canvas.insertTitle')" :description="t('canvas.insertHint')" :width="560">
    <div class="ci">
      <UiSegmented v-model="tab" :options="tabs" />
      <UiInput v-model="q" :icon="Search" :placeholder="t('canvas.searchPlaceholder')" autofocus />
      <div class="ci-list">
        <button v-for="r in rows" :key="key(r.ref)" class="ci-row" :class="{ on: picked.includes(lootyUrl(r.ref)) }" @click="toggle(r)" @dblclick="insert([lootyUrl(r.ref)])">
          <span class="ci-ic"><span v-if="typeof r.icon === 'string'">{{ r.icon }}</span><component :is="r.icon" v-else :size="15" /></span>
          <span class="ci-text"><span class="truncate">{{ r.title }}</span><small class="truncate">{{ r.hint }}</small></span>
          <span class="ci-tick" />
        </button>
        <p v-if="!rows.length" class="ci-empty">{{ t('canvas.nothing') }}</p>
      </div>
    </div>
    <template #footer>
      <UiButton variant="ghost" @click="open = false">{{ t('common.cancel') }}</UiButton>
      <UiButton variant="primary" :disabled="!picked.length" @click="insert()">{{ t('canvas.insertN', { count: picked.length }) }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.ci { display: flex; flex-direction: column; gap: 10px; }
.ci-list { height: 340px; overflow-y: auto; display: flex; flex-direction: column; gap: 2px; margin: 0 -6px; }
.ci-row { display: flex; align-items: center; gap: 10px; padding: 7px 8px; border-radius: var(--r-md); text-align: left; color: var(--text); }
.ci-row:hover { background: var(--bg-hover); }
.ci-row.on { background: var(--accent-soft); }
.ci-ic { flex: none; width: 26px; height: 26px; display: grid; place-items: center; border-radius: 7px; background: var(--bg-hover); color: var(--text-2); }
.ci-text { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.ci-text small { color: var(--text-3); font-size: var(--fs-sm); }
.ci-tick { flex: none; width: 16px; height: 16px; border-radius: 50%; box-shadow: inset 0 0 0 1.5px var(--border-strong); }
.ci-row.on .ci-tick { background: var(--accent); box-shadow: inset 0 0 0 4px var(--accent-soft); }
.ci-empty { color: var(--text-3); text-align: center; padding: 30px 0; }
</style>
