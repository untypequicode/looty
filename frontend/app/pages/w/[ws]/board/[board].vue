<script setup lang="ts">
import { Kanban, Columns3, List, GanttChart, Settings2, MoreHorizontal, Link2, Trash2, Pencil, Plus, SearchX, ExternalLink, CircleCheck, Signal, UserRound, Tag, CalendarDays, ArrowRightLeft, CalendarRange } from 'lucide-vue-next'
import type { Task, Priority } from '~/types'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { useBoardQuery, useBoardActions, allTasks, positionBetween } from '~/composables/useBoard'
import { useBoardFilters } from '~/composables/useBoardFilters'
import { useAccess } from '~/composables/useAccess'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { useBoards } from '~/composables/useBoards'
import { useAuth } from '~/composables/useAuth'
import { useUi } from '~/composables/useUi'
import { useShortcuts } from '~/composables/useShortcuts'
import { useI18n } from '~/composables/useI18n'
import { motion } from '~/composables/useMotion'
import { toast } from '~/composables/useToast'

definePageMeta({ layout: 'workspace' })

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const { wsId, isAdmin, members } = useWorkspace()
// Droits du rôle et mode d'édition personnel (tableaux).
const { can } = useAccess(wsId)
const canWrite = computed(() => can.value.editTasks)
const canValidate = computed(() => can.value.validate)
const canManage = computed(() => can.value.manageBoards)
const { user } = useAuth()
const { openCreateTask } = useUi()
const { renameBoard, deleteBoard } = useBoards()

const boardId = computed(() => route.params.board as string)
const { data: board, isLoading, error } = useBoardQuery(boardId)
const actions = useBoardActions(boardId)
const filters = useBoardFilters(board)
const view = computed<'board' | 'list' | 'timeline' | 'calendar'>({
  get: () => (['list', 'timeline', 'calendar'].includes(route.query.view as string) ? (route.query.view as any) : 'board'),
  set: (v) => router.replace({ query: { ...route.query, view: v === 'board' ? undefined : v } }),
})

const taskId = computed(() => (route.query.task as string) || null)
function openTask(tk: Task | string) {
  const id = typeof tk === 'string' ? tk : tk.id
  focusedId.value = id
  router.push({ query: { ...route.query, task: id } })
}
function closeTask() {
  const { task: _t, ...rest } = route.query
  router.push({ query: rest })
}

const visible = computed(() => filters.filtered.value)
const order = computed(() => (visible.value?.columns ?? []).flatMap((c) => c.tasks.map((x) => x.id)))
const totalCount = computed(() => allTasks(board.value).filter((x) => !x.parent_id || filters.showSubtasks.value).length)

// --- Renommage du tableau ---
const renaming = ref(false)
const nameDraft = ref('')
function startRename() {
  if (!canWrite.value || !board.value) return
  nameDraft.value = board.value.name
  renaming.value = true
  nextTick(() => (document.querySelector('.board-title-input') as HTMLInputElement | null)?.select())
}
async function commitRename() {
  renaming.value = false
  if (board.value && nameDraft.value.trim() && nameDraft.value.trim() !== board.value.name) {
    await renameBoard(board.value.id, nameDraft.value.trim())
  }
}

const settingsOpen = ref(false)
const boardMenuOpen = ref(false)
const boardMenu = computed<MenuItem[]>(() => [
  { label: t('common.copyLink'), icon: Link2, onSelect: () => { navigator.clipboard.writeText(location.href.split('?')[0]); toast.success(t('common.linkCopied')) } },
  ...(canManage.value ? [
    { label: t('common.rename'), icon: Pencil, onSelect: startRename },
    { label: t('board.settings'), icon: Settings2, onSelect: () => (settingsOpen.value = true) },
  ] : []),
  ...(isAdmin.value && board.value ? [{ divider: true }, { label: t('common.delete'), icon: Trash2, danger: true, onSelect: () => deleteBoard(board.value!) }] : []),
])

// --- Actions des vues ---
async function onToggle(tk: Task, ev?: MouseEvent) {
  if (ev?.currentTarget) motion.pulse(ev.currentTarget as Element)
  await actions.toggleDone(tk.id, user.value?.id)
}
async function onCreate(columnId: string, title: string, atTop: boolean) {
  // Les filtres actifs pré-remplissent la nouvelle tâche (comme Linear)
  await actions.createTask(columnId, {
    title,
    at_top: atTop,
    assignees: filters.assignees.value.filter((a) => a !== 'none'),
    labels: filters.labels.value,
    priority: (filters.priorities.value.length === 1 ? Number(filters.priorities.value[0]) : 0) as Priority,
  })
}
async function onSetDone(columnId: string) {
  const cur = board.value?.automations ?? {}
  await actions.saveAutomations({ ...cur, on_complete_move_to: cur.on_complete_move_to === columnId ? null : columnId, auto_validate_on_move: cur.auto_validate_on_move ?? true })
}
async function onReschedule(tk: Task, start: string | null, due: string) {
  await actions.updateTask(tk.id, { start_date: start, due_date: due })
}

// --- Menu contextuel & sélecteurs rapides ---
const ctxTask = ref<Task | null>(null)
const ctxPos = ref<{ x: number; y: number } | null>(null)
const ctxOpen = ref(false)
const picker = ref<'status' | 'priority' | 'assignee' | 'label' | 'due' | null>(null)
const pickerOpen = computed({ get: () => !!picker.value, set: (v) => { if (!v) picker.value = null } })

function openMenu(tk: Task, ev: MouseEvent) {
  ctxTask.value = tk
  ctxPos.value = { x: ev.clientX, y: ev.clientY }
  ctxOpen.value = true
}
function openPicker(kind: NonNullable<typeof picker.value>, tk: Task, pos?: { x: number; y: number }) {
  ctxTask.value = tk
  if (pos) ctxPos.value = pos
  nextTick(() => (picker.value = kind))
}
const ctxItems = computed<MenuItem[]>(() => {
  const tk = ctxTask.value
  if (!tk) return []
  return [
    { label: t('common.open'), icon: ExternalLink, shortcut: 'enter', onSelect: () => openTask(tk) },
    ...(canValidate.value ? [{ label: tk.is_completed ? t('task.reopen') : t('task.markDone'), icon: CircleCheck, shortcut: 'x', onSelect: () => onToggle(tk) }] : []),
    ...(canWrite.value ? [
      { divider: true },
      { label: t('task.changeStatus'), icon: ArrowRightLeft, shortcut: 's', onSelect: () => openPicker('status', tk) },
      { label: t('task.setPriority'), icon: Signal, shortcut: 'p', onSelect: () => openPicker('priority', tk) },
      { label: t('task.assignTo'), icon: UserRound, shortcut: 'a', onSelect: () => openPicker('assignee', tk) },
      { label: t('task.addLabels'), icon: Tag, shortcut: 'l', onSelect: () => openPicker('label', tk) },
      { label: t('task.dueDate'), icon: CalendarDays, shortcut: 'd', onSelect: () => openPicker('due', tk) },
    ] : []),
    { divider: true },
    { label: t('common.copyLink'), icon: Link2, onSelect: () => { navigator.clipboard.writeText(location.origin + links.task(wsId.value, boardId.value, tk.id)); toast.success(t('common.linkCopied')) } },
    ...(canWrite.value ? [{ label: t('common.delete'), icon: Trash2, danger: true, shortcut: 'delete', onSelect: () => actions.deleteTask(tk) }] : []),
  ]
})
const ctxLive = computed(() => (ctxTask.value ? allTasks(board.value).find((x) => x.id === ctxTask.value!.id) ?? ctxTask.value : null))
const pStatus = computed({ get: () => ctxLive.value?.column_id ?? '', set: (cid: string) => {
  if (!ctxLive.value) return
  const col = board.value?.columns.find((c) => c.id === cid)
  actions.moveTask(ctxLive.value.id, cid, positionBetween(col?.tasks.at(-1)?.position, undefined))
} })
const pPriority = computed({ get: () => (ctxLive.value?.priority ?? 0) as Priority, set: (p: Priority) => ctxLive.value && actions.updateTask(ctxLive.value.id, { priority: p }) })
const pAssignees = computed({ get: () => ctxLive.value?.assignees ?? [], set: (v: string[]) => ctxLive.value && actions.setAssignees(ctxLive.value.id, v) })
const pLabels = computed({ get: () => ctxLive.value?.labels.map((l) => l.id) ?? [], set: (v: string[]) => ctxLive.value && board.value && actions.setLabels(ctxLive.value.id, v, board.value.labels) })
const pDue = computed({ get: () => ctxLive.value?.due_date ?? null, set: (d: string | null | undefined) => ctxLive.value && actions.updateTask(ctxLive.value.id, { due_date: d ?? null }) })
async function pCreateLabel(name: string, color: string) {
  const l = await actions.createLabel(name, color)
  if (ctxLive.value) await actions.setLabel(ctxLive.value.id, l, true)
}

// --- Navigation clavier (J/K/H/L façon Linear / kan.bn) ---
const focusedId = ref<string | null>(null)
function focusedPos() {
  const cols = visible.value?.columns ?? []
  for (let c = 0; c < cols.length; c++) {
    const i = cols[c].tasks.findIndex((x) => x.id === focusedId.value)
    if (i !== -1) return { c, i }
  }
  return null
}
function setFocus(id: string | undefined) {
  if (!id) return
  focusedId.value = id
  nextTick(() => document.querySelector(`[data-task-id="${id}"]`)?.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'smooth' }))
}
function moveFocus(dc: number, di: number) {
  const cols = visible.value?.columns ?? []
  const p = focusedPos()
  if (!p) return setFocus(cols.find((c) => c.tasks.length)?.tasks[0]?.id)
  if (view.value === 'list' && dc === 0) return setFocus(order.value[order.value.indexOf(focusedId.value!) + di])
  if (dc) {
    let c = p.c + dc
    while (cols[c] && !cols[c].tasks.length) c += dc
    if (cols[c]) setFocus(cols[c].tasks[Math.min(p.i, cols[c].tasks.length - 1)].id)
  } else setFocus(cols[p.c].tasks[Math.max(0, Math.min(cols[p.c].tasks.length - 1, p.i + di))]?.id)
}
function focusedTask() { return allTasks(board.value).find((x) => x.id === focusedId.value) }
function pickerAtFocus(kind: NonNullable<typeof picker.value>) {
  const tk = focusedTask()
  if (!tk || !canWrite.value) return
  const r = document.querySelector(`[data-task-id="${tk.id}"]`)?.getBoundingClientRect()
  openPicker(kind, tk, r ? { x: r.left + 12, y: r.bottom } : undefined)
}
async function shiftColumn(dir: number) {
  const tk = focusedTask()
  const cols = board.value?.columns ?? []
  const ci = cols.findIndex((c) => c.id === tk?.column_id)
  const dest = cols[ci + dir]
  if (!tk || !dest || !canWrite.value) return
  await actions.moveTask(tk.id, dest.id, positionBetween(undefined, dest.tasks[0]?.position))
  setFocus(tk.id)
}
const toolbar = ref<{ openSearch: () => void; openFilters: () => void } | null>(null)
const panelOpen = computed(() => !!taskId.value)
const when = (fn: () => void) => () => {
  if (panelOpen.value || view.value === 'timeline' || view.value === 'calendar') return false
  fn()
}
useShortcuts([
  { keys: 'j', handler: when(() => moveFocus(0, 1)) },
  { keys: 'k', handler: when(() => moveFocus(0, -1)) },
  { keys: 'arrowdown', handler: when(() => moveFocus(0, 1)) },
  { keys: 'arrowup', handler: when(() => moveFocus(0, -1)) },
  { keys: 'h', handler: when(() => moveFocus(-1, 0)) },
  { keys: 'l', handler: when(() => moveFocus(1, 0)) },
  { keys: 'arrowleft', handler: when(() => moveFocus(-1, 0)) },
  { keys: 'arrowright', handler: when(() => moveFocus(1, 0)) },
  { keys: 'shift+arrowleft', handler: when(() => shiftColumn(-1)) },
  { keys: 'shift+arrowright', handler: when(() => shiftColumn(1)) },
  { keys: 'enter', handler: when(() => focusedId.value && openTask(focusedId.value)) },
  { keys: 'x', handler: when(() => { const tk = focusedTask(); if (tk && canValidate.value) onToggle(tk) }) },
  { keys: 'p', handler: when(() => pickerAtFocus('priority')) },
  { keys: 'a', handler: when(() => pickerAtFocus('assignee')) },
  { keys: 's', handler: when(() => pickerAtFocus('status')) },
  { keys: 'd', handler: when(() => pickerAtFocus('due')) },
  { keys: 'shift+l', handler: when(() => pickerAtFocus('label')) },
  { keys: 'delete', handler: when(() => { const tk = focusedTask(); if (tk && canWrite.value) actions.deleteTask(tk) }) },
  { keys: 'backspace', handler: when(() => { const tk = focusedTask(); if (tk && canWrite.value) actions.deleteTask(tk) }) },
  { keys: 'f', handler: () => toolbar.value?.openSearch() },
  { keys: 'shift+f', handler: () => toolbar.value?.openFilters() },
  { keys: '1', handler: () => (view.value = 'board') },
  { keys: '2', handler: () => (view.value = 'list') },
  { keys: '3', handler: () => (view.value = 'timeline') },
  { keys: '4', handler: () => (view.value = 'calendar') },
])

// Tableau introuvable / inaccessible
watch(error, (e: any) => { if (e) toast.error(t('board.notFound')) })

const root = ref<HTMLElement | null>(null)
watch([board, view], async ([b], [old]) => {
  if (!b || (old && old.id === b.id && view.value === 'board' && root.value?.dataset.animated === view.value)) return
  await nextTick()
  if (root.value) {
    root.value.dataset.animated = view.value
    motion.stagger(root.value.querySelectorAll('.col, .lv-group, .tl-row'), { y: 8, each: 0.03 })
  }
}, { immediate: true })

useHead({ title: () => board.value?.name ?? t('nav.boards') })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: t('nav.boards'), icon: Kanban }]">
      <template #title>
        <input v-if="renaming" v-model="nameDraft" class="board-title-input" @keydown.enter="commitRename" @keydown.esc="renaming = false" @blur="commitRename" />
        <span v-else-if="board" class="board-title" @dblclick="startRename">{{ board.name }}</span>
      </template>
      <span class="members">
        <UiAvatar v-for="m in (members.data.value ?? []).slice(0, 5)" :key="m.user_id" :name="m.name" :src="m.avatar_url" :size="20" ring />
      </span>
      <UiSegmented
        v-model="view"
        class="views"
        :options="[{ value: 'board', icon: Columns3, title: t('board.views.board') }, { value: 'list', icon: List, title: t('board.views.list') }, { value: 'timeline', icon: GanttChart, title: t('board.views.timeline') }, { value: 'calendar', icon: CalendarRange, title: t('planning.boardCalendar') }]"
      />
      <UiMenu v-model:open="boardMenuOpen" :items="boardMenu" placement="bottom-end">
        <UiButton size="sm" variant="ghost" :icon="MoreHorizontal" />
      </UiMenu>
      <AppEditToggle area="boards" />
      <UiButton v-if="canWrite" size="sm" variant="primary" :icon="Plus" class="new-btn" @click="openCreateTask({ boardId })">{{ t('task.new') }}</UiButton>
    </AppPageHeader>

    <div v-if="isLoading" class="loading">
      <div v-for="i in 4" :key="i" class="skel-col"><div class="skeleton" style="height: 20px; width: 50%" /><div v-for="j in 3" :key="j" class="skeleton" style="height: 74px" /></div>
    </div>
    <UiEmpty v-else-if="!board" :icon="SearchX" :title="t('board.notFound')" :description="t('board.notFoundHint')">
      <UiButton :to="links.home(wsId)">{{ t('common.backHome') }}</UiButton>
    </UiEmpty>
    <template v-else>
      <BoardToolbar ref="toolbar" :board="board" :filters="filters" :total="totalCount" :shown="order.length" />
      <div ref="root" class="board-area">
        <BoardKanban
          v-if="view === 'board'"
          :board="visible!"
          :full-board="board"
          :can-write="canWrite"
          :can-manage="canManage"
          :can-validate="canValidate"
          :focused-id="focusedId"
          @open="openTask"
          @toggle="onToggle"
          @menu="openMenu"
          @move="(id, col, pos) => actions.moveTask(id, col, pos)"
          @move-column="(id, pos) => actions.moveColumn(id, pos)"
          @create="onCreate"
          @create-column="(name) => actions.createColumn(name)"
          @rename-column="(id, name) => actions.renameColumn(id, name)"
          @delete-column="(c) => actions.deleteColumn(c)"
          @set-done-column="onSetDone"
        />
        <BoardListView
          v-else-if="view === 'list'"
          :board="visible!"
          :full-board="board"
          :can-write="canWrite"
          :can-validate="canValidate"
          :focused-id="focusedId"
          @open="openTask"
          @toggle="onToggle"
          @menu="openMenu"
          @create="onCreate"
        />
        <BoardCalendarView v-else-if="view === 'calendar'" :board="visible!" :full-board="board" @open="openTask" />
        <BoardTimeline v-else :board="visible!" :full-board="board" :can-write="canWrite" @open="openTask" @reschedule="onReschedule" />

        <Transition :css="false" @enter="motion.panelRight.onEnter" @leave="motion.panelRight.onLeave">
          <TaskPanel v-if="taskId" :key="'panel'" :board="board" :task-id="taskId" :order="order" :can-write="canWrite" :can-validate="canValidate" @close="closeTask" @navigate="(id) => router.replace({ query: { ...route.query, task: id } })" />
        </Transition>
      </div>

      <UiMenu v-model:open="ctxOpen" :items="ctxItems" :virtual-rect="ctxPos" :width="230" />
      <template v-if="ctxLive">
        <TaskStatusPicker v-if="picker === 'status'" v-model="pStatus" v-model:open="pickerOpen" :board="board" :virtual-rect="ctxPos" />
        <TaskPriorityPicker v-if="picker === 'priority'" v-model="pPriority" v-model:open="pickerOpen" :virtual-rect="ctxPos" />
        <TaskAssigneePicker v-if="picker === 'assignee'" v-model="pAssignees" v-model:open="pickerOpen" :virtual-rect="ctxPos" />
        <TaskLabelPicker v-if="picker === 'label'" v-model="pLabels" v-model:open="pickerOpen" :labels="board.labels" :can-create="canWrite" :virtual-rect="ctxPos" @create="pCreateLabel" />
        <TaskDatePicker v-if="picker === 'due'" v-model="pDue" v-model:open="pickerOpen" :virtual-rect="ctxPos" />
      </template>
      <BoardSettingsDialog v-model:open="settingsOpen" :board="board" />
    </template>
  </div>
</template>

<style scoped>
.board-title { font-weight: 600; padding: 0 6px; cursor: text; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.board-title-input { font-weight: 600; height: 26px; padding: 0 6px; border: 1px solid var(--border-focus); border-radius: var(--r-sm); background: var(--bg-input); outline: none; }
.members { display: flex; margin-right: 8px; }
.members > * + * { margin-left: -5px; }
.board-area { position: relative; flex: 1; min-height: 0; display: flex; flex-direction: column; overflow: hidden; }
.loading { display: flex; gap: 12px; padding: 16px; }
.skel-col { width: 300px; display: flex; flex-direction: column; gap: 8px; }
@media (max-width: 639px) {
  .members, .new-btn { display: none; }
}
</style>
