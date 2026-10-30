<script setup lang="ts">
import { CheckCircle2, Circle, CalendarDays, Kanban, FileText, Sigma, Globe, NotebookText, PenTool, MapPin, AlertCircle } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import type { Board, Page, Task } from '~/types'
import type { EventDetail } from '~/composables/useCalendar'
import type { ProjectDetail } from '~/composables/useProjects'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useWorkspace } from '~/composables/useWorkspace'
import { boardKey, taskKey, upsertTaskInBoards } from '~/composables/useRealtime'
import { projectKeys } from '~/composables/useProjects'
import { isTimed, timeOf } from '~/utils/dates'
import { parseLootyLink } from '~/utils/lootyLinks'

/**
 * Carte vivante d'un élément de Looty sur le canevas (tâche, document, canevas, tableau,
 * événement, projet). Mêmes requêtes que le reste de l'interface : le temps réel la tient
 * à jour. Une tâche se coche directement depuis la carte.
 */
const props = defineProps<{ url: string; editable?: boolean }>()
const { t, formatDate, relativeTime } = useI18n()
const { member, boards } = useWorkspace()
const qc = useQueryClient()
const ref_ = computed(() => parseLootyLink(props.url))
const kind = computed(() => ref_.value?.kind)
const id = computed(() => ref_.value?.id ?? '')
const ws = computed(() => ref_.value?.ws ?? '')

const task = useQuery({
  queryKey: computed(() => taskKey(id.value)),
  queryFn: () => api<{ task: Task; board_id: string }>(`/api/boards/tasks/${id.value}`, { silent: true }),
  enabled: computed(() => kind.value === 'task'),
})
const page = useQuery({
  queryKey: computed(() => ['page', id.value]),
  queryFn: () => api<Page & { canvas_preview?: string | null }>(`/api/workspaces/${ws.value}/pages/${id.value}`, { silent: true }),
  enabled: computed(() => kind.value === 'page' || kind.value === 'canvas'),
})
const board = useQuery({
  queryKey: computed(() => boardKey(id.value)),
  queryFn: () => api<Board>(`/api/boards/${id.value}`, { silent: true }),
  enabled: computed(() => kind.value === 'board'),
})
// Tableau d'une tâche (nom de sa colonne) : celui déjà en cache s'il est ouvert ailleurs.
const taskBoard = useQuery({
  queryKey: computed(() => boardKey(task.data.value?.board_id ?? '')),
  queryFn: () => api<Board>(`/api/boards/${task.data.value!.board_id}`, { silent: true }),
  enabled: computed(() => kind.value === 'task' && !!task.data.value?.board_id),
})
const event = useQuery({
  queryKey: computed(() => ['ws', ws.value, 'event', id.value]),
  queryFn: () => api<EventDetail>(`/api/workspaces/${ws.value}/events/${id.value}`, { silent: true }),
  enabled: computed(() => kind.value === 'event'),
})
const project = useQuery({
  queryKey: computed(() => projectKeys.one(id.value)),
  queryFn: () => api<ProjectDetail>(`/api/projects/${id.value}`, { silent: true }),
  enabled: computed(() => kind.value === 'project'),
})
const failed = computed(() => [task, page, board, event, project].some((q) => q.isError.value))

/** Texte brut d'un HTML (extrait d'un document). */
const excerpt = computed(() => {
  const html = page.data.value?.content ?? ''
  const div = document.createElement('div')
  div.innerHTML = html.replace(/<(script|style)[\s\S]*?<\/\1>/gi, '')
  return (div.textContent ?? '').replace(/\s+/g, ' ').trim().slice(0, 220)
})
const boardName = (bid?: string) => boards.data.value?.find((b) => b.id === bid)?.name ?? ''
const columnName = computed(() => taskBoard.data.value?.columns.find((c) => c.id === task.data.value?.task.column_id)?.name)
const counts = computed(() => (board.data.value?.columns ?? []).map((c) => ({ name: c.name, n: c.tasks.filter((x) => !x.parent_id).length })))
const total = computed(() => counts.value.reduce((s, c) => s + c.n, 0) || 1)
const projectIcon = computed(() => (project.data.value?.project.kind === 'site' ? Globe : project.data.value?.project.kind === 'notebook' ? NotebookText : Sigma))
const eventWhen = computed(() => {
  const e = event.data.value
  if (!e) return ''
  return e.all_day ? formatDate(e.start, { weekday: 'short', day: 'numeric', month: 'short' }) : `${formatDate(e.start, { weekday: 'short', day: 'numeric', month: 'short' })} · ${timeOf(e.start)}–${timeOf(e.end)}`
})
const overdue = computed(() => {
  const tk = task.data.value?.task
  return !!tk?.due_date && !tk.is_completed && new Date(tk.due_date) < new Date()
})

async function toggleTask() {
  const tk = task.data.value?.task
  if (!tk || !props.editable) return
  const r = await api<Task>(`/api/boards/tasks/${tk.id}/toggle_validation`, { method: 'PUT' })
  upsertTaskInBoards(qc, r)
  qc.setQueryData(taskKey(tk.id), (old: any) => (old ? { ...old, task: r } : old))
}
const previewSrc = computed(() => (page.data.value?.canvas_preview ? `data:image/svg+xml;charset=utf-8,${encodeURIComponent(page.data.value.canvas_preview)}` : ''))
</script>

<template>
  <div class="lc" :class="[kind, { failed }]">
    <div v-if="failed" class="lc-row lc-muted"><AlertCircle :size="15" /> {{ t('canvas.cardUnavailable') }}</div>

    <!-- Tâche -->
    <template v-else-if="kind === 'task'">
      <div v-if="!task.data.value" class="lc-skel" />
      <template v-else>
        <div class="lc-head">
          <button class="lc-check" :class="{ on: task.data.value.task.is_completed }" :disabled="!editable" :aria-label="t('task.markDone')" @pointerdown.stop @click.stop="toggleTask">
            <component :is="task.data.value.task.is_completed ? CheckCircle2 : Circle" :size="17" />
          </button>
          <span class="lc-title" :class="{ done: task.data.value.task.is_completed }">{{ task.data.value.task.title }}</span>
        </div>
        <div class="lc-meta">
          <span class="lc-chip"><Kanban :size="12" /> {{ boardName(task.data.value.board_id) }}<template v-if="columnName"> · {{ columnName }}</template></span>
          <span v-if="task.data.value.task.due_date" class="lc-chip" :class="{ late: overdue }">
            <CalendarDays :size="12" /> {{ isTimed(task.data.value.task.due_date) ? `${formatDate(task.data.value.task.due_date)} · ${timeOf(task.data.value.task.due_date)}` : formatDate(task.data.value.task.due_date) }}
          </span>
        </div>
        <div v-if="task.data.value.task.labels.length || task.data.value.task.assignees.length" class="lc-foot">
          <span v-for="l in task.data.value.task.labels.slice(0, 3)" :key="l.id" class="lc-label"><span class="dot" :style="{ background: l.color }" />{{ l.name }}</span>
          <span class="lc-space" />
          <span class="lc-avatars"><UiAvatar v-for="a in task.data.value.task.assignees.slice(0, 4)" :key="a" :name="member(a)?.name" :src="member(a)?.avatar_url" :size="20" ring /></span>
        </div>
      </template>
    </template>

    <!-- Document ou canevas -->
    <template v-else-if="kind === 'page' || kind === 'canvas'">
      <div v-if="!page.data.value" class="lc-skel" />
      <template v-else>
        <div class="lc-head">
          <span class="lc-emoji">{{ page.data.value.icon || '' }}</span>
          <component :is="kind === 'canvas' ? PenTool : FileText" v-if="!page.data.value.icon" :size="16" class="lc-ic" />
          <span class="lc-title">{{ page.data.value.title || t('docs.untitled') }}</span>
        </div>
        <img v-if="kind === 'canvas' && previewSrc" class="lc-preview" :src="previewSrc" alt="" />
        <p v-else-if="excerpt" class="lc-excerpt">{{ excerpt }}</p>
        <div class="lc-meta"><span class="lc-chip">{{ t('editor.lastEdited', { name: member(page.data.value.updated_by ?? page.data.value.created_by)?.name ?? '?', time: relativeTime(page.data.value.updated_at) }) }}</span></div>
      </template>
    </template>

    <!-- Tableau -->
    <template v-else-if="kind === 'board'">
      <div v-if="!board.data.value" class="lc-skel" />
      <template v-else>
        <div class="lc-head"><Kanban :size="16" class="lc-ic" /><span class="lc-title">{{ board.data.value.name }}</span></div>
        <div class="lc-bars">
          <div v-for="c in counts" :key="c.name" class="lc-bar">
            <span class="lc-bar-name truncate">{{ c.name }}</span>
            <span class="lc-bar-track"><span class="lc-bar-fill" :style="{ width: `${(c.n / total) * 100}%` }" /></span>
            <span class="lc-bar-n tabular">{{ c.n }}</span>
          </div>
        </div>
      </template>
    </template>

    <!-- Événement -->
    <template v-else-if="kind === 'event'">
      <div v-if="!event.data.value" class="lc-skel" />
      <template v-else>
        <div class="lc-head"><CalendarDays :size="16" class="lc-ic" /><span class="lc-title">{{ event.data.value.title }}</span></div>
        <div class="lc-when">{{ eventWhen }}</div>
        <div v-if="event.data.value.location" class="lc-meta"><span class="lc-chip"><MapPin :size="12" /> {{ event.data.value.location }}</span></div>
      </template>
    </template>

    <!-- Projet -->
    <template v-else-if="kind === 'project'">
      <div v-if="!project.data.value" class="lc-skel" />
      <template v-else>
        <div class="lc-head"><component :is="projectIcon" :size="16" class="lc-ic" /><span class="lc-title">{{ project.data.value.project.name }}</span></div>
        <div class="lc-meta">
          <span class="lc-chip">{{ t('projects.fileCount', { count: project.data.value.files.filter((f) => !f.is_dir).length }) }}</span>
          <span v-if="project.data.value.project.last_build_ok != null" class="lc-chip" :class="{ late: !project.data.value.project.last_build_ok }">{{ project.data.value.project.last_build_ok ? t('projects.buildOk') : t('projects.buildFailed') }}</span>
        </div>
      </template>
    </template>

  </div>
</template>

<style scoped>
.lc {
  --c: var(--accent);
  position: relative; box-sizing: border-box; width: 100%; height: 100%; overflow: hidden;
  display: flex; flex-direction: column; gap: 8px; padding: 12px 14px;
  border-radius: 14px; background: var(--bg-panel); color: var(--text);
  box-shadow: inset 0 0 0 1px var(--border), 0 1px 2px rgb(0 0 0 / .06);
  font: 13px/1.4 var(--font-sans, Inter, system-ui, sans-serif); text-align: left;
}
.lc::before { content: ''; position: absolute; left: 0; top: 10px; bottom: 10px; width: 3px; border-radius: 0 3px 3px 0; background: var(--c); }
.lc.task { --c: #5e6ad2; }
.lc.page { --c: #8a8f98; }
.lc.canvas { --c: #b18cff; }
.lc.board { --c: #26b5ce; }
.lc.event { --c: #e2803f; }
.lc.project { --c: #3f8f5a; }
.lc-head { display: flex; align-items: center; gap: 8px; min-width: 0; }
.lc-title { font-weight: 600; font-size: 14px; overflow: hidden; text-overflow: ellipsis; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; }
.lc-title.done { text-decoration: line-through; color: var(--text-3); }
.lc-emoji:empty { display: none; }
.lc-emoji { font-size: 16px; }
.lc-ic { flex: none; color: var(--c); }
.lc-check { flex: none; display: grid; place-items: center; color: var(--text-3); }
.lc-check.on { color: var(--green); }
.lc-check:not(:disabled):hover { color: var(--accent); }
.lc-meta, .lc-foot { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
.lc-chip { display: inline-flex; align-items: center; gap: 4px; padding: 1px 7px; border-radius: 999px; background: var(--bg-hover); color: var(--text-2); font-size: 11.5px; }
.lc-chip.late { background: color-mix(in srgb, var(--red) 14%, transparent); color: var(--red); }
.lc-label { display: inline-flex; align-items: center; gap: 4px; font-size: 11.5px; color: var(--text-2); }
.lc-label .dot { width: 7px; height: 7px; border-radius: 50%; }
.lc-space { flex: 1; }
.lc-avatars { display: flex; }
.lc-avatars > :not(:first-child) { margin-left: -6px; }
.lc-excerpt { margin: 0; color: var(--text-2); overflow: hidden; display: -webkit-box; -webkit-line-clamp: 4; -webkit-box-orient: vertical; }
.lc-preview { flex: 1; min-height: 0; width: 100%; object-fit: contain; border-radius: 8px; background: var(--bg); }
.lc-when { font-weight: 500; color: var(--c); }
.lc-bars { display: flex; flex-direction: column; gap: 5px; }
.lc-bar { display: grid; grid-template-columns: 84px 1fr 22px; align-items: center; gap: 8px; font-size: 11.5px; color: var(--text-2); }
.lc-bar-track { height: 6px; border-radius: 3px; background: var(--bg-hover); overflow: hidden; }
.lc-bar-fill { display: block; height: 100%; border-radius: 3px; background: var(--c); }
.lc-bar-n { text-align: right; }
.lc-skel { flex: 1; border-radius: 8px; background: var(--bg-hover); animation: lc-pulse 1.2s ease-in-out infinite; }
.lc-muted { display: flex; align-items: center; gap: 6px; color: var(--text-3); }
@keyframes lc-pulse { 50% { opacity: .55; } }
</style>
