<script setup lang="ts">
import { X, ChevronUp, ChevronDown, Link2, MoreHorizontal, Trash2, CalendarDays, CalendarClock, UserRound, Tag, Users, Kanban, CornerLeftUp, Copy } from 'lucide-vue-next'
import { useDebounceFn } from '@vueuse/core'
import type { Board, Task, Priority } from '~/types'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { useBoardActions, columnProgress, allTasks } from '~/composables/useBoard'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { useAuth } from '~/composables/useAuth'
import { useI18n } from '~/composables/useI18n'
import { useLayer } from '~/composables/useLayer'
import { useShortcuts } from '~/composables/useShortcuts'
import { useBreakpoint } from '~/composables/useBreakpoint'
import { motion } from '~/composables/useMotion'
import { toast } from '~/composables/useToast'

const props = defineProps<{ board: Board; taskId: string; order: string[]; canWrite: boolean }>()
const emit = defineEmits<{ (e: 'close'): void; (e: 'navigate', id: string): void }>()

const { wsId, member } = useWorkspace()
const { user } = useAuth()
const { t, formatDate, relativeTime } = useI18n()
const { isMobile } = useBreakpoint()
const boardId = computed(() => props.board.id)
const actions = useBoardActions(boardId)

const task = computed<Task | undefined>(() => allTasks(props.board).find((x) => x.id === props.taskId))
const parent = computed(() => (task.value?.parent_id ? allTasks(props.board).find((x) => x.id === task.value!.parent_id) : undefined))
const subtasks = computed(() => allTasks(props.board).filter((x) => x.parent_id === props.taskId).sort((a, b) => a.position - b.position))
const column = computed(() => props.board.columns.find((c) => c.id === task.value?.column_id))

const open = ref(true)
function close() { emit('close') }
useLayer(open, close, { modal: isMobile.value })

// --- Titre ---
const title = ref('')
const titleEl = ref<HTMLTextAreaElement | null>(null)
watch(() => task.value?.title, (v) => { if (v !== undefined && document.activeElement !== titleEl.value) title.value = v; nextTick(autosize) }, { immediate: true })
function autosize() {
  if (!titleEl.value) return
  titleEl.value.style.height = 'auto'
  titleEl.value.style.height = `${titleEl.value.scrollHeight}px`
}
async function saveTitle() {
  const v = title.value.trim()
  if (!task.value || !v || v === task.value.title) { title.value = task.value?.title ?? ''; return }
  await actions.updateTask(task.value.id, { title: v })
}

// --- Description (sauvegarde automatique) ---
const description = ref('')
const descFocused = ref(false)
watch(() => [props.taskId, task.value?.description] as const, ([, d]) => { if (!descFocused.value) description.value = d ?? '' }, { immediate: true })
const saveDesc = useDebounceFn(async () => {
  if (!task.value) return
  const v = description.value || null
  if (v === (task.value.description ?? null)) return
  await actions.updateTask(task.value.id, { description: v })
}, 900)
watch(description, () => { if (descFocused.value) saveDesc() })

// --- Propriétés ---
const columnModel = computed({
  get: () => task.value?.column_id ?? '',
  set: (cid: string) => {
    if (!task.value || cid === task.value.column_id) return
    const dest = props.board.columns.find((c) => c.id === cid)
    const max = dest?.tasks.reduce((m, x) => Math.max(m, x.position), 0) ?? 0
    actions.moveTask(task.value.id, cid, max + 65536)
  },
})
const priorityModel = computed({
  get: () => (task.value?.priority ?? 0) as Priority,
  set: (p: Priority) => task.value && actions.updateTask(task.value.id, { priority: p }),
})
const assigneeModel = computed({
  get: () => task.value?.assignees ?? [],
  set: (ids: string[]) => task.value && actions.setAssignees(task.value.id, ids),
})
const labelModel = computed({
  get: () => task.value?.labels.map((l) => l.id) ?? [],
  set: (ids: string[]) => task.value && actions.setLabels(task.value.id, ids, props.board.labels),
})
const dueModel = computed({
  get: () => task.value?.due_date ?? null,
  set: (d: string | null | undefined) => task.value && actions.updateTask(task.value.id, { due_date: d ?? null }),
})
const startModel = computed({
  get: () => task.value?.start_date ?? null,
  set: (d: string | null | undefined) => task.value && actions.updateTask(task.value.id, { start_date: d ?? null }),
})
async function createLabel(name: string, color: string) {
  if (!task.value) return
  const l = await actions.createLabel(name, color)
  await actions.setLabel(task.value.id, l, true)
}
function setPerPerson(v: boolean) { if (task.value) actions.updateTask(task.value.id, { is_per_person: v }) }
function setRequireAll(v: boolean) { if (task.value) actions.updateTask(task.value.id, { require_all_validations: v }) }

const myValidated = computed(() => !!task.value?.is_per_person && !!user.value && task.value.validations.includes(user.value.id))
const checkEl = ref<HTMLElement | null>(null)
function toggleDone() {
  if (!task.value || !props.canWrite) return
  motion.pulse(checkEl.value)
  actions.toggleDone(task.value.id, user.value?.id)
}

// --- Navigation ---
const idx = computed(() => props.order.indexOf(props.taskId))
const prevId = computed(() => (idx.value > 0 ? props.order[idx.value - 1] : null))
const nextId = computed(() => (idx.value >= 0 && idx.value < props.order.length - 1 ? props.order[idx.value + 1] : null))
useShortcuts([
  { keys: 'k', handler: () => prevId.value && emit('navigate', prevId.value) },
  { keys: 'j', handler: () => nextId.value && emit('navigate', nextId.value) },
  { keys: 'x', handler: toggleDone },
  { keys: 'mod+.', global: true, handler: copyLink },
])

const link = computed(() => `${location.origin}${links.task(wsId.value, props.board.id, props.taskId)}`)
function copyLink() {
  navigator.clipboard.writeText(link.value)
  toast.success(t('common.linkCopied'))
}
const menuOpen = ref(false)
const menu = computed<MenuItem[]>(() => [
  { label: t('common.copyLink'), icon: Link2, shortcut: ['mod', '.'], onSelect: copyLink },
  { label: t('task.copyId'), icon: Copy, onSelect: () => { navigator.clipboard.writeText(props.taskId); toast.success(t('common.copied')) } },
  ...(props.canWrite && task.value ? [{ divider: true }, { label: t('common.delete'), icon: Trash2, danger: true, onSelect: async () => { if (await actions.deleteTask(task.value!)) close() } }] : []),
])

// Si la tâche disparaît (supprimée par quelqu'un d'autre), on ferme.
watch(task, (v, old) => { if (!v && old) close() })

// Animation à chaque changement de tâche
const body = ref<HTMLElement | null>(null)
watch(() => props.taskId, () => nextTick(() => body.value && motion.stagger(body.value.children, { y: 4, each: 0.02 })))
</script>

<template>
  <aside class="panel" :class="{ mobile: isMobile }" role="dialog" :aria-label="task?.title">
    <header class="panel-head">
      <nav class="panel-crumbs">
        <NuxtLink :to="links.board(wsId, board.id)" class="crumb"><Kanban :size="13" /> {{ board.name }}</NuxtLink>
        <span class="crumb-sep">›</span>
        <span class="crumb"><TaskStatusIcon v-if="task" v-bind="columnProgress(board, task.column_id)" :size="13" /> {{ column?.name }}</span>
      </nav>
      <span class="spacer" />
      <UiTooltip :text="t('shortcuts.prevTask')" shortcut="k"><UiButton size="sm" variant="ghost" :icon="ChevronUp" :disabled="!prevId" @click="prevId && emit('navigate', prevId)" /></UiTooltip>
      <UiTooltip :text="t('shortcuts.nextTask')" shortcut="j"><UiButton size="sm" variant="ghost" :icon="ChevronDown" :disabled="!nextId" @click="nextId && emit('navigate', nextId)" /></UiTooltip>
      <UiTooltip :text="t('common.copyLink')"><UiButton size="sm" variant="ghost" :icon="Link2" @click="copyLink" /></UiTooltip>
      <UiMenu v-model:open="menuOpen" :items="menu" placement="bottom-end"><UiButton size="sm" variant="ghost" :icon="MoreHorizontal" /></UiMenu>
      <UiTooltip :text="t('common.close')" shortcut="esc"><UiButton size="sm" variant="ghost" :icon="X" @click="close" /></UiTooltip>
    </header>

    <div v-if="!task" class="panel-empty"><UiSpinner /></div>
    <div v-else class="panel-scroll">
      <div class="panel-grid">
        <div ref="body" class="panel-main">
          <NuxtLink v-if="parent" :to="links.task(wsId, board.id, parent.id)" class="parent-link"><CornerLeftUp :size="13" /> {{ parent.title }}</NuxtLink>
          <div class="title-row">
            <button ref="checkEl" class="big-check" :class="{ on: task.is_completed, partial: myValidated && !task.is_completed }" :disabled="!canWrite" :title="t('task.markDone')" @click="toggleDone">
              <svg v-if="task.is_completed || myValidated" width="12" height="12" viewBox="0 0 10 10"><path d="M2 5.2 4.1 7.2 8 3" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
            </button>
            <textarea
              ref="titleEl"
              v-model="title"
              class="title"
              rows="1"
              :readonly="!canWrite"
              :placeholder="t('task.titlePlaceholder')"
              @input="autosize"
              @keydown.enter.prevent="($event.target as HTMLElement).blur()"
              @blur="saveTitle"
            />
          </div>

          <div class="desc" @focusin="descFocused = true" @focusout="descFocused = false; saveDesc()">
            <EditorLite v-model="description" :editable="canWrite" :placeholder="t('task.descriptionPlaceholder')" />
          </div>

          <TaskSubtasks
            :subtasks="subtasks"
            :can-write="canWrite"
            @create="(title) => actions.createSubtask(task!.id, title)"
            @toggle="(s) => actions.toggleDone(s.id, user?.id)"
            @open="(s) => emit('navigate', s.id)"
          />
          <TaskAttachments :task-id="task.id" :can-write="canWrite" />
          <TaskActivity :task-id="task.id" :board="board" :can-write="canWrite" />
        </div>

        <div class="panel-props">
          <div class="prop">
            <span class="prop-label">{{ t('task.status') }}</span>
            <TaskStatusPicker v-model="columnModel" :board="board">
              <button class="prop-val" :disabled="!canWrite"><TaskStatusIcon v-bind="columnProgress(board, task.column_id)" :size="14" /> {{ column?.name }}</button>
            </TaskStatusPicker>
          </div>
          <div class="prop">
            <span class="prop-label">{{ t('task.priority') }}</span>
            <TaskPriorityPicker v-model="priorityModel">
              <button class="prop-val" :disabled="!canWrite"><TaskPriorityIcon :priority="task.priority" :size="14" /> {{ t(`priority.${task.priority}`) }}</button>
            </TaskPriorityPicker>
          </div>
          <div class="prop">
            <span class="prop-label">{{ t('task.assignees') }}</span>
            <TaskAssigneePicker v-model="assigneeModel">
              <button class="prop-val" :disabled="!canWrite">
                <template v-if="task.assignees.length">
                  <span class="avatars"><UiAvatar v-for="a in task.assignees.slice(0, 4)" :key="a" :name="member(a)?.name" :src="member(a)?.avatar_url" :size="18" ring /></span>
                  <span class="truncate">{{ task.assignees.length === 1 ? member(task.assignees[0])?.name : t('task.nAssignees', { count: task.assignees.length }) }}</span>
                </template>
                <template v-else><UserRound :size="14" class="faint" /> <span class="faint">{{ t('task.unassigned') }}</span></template>
              </button>
            </TaskAssigneePicker>
          </div>
          <div class="prop">
            <span class="prop-label">{{ t('task.labels') }}</span>
            <TaskLabelPicker v-model="labelModel" :labels="board.labels" :can-create="canWrite" @create="createLabel">
              <button class="prop-val wrap" :disabled="!canWrite">
                <template v-if="task.labels.length">
                  <span v-for="l in task.labels" :key="l.id" class="lbl"><span class="dot" :style="{ background: l.color }" />{{ l.name }}</span>
                </template>
                <template v-else><Tag :size="14" class="faint" /> <span class="faint">{{ t('task.addLabels') }}</span></template>
              </button>
            </TaskLabelPicker>
          </div>
          <div class="prop">
            <span class="prop-label">{{ t('task.startDate') }}</span>
            <TaskDatePicker v-model="startModel">
              <button class="prop-val" :disabled="!canWrite"><CalendarClock :size="14" class="faint" /> <span :class="{ faint: !task.start_date }">{{ task.start_date ? formatDate(task.start_date) : t('task.noDate') }}</span></button>
            </TaskDatePicker>
          </div>
          <div class="prop">
            <span class="prop-label">{{ t('task.dueDate') }}</span>
            <TaskDatePicker v-model="dueModel">
              <button class="prop-val" :disabled="!canWrite" :class="{ overdue: task.due_date && !task.is_completed && new Date(task.due_date) < new Date() }">
                <CalendarDays :size="14" class="faint" /> <span :class="{ faint: !task.due_date }">{{ task.due_date ? formatDate(task.due_date) : t('task.noDate') }}</span>
              </button>
            </TaskDatePicker>
          </div>

          <div class="prop-sep" />
          <label class="prop-switch">
            <span><Users :size="14" class="faint" /> {{ t('task.perPerson') }}</span>
            <UiSwitch :model-value="task.is_per_person" :disabled="!canWrite" @update:model-value="setPerPerson" />
          </label>
          <p class="prop-hint">{{ t('task.perPersonHint') }}</p>
          <label v-if="task.is_per_person" class="prop-switch">
            <span>{{ t('task.requireAll') }}</span>
            <UiSwitch :model-value="task.require_all_validations" :disabled="!canWrite" @update:model-value="setRequireAll" />
          </label>
          <div v-if="task.is_per_person && task.assignees.length" class="validations">
            <div v-for="a in task.assignees" :key="a" class="validation">
              <UiAvatar :name="member(a)?.name" :src="member(a)?.avatar_url" :size="18" />
              <span class="truncate">{{ member(a)?.name }}</span>
              <span class="v-state" :class="{ ok: task.validations.includes(a) }">{{ task.validations.includes(a) ? t('task.validated') : t('task.pending') }}</span>
            </div>
          </div>

          <div class="prop-sep" />
          <p class="prop-meta">{{ t('task.createdAgo', { time: relativeTime(task.created_at) }) }}</p>
          <p class="prop-meta">{{ t('task.updatedAgo', { time: relativeTime(task.updated_at) }) }}</p>
        </div>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.panel {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  z-index: 20;
  width: min(940px, calc(100% - 80px));
  display: flex;
  flex-direction: column;
  background: var(--bg-panel);
  border-left: 1px solid var(--border);
  box-shadow: -24px 0 48px -24px rgba(0, 0, 0, 0.4);
}
.panel.mobile { position: fixed; inset: 0; width: 100%; z-index: 60; border: none; padding-top: var(--safe-top); }
.panel-head { display: flex; align-items: center; gap: 2px; height: 46px; padding: 0 8px 0 16px; border-bottom: 1px solid var(--border); flex: none; }
.panel-crumbs { display: flex; align-items: center; gap: 6px; min-width: 0; font-size: var(--fs-sm); }
.crumb { display: inline-flex; align-items: center; gap: 6px; color: var(--text-2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
a.crumb:hover { color: var(--text); }
.crumb-sep { color: var(--text-4); }
.panel-empty { flex: 1; display: grid; place-items: center; }
.panel-scroll { flex: 1; overflow-y: auto; }
.panel-grid { display: grid; grid-template-columns: minmax(0, 1fr) 260px; min-height: 100%; }
.panel-main { padding: 24px 32px 64px; display: flex; flex-direction: column; gap: 22px; min-width: 0; }
.parent-link { display: inline-flex; align-items: center; gap: 6px; color: var(--text-3); font-size: var(--fs-sm); margin-bottom: -14px; }
.parent-link:hover { color: var(--text); }
.title-row { display: flex; align-items: flex-start; gap: 12px; }
.big-check {
  flex: none; margin-top: 7px; width: 20px; height: 20px; border-radius: 50%; border: 1.8px solid var(--text-3);
  display: grid; place-items: center; color: #fff; transition: background var(--dur-fast), border-color var(--dur-fast);
}
.big-check:hover:not(:disabled) { border-color: var(--accent); }
.big-check.on { background: var(--accent); border-color: var(--accent); }
.big-check.partial { border-color: var(--accent); border-style: dashed; color: var(--accent); }
.title {
  flex: 1; border: none; outline: none; resize: none; background: transparent; overflow: hidden;
  font-family: var(--font-display); font-size: 22px; font-weight: 600; letter-spacing: -0.018em; line-height: 1.35; padding: 0;
}
.title::placeholder { color: var(--text-4); }
.desc { min-height: 60px; margin-top: -8px; }
.desc :deep(.lite-editor) { font-size: 14.5px; }
.panel-props { border-left: 1px solid var(--border); padding: 20px 16px; display: flex; flex-direction: column; gap: 2px; background: var(--bg-sidebar); }
.prop { display: flex; flex-direction: column; gap: 2px; margin-bottom: 8px; }
.prop-label { font-size: var(--fs-xs); color: var(--text-3); font-weight: 500; padding-left: 8px; }
.prop-val {
  display: flex; align-items: center; gap: 8px; width: 100%; min-height: 30px; padding: 4px 8px; border-radius: var(--r-sm);
  text-align: left; font-weight: 500; transition: background var(--dur-fast);
}
.prop-val.wrap { flex-wrap: wrap; }
.prop-val:hover:not(:disabled) { background: var(--bg-hover); }
.prop-val:disabled { cursor: default; }
.prop-val.overdue { color: var(--red); }
.avatars { display: flex; }
.avatars > * + * { margin-left: -5px; }
.lbl { display: inline-flex; align-items: center; gap: 5px; height: 22px; padding: 0 8px; border-radius: var(--r-full); border: 1px solid var(--border); font-size: var(--fs-sm); }
.dot { width: 8px; height: 8px; border-radius: 50%; }
.prop-sep { height: 1px; background: var(--border); margin: 10px 0; }
.prop-switch { display: flex; align-items: center; justify-content: space-between; gap: 8px; min-height: 30px; padding: 0 8px; cursor: pointer; font-weight: 500; }
.prop-switch > span { display: flex; align-items: center; gap: 8px; }
.prop-hint { margin: 0 8px 6px; font-size: var(--fs-xs); color: var(--text-3); line-height: 1.45; }
.validations { display: flex; flex-direction: column; gap: 2px; padding: 0 8px; }
.validation { display: flex; align-items: center; gap: 8px; height: 28px; font-size: var(--fs-sm); }
.validation .truncate { flex: 1; }
.v-state { font-size: var(--fs-xs); color: var(--text-3); }
.v-state.ok { color: var(--green); }
.prop-meta { margin: 0 8px 4px; font-size: var(--fs-xs); color: var(--text-3); }
@media (max-width: 1023px) {
  .panel-grid { grid-template-columns: 1fr; }
  .panel-props { order: -1; border-left: none; border-bottom: 1px solid var(--border); display: grid; grid-template-columns: 1fr 1fr; gap: 4px 8px; padding: 12px; }
  .prop-sep, .prop-meta, .prop-hint { grid-column: 1 / -1; }
  .prop-switch, .validations { grid-column: 1 / -1; }
  .panel-main { padding: 18px 16px 48px; }
  .title { font-size: 19px; }
}
</style>
