<script setup lang="ts">
import { useGroups } from '~/composables/useGroups'
import { Kanban, ChevronRight, CalendarDays, CalendarClock, CalendarRange, Repeat, Tag, UserRound, Users, LockKeyhole, Maximize2, Minimize2 } from 'lucide-vue-next'
import { useQueryClient } from '@tanstack/vue-query'
import { useLocalStorage } from '@vueuse/core'
import type { Board, Priority, TaskRepeat } from '~/types'
import type { CalEvent } from '~/composables/useCalendar'
import { isTimed, timeOf } from '~/utils/dates'
import { useUi } from '~/composables/useUi'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { useBoardActions, columnProgress } from '~/composables/useBoard'
import { boardKey } from '~/composables/useRealtime'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'

const { createTaskOpen, createTaskDefaults } = useUi()
const { wsId, boards, member, workspace } = useWorkspace()
const { data: allGroups } = useGroups(wsId)
const groupName = (id: string) => allGroups.value?.find((g) => g.id === id)?.name ?? '…'
const { t, formatDate } = useI18n()
const qc = useQueryClient()
const route = useRoute()

/** Formulaire complet (toutes les propriétés du panneau) plutôt que la création rapide ; mémorisé. */
const full = useLocalStorage('looty.createTask.full', false)

const boardId = ref<string | null>(null)
const columnId = ref('')
const title = ref('')
/** Description en HTML (même éditeur que le panneau). */
const description = ref('')
const priority = ref<Priority>(0)
const assignees = ref<string[]>([])
/** Groupes reliés à la tâche (elle suivra leurs membres). */
const groupIds = ref<string[] | undefined>([])
const labelIds = ref<string[]>([])
const startDate = ref<string | null>(null)
const dueDate = ref<string | null>(null)
const repeat = ref<TaskRepeat | null>(null)
/** Occurrence d'événement liée : l'échéance prend son début. */
const linkedEvent = ref<CalEvent | null>(null)
const perPerson = ref(false)
const requireAll = ref(true)
const createMore = ref(false)
/** Tâche personnelle : visible de soi seul (d'office dans un tableau personnel). */
const personal = ref(false)
const saving = ref(false)
const board = ref<Board | null>(null)
const titleInput = ref<{ focus: () => void } | null>(null)
const eventOpen = ref(false)
const repeatOpen = ref(false)
const { createTask, createLabel } = useBoardActions(boardId)

const boardItems = computed(() => (boards.data.value ?? []).map((b) => ({ value: b.id, label: b.name })))
const canBePersonal = computed(() => !!board.value && !board.value.private_to && !workspace.data.value?.personal)

const REPEATS: TaskRepeat[] = ['daily', 'weekly', 'biweekly', 'monthly', 'yearly']
const repeatItems = computed(() => [{ value: '', label: t('taskRepeat.none') }, ...REPEATS.map((r) => ({ value: r, label: t(`taskRepeat.${r}`) }))])
const repeatModel = computed({
  get: () => repeat.value ?? '',
  set: (v: string) => {
    if (v && !dueDate.value && !startDate.value) { toast.error(t('taskRepeat.needsDate')); return }
    repeat.value = (v || null) as TaskRepeat | null
  },
})
const dateLabel = (d: string) => (isTimed(d) ? `${formatDate(d)} · ${timeOf(d)}` : formatDate(d))

function linkEvent(e: CalEvent | null) {
  linkedEvent.value = e
  if (e) dueDate.value = e.starts_at
}

const escapeHtml = (s: string) => s.replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' })[c]!)
const textToHtml = (s: string) => (s.trim() ? `<p>${escapeHtml(s.trim()).replace(/\n/g, '</p><p>')}</p>` : '')

async function loadBoard(id: string | null) {
  board.value = null
  if (!id) return
  const loaded = await qc.fetchQuery({ queryKey: boardKey(id), queryFn: () => api<Board>(`/api/boards/${id}`) })
  board.value = loaded
  const cols = loaded.columns
  if (!cols.some((c) => c.id === columnId.value)) {
    // Par défaut : première colonne non « backlog » si possible.
    columnId.value = (cols.length > 2 ? cols[1] : cols[0])?.id ?? ''
  }
  labelIds.value = labelIds.value.filter((l) => board.value!.labels.some((x) => x.id === l))
}

watch(createTaskOpen, (v) => {
  if (!v) return
  title.value = createTaskDefaults.value.title ?? ''
  description.value = textToHtml(createTaskDefaults.value.description ?? '')
  priority.value = 0
  assignees.value = []
  groupIds.value = []
  personal.value = false
  labelIds.value = []
  startDate.value = null
  dueDate.value = createTaskDefaults.value.dueDate ?? null
  repeat.value = null
  linkedEvent.value = null
  perPerson.value = false
  requireAll.value = true
  columnId.value = createTaskDefaults.value.columnId ?? ''
  const fromRoute = route.params.board as string | undefined
  boardId.value = createTaskDefaults.value.boardId ?? fromRoute ?? boards.data.value?.[0]?.id ?? null
  loadBoard(boardId.value)
})
watch(boardId, (id, prev) => { if (prev !== undefined && id !== prev) loadBoard(id) })
// Une date retirée : la répétition n'a plus d'appui.
watch([startDate, dueDate], ([s, d]) => { if (!s && !d) repeat.value = null })

const column = computed(() => board.value?.columns.find((c) => c.id === columnId.value))

async function submit() {
  if (!title.value.trim() || !columnId.value || saving.value) return
  if (repeat.value && !dueDate.value && !startDate.value) { toast.error(t('taskRepeat.needsDate')); return }
  saving.value = true
  try {
    const task = await createTask(columnId.value, {
      title: title.value.trim(),
      description: description.value || null,
      priority: priority.value,
      assignees: assignees.value,
      groups: personal.value ? [] : groupIds.value ?? [],
      personal: personal.value,
      labels: labelIds.value,
      start_date: startDate.value,
      due_date: dueDate.value,
      repeat: repeat.value,
      event_id: linkedEvent.value?.event_id ?? null,
      event_start: linkedEvent.value?.starts_at ?? null,
      is_per_person: perPerson.value,
      require_all_validations: requireAll.value,
    })
    toast.success(t('task.created'), { action: { label: t('common.open'), run: () => navigateTo(links.task(wsId.value, boardId.value!, task.id)) } })
    if (createMore.value) {
      title.value = ''
      description.value = ''
      titleInput.value?.focus()
    } else createTaskOpen.value = false
  } finally {
    saving.value = false
  }
}

async function onCreateLabel(name: string, color: string) {
  const l = await createLabel(name, color)
  if (board.value) board.value.labels.push(l)
  labelIds.value = [...labelIds.value, l.id]
}
</script>

<template>
  <UiDialog v-model:open="createTaskOpen" :width="full ? 900 : 640" :closable="true">
    <template v-if="!boardItems.length">
      <UiEmpty :icon="Kanban" :title="t('board.noBoards')" :description="t('board.noBoardsHint')" />
    </template>
    <form v-else class="ct" :class="{ full }" @submit.prevent="submit" @keydown.meta.enter.prevent="submit" @keydown.ctrl.enter.prevent="submit">
      <div class="ct-path">
        <UiCombobox v-model="boardId" :items="boardItems" :placeholder="t('board.search')">
          <button type="button" class="chip"><Kanban :size="13" /> {{ boards.data.value?.find((b) => b.id === boardId)?.name ?? t('board.choose') }}</button>
        </UiCombobox>
        <ChevronRight :size="13" class="faint" />
        <span class="faint">{{ t('task.new') }}</span>
      </div>

      <div class="ct-body">
        <div class="ct-main">
          <UiInput ref="titleInput" v-model="title" class="ct-title" :placeholder="t('task.titlePlaceholder')" autofocus />
          <div class="ct-desc">
            <EditorLite v-model="description" :placeholder="t('task.descriptionPlaceholder')" submit-on-enter @submit="submit" />
          </div>

          <!-- Création rapide : les propriétés en pastilles. -->
          <template v-if="!full">
            <div v-if="board" class="ct-props">
              <TaskStatusPicker v-model="columnId" :board="board">
                <button type="button" class="chip"><TaskStatusIcon v-bind="columnProgress(board, columnId)" :size="14" /> {{ column?.name }}</button>
              </TaskStatusPicker>
              <TaskPriorityPicker v-model="priority">
                <button type="button" class="chip"><TaskPriorityIcon :priority="priority" :size="14" /> {{ t(`priority.${priority}`) }}</button>
              </TaskPriorityPicker>
              <TaskAssigneePicker v-model="assignees" v-model:groups="groupIds">
                <button type="button" class="chip">
                  <template v-if="groupIds?.length">{{ groupIds.map((g) => `@${groupName(g)}`).join(', ') }}<template v-if="assignees.length"> +</template></template>
                  <template v-if="assignees.length">
                    <span class="stack"><UiAvatar v-for="a in assignees.slice(0, 3)" :key="a" :name="member(a)?.name" :src="member(a)?.avatar_url" :size="16" ring /></span>
                    {{ assignees.length === 1 ? member(assignees[0])?.name : t('task.nAssignees', { count: assignees.length }) }}
                  </template>
                  <template v-else-if="!groupIds?.length"><UserRound :size="14" /> {{ t('task.assignee') }}</template>
                </button>
              </TaskAssigneePicker>
              <TaskLabelPicker v-model="labelIds" :labels="board.labels" can-create @create="onCreateLabel">
                <button type="button" class="chip">
                  <template v-if="labelIds.length">
                    <span v-for="l in board.labels.filter((x) => labelIds.includes(x.id)).slice(0, 3)" :key="l.id" class="dot" :style="{ background: l.color }" />
                    {{ labelIds.length === 1 ? board.labels.find((x) => x.id === labelIds[0])?.name : t('task.nLabels', { count: labelIds.length }) }}
                  </template>
                  <template v-else><Tag :size="14" /> {{ t('task.labels') }}</template>
                </button>
              </TaskLabelPicker>
              <TaskDatePicker v-if="startDate" v-model="startDate">
                <button type="button" class="chip"><CalendarClock :size="14" /> {{ dateLabel(startDate) }}</button>
              </TaskDatePicker>
              <TaskDatePicker v-model="dueDate">
                <button type="button" class="chip"><CalendarDays :size="14" /> {{ dueDate ? dateLabel(dueDate) : t('task.dueDate') }}</button>
              </TaskDatePicker>
              <!-- Réglés depuis le formulaire complet : restent visibles ici. -->
              <UiCombobox v-if="repeat" v-model:open="repeatOpen" v-model="repeatModel" :items="repeatItems" :width="230">
                <button type="button" class="chip"><Repeat :size="14" /> {{ t(`taskRepeat.${repeat}`) }}</button>
              </UiCombobox>
              <TaskEventPicker v-if="linkedEvent" v-model:open="eventOpen" linked @pick="linkEvent">
                <button type="button" class="chip"><CalendarRange :size="14" /> <span class="truncate">{{ linkedEvent.summary || t('calendar.untitled') }}</span></button>
              </TaskEventPicker>
              <button v-if="perPerson" type="button" class="chip on" :aria-pressed="true" @click="perPerson = false"><Users :size="13" /> {{ t('task.perPerson') }}</button>
              <button v-if="canBePersonal" type="button" class="chip" :class="{ on: personal }" :title="t('personalItem.createHint')" :aria-pressed="personal" @click="personal = !personal">
                <LockKeyhole :size="13" /> {{ t('personalItem.create') }}
              </button>
            </div>
            <div v-else class="ct-props"><span class="skeleton" style="width: 320px; height: 24px" /></div>
          </template>
        </div>

        <!-- Formulaire complet : les propriétés du panneau de la tâche. -->
        <div v-if="full" class="ct-side">
          <template v-if="board">
            <div class="prop">
              <span class="prop-label">{{ t('task.status') }}</span>
              <TaskStatusPicker v-model="columnId" :board="board">
                <button type="button" class="prop-val"><TaskStatusIcon v-bind="columnProgress(board, columnId)" :size="14" /> {{ column?.name }}</button>
              </TaskStatusPicker>
            </div>
            <div class="prop">
              <span class="prop-label">{{ t('task.priority') }}</span>
              <TaskPriorityPicker v-model="priority">
                <button type="button" class="prop-val"><TaskPriorityIcon :priority="priority" :size="14" /> {{ t(`priority.${priority}`) }}</button>
              </TaskPriorityPicker>
            </div>
            <div class="prop">
              <span class="prop-label">{{ t('task.assignees') }}</span>
              <TaskAssigneePicker v-model="assignees" v-model:groups="groupIds">
                <button type="button" class="prop-val wrap">
                  <span v-for="g in groupIds ?? []" :key="g" class="lbl" :title="t('memberGroups.linkedTitle')"><span class="dot" :style="{ background: allGroups?.find((x) => x.id === g)?.color }" />@{{ groupName(g) }}</span>
                  <template v-if="assignees.length">
                    <span class="stack"><UiAvatar v-for="a in assignees.slice(0, 4)" :key="a" :name="member(a)?.name" :src="member(a)?.avatar_url" :size="18" ring /></span>
                    <span class="truncate">{{ assignees.length === 1 ? member(assignees[0])?.name : t('task.nAssignees', { count: assignees.length }) }}</span>
                  </template>
                  <template v-else-if="!groupIds?.length"><UserRound :size="14" class="faint" /> <span class="faint">{{ t('task.unassigned') }}</span></template>
                </button>
              </TaskAssigneePicker>
            </div>
            <div class="prop">
              <span class="prop-label">{{ t('task.labels') }}</span>
              <TaskLabelPicker v-model="labelIds" :labels="board.labels" can-create @create="onCreateLabel">
                <button type="button" class="prop-val wrap">
                  <template v-if="labelIds.length">
                    <span v-for="l in board.labels.filter((x) => labelIds.includes(x.id))" :key="l.id" class="lbl"><span class="dot" :style="{ background: l.color }" />{{ l.name }}</span>
                  </template>
                  <template v-else><Tag :size="14" class="faint" /> <span class="faint">{{ t('task.addLabels') }}</span></template>
                </button>
              </TaskLabelPicker>
            </div>
            <div class="prop">
              <span class="prop-label">{{ t('task.startDate') }}</span>
              <TaskDatePicker v-model="startDate">
                <button type="button" class="prop-val"><CalendarClock :size="14" class="faint" /> <span :class="{ faint: !startDate }">{{ startDate ? formatDate(startDate) : t('task.noDate') }}</span></button>
              </TaskDatePicker>
            </div>
            <div class="prop">
              <span class="prop-label">{{ t('task.dueDate') }}</span>
              <TaskDatePicker v-model="dueDate">
                <button type="button" class="prop-val"><CalendarDays :size="14" class="faint" /> <span :class="{ faint: !dueDate }">{{ dueDate ? dateLabel(dueDate) : t('task.noDate') }}</span></button>
              </TaskDatePicker>
            </div>
            <div class="prop">
              <span class="prop-label">{{ t('taskRepeat.label') }}</span>
              <UiCombobox v-model:open="repeatOpen" v-model="repeatModel" :items="repeatItems" :width="230">
                <button type="button" class="prop-val" :title="t('taskRepeat.hint')">
                  <Repeat :size="14" class="faint" /> <span :class="{ faint: !repeat }">{{ repeat ? t(`taskRepeat.${repeat}`) : t('taskRepeat.none') }}</span>
                </button>
              </UiCombobox>
            </div>
            <div class="prop">
              <span class="prop-label">{{ t('planning.event') }}</span>
              <TaskEventPicker v-model:open="eventOpen" :linked="!!linkedEvent" @pick="linkEvent">
                <button type="button" class="prop-val">
                  <CalendarRange :size="14" class="faint" />
                  <span v-if="linkedEvent" class="truncate">{{ linkedEvent.summary || t('calendar.untitled') }}<span class="faint"> · {{ dateLabel(linkedEvent.starts_at) }}</span></span>
                  <span v-else class="faint">{{ t('planning.linkEvent') }}</span>
                </button>
              </TaskEventPicker>
            </div>

            <div class="prop-sep" />
            <label class="prop-switch">
              <span><Users :size="14" class="faint" /> {{ t('task.perPerson') }}</span>
              <UiSwitch v-model="perPerson" />
            </label>
            <p class="prop-hint">{{ t('task.perPersonHint') }}</p>
            <label v-if="perPerson" class="prop-switch">
              <span>{{ t('task.requireAll') }}</span>
              <UiSwitch v-model="requireAll" />
            </label>
            <template v-if="canBePersonal">
              <div class="prop-sep" />
              <label class="prop-switch">
                <span><LockKeyhole :size="14" class="faint" /> {{ t('personalItem.create') }}</span>
                <UiSwitch v-model="personal" />
              </label>
              <p class="prop-hint">{{ t('personalItem.createHint') }}</p>
            </template>
          </template>
          <span v-else class="skeleton" style="width: 100%; height: 200px" />
        </div>
      </div>
    </form>
    <template v-if="boardItems.length" #footer>
      <UiButton variant="ghost" size="sm" :icon="full ? Minimize2 : Maximize2" :aria-pressed="full" @click="full = !full">
        {{ full ? t('task.quickForm') : t('task.fullForm') }}
      </UiButton>
      <label class="ct-more"><UiSwitch v-model="createMore" /> {{ t('task.createMore') }}</label>
      <div class="spacer" />
      <UiButton variant="primary" :loading="saving" :disabled="!title.trim() || !columnId" @click="submit">
        {{ t('task.create') }} <UiKbd :keys="['mod', 'enter']" class="ct-kbd" />
      </UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.ct { display: flex; flex-direction: column; gap: 6px; margin-top: -8px; }
.ct-path { display: flex; align-items: center; gap: 6px; font-size: var(--fs-sm); margin-bottom: 6px; }
.ct-body { display: flex; flex-direction: column; }
.ct-main { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
.ct-title :deep(input) { font-size: 18px; font-weight: 600; letter-spacing: -0.015em; }
.ct-title { border: none !important; box-shadow: none !important; background: transparent !important; padding: 0 !important; height: 34px !important; }
.ct-desc { min-height: 60px; max-height: 40vh; overflow-y: auto; font-size: var(--fs-lg); line-height: 1.55; }
.ct.full .ct-body { display: grid; grid-template-columns: minmax(0, 1fr) 260px; gap: 20px; }
.ct.full .ct-desc { min-height: 220px; max-height: 55vh; }
.ct-side { display: flex; flex-direction: column; gap: 2px; padding: 12px; margin-right: -4px; border-radius: var(--r-md); border: 1px solid var(--border); background: var(--bg-sidebar); }
.ct-props { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 6px; }
.chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 26px;
  max-width: 240px;
  padding: 0 9px;
  border-radius: var(--r-sm);
  border: 1px solid var(--border-strong);
  color: var(--text-2);
  font-size: var(--fs-sm);
  font-weight: 500;
  background: var(--bg-elevated);
  transition: background var(--dur-fast), color var(--dur-fast);
}
.chip:hover { background: var(--bg-hover); color: var(--text); }
.chip.on { background: var(--bg-active); color: var(--text); border-color: var(--border-strong); }
.dot { width: 8px; height: 8px; border-radius: 50%; }
.stack { display: inline-flex; }
.stack > * + * { margin-left: -4px; }
.prop { display: flex; flex-direction: column; gap: 2px; margin-bottom: 6px; }
.prop-label { font-size: var(--fs-xs); color: var(--text-3); font-weight: 500; padding-left: 8px; }
.prop-val {
  display: flex; align-items: center; gap: 8px; width: 100%; min-height: 30px; padding: 4px 8px; border-radius: var(--r-sm);
  text-align: left; font-weight: 500; transition: background var(--dur-fast);
}
.prop-val.wrap { flex-wrap: wrap; }
.prop-val:hover { background: var(--bg-hover); }
.lbl { display: inline-flex; align-items: center; gap: 5px; height: 22px; padding: 0 8px; border-radius: var(--r-full); border: 1px solid var(--border); font-size: var(--fs-sm); }
.prop-sep { height: 1px; background: var(--border); margin: 8px 0; }
.prop-switch { display: flex; align-items: center; justify-content: space-between; gap: 8px; min-height: 30px; padding: 0 8px; cursor: pointer; font-weight: 500; }
.prop-switch > span { display: flex; align-items: center; gap: 8px; }
.prop-hint { margin: 0 8px 6px; font-size: var(--fs-xs); color: var(--text-3); line-height: 1.45; }
.ct-more { display: flex; align-items: center; gap: 8px; color: var(--text-2); font-size: var(--fs-sm); cursor: pointer; }
.ct-kbd { margin-left: 4px; opacity: 0.8; }
.ct-kbd :deep(.kbd) { background: rgba(255, 255, 255, 0.16); border-color: transparent; color: #fff; }
@media (max-width: 760px) {
  .ct.full .ct-body { grid-template-columns: 1fr; }
  .ct.full .ct-desc { min-height: 120px; }
  .ct-side { margin-right: 0; }
}
</style>
