<script setup lang="ts">
import { Kanban, ChevronRight, CalendarDays, Tag, UserRound } from 'lucide-vue-next'
import { useQueryClient } from '@tanstack/vue-query'
import type { Board, Priority } from '~/types'
import { useUi } from '~/composables/useUi'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { useBoardActions, columnProgress } from '~/composables/useBoard'
import { boardKey } from '~/composables/useRealtime'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'

const { createTaskOpen, createTaskDefaults } = useUi()
const { wsId, boards, member } = useWorkspace()
const { t, formatDate } = useI18n()
const qc = useQueryClient()
const route = useRoute()

const boardId = ref<string | null>(null)
const columnId = ref('')
const title = ref('')
const description = ref('')
const priority = ref<Priority>(0)
const assignees = ref<string[]>([])
const labelIds = ref<string[]>([])
const dueDate = ref<string | null>(null)
const createMore = ref(false)
const saving = ref(false)
const board = ref<Board | null>(null)
const titleInput = ref<{ focus: () => void } | null>(null)
const { createTask, createLabel } = useBoardActions(boardId)

const boardItems = computed(() => (boards.data.value ?? []).map((b) => ({ value: b.id, label: b.name })))

async function loadBoard(id: string | null) {
  board.value = null
  if (!id) return
  board.value = await qc.fetchQuery({ queryKey: boardKey(id), queryFn: () => api<Board>(`/api/boards/${id}`) })
  const cols = board.value.columns
  if (!cols.some((c) => c.id === columnId.value)) {
    // Par défaut : première colonne non « backlog » si possible.
    columnId.value = (cols.length > 2 ? cols[1] : cols[0])?.id ?? ''
  }
  labelIds.value = labelIds.value.filter((l) => board.value!.labels.some((x) => x.id === l))
}

watch(createTaskOpen, (v) => {
  if (!v) return
  title.value = createTaskDefaults.value.title ?? ''
  description.value = createTaskDefaults.value.description ?? ''
  priority.value = 0
  assignees.value = []
  labelIds.value = []
  dueDate.value = createTaskDefaults.value.dueDate ?? null
  columnId.value = createTaskDefaults.value.columnId ?? ''
  const fromRoute = route.params.board as string | undefined
  boardId.value = createTaskDefaults.value.boardId ?? fromRoute ?? boards.data.value?.[0]?.id ?? null
  loadBoard(boardId.value)
})
watch(boardId, (id, prev) => { if (prev !== undefined && id !== prev) loadBoard(id) })

const column = computed(() => board.value?.columns.find((c) => c.id === columnId.value))

async function submit() {
  if (!title.value.trim() || !columnId.value || saving.value) return
  saving.value = true
  try {
    const task = await createTask(columnId.value, {
      title: title.value.trim(),
      description: description.value.trim() ? `<p>${description.value.trim().replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' })[c]!).replace(/\n/g, '</p><p>')}</p>` : null,
      priority: priority.value,
      assignees: assignees.value,
      labels: labelIds.value,
      due_date: dueDate.value,
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
  <UiDialog v-model:open="createTaskOpen" :width="640" :closable="true">
    <template v-if="!boardItems.length">
      <UiEmpty :icon="Kanban" :title="t('board.noBoards')" :description="t('board.noBoardsHint')" />
    </template>
    <form v-else class="ct" @submit.prevent="submit" @keydown.meta.enter.prevent="submit" @keydown.ctrl.enter.prevent="submit">
      <div class="ct-path">
        <UiCombobox v-model="boardId" :items="boardItems" :placeholder="t('board.search')">
          <button type="button" class="chip"><Kanban :size="13" /> {{ boards.data.value?.find((b) => b.id === boardId)?.name ?? t('board.choose') }}</button>
        </UiCombobox>
        <ChevronRight :size="13" class="faint" />
        <span class="faint">{{ t('task.new') }}</span>
      </div>

      <UiInput ref="titleInput" v-model="title" class="ct-title" :placeholder="t('task.titlePlaceholder')" autofocus />
      <textarea v-model="description" class="ct-desc" :placeholder="t('task.descriptionPlaceholder')" rows="3" />

      <div v-if="board" class="ct-props">
        <TaskStatusPicker v-model="columnId" :board="board">
          <button type="button" class="chip"><TaskStatusIcon v-bind="columnProgress(board, columnId)" :size="14" /> {{ column?.name }}</button>
        </TaskStatusPicker>
        <TaskPriorityPicker v-model="priority">
          <button type="button" class="chip"><TaskPriorityIcon :priority="priority" :size="14" /> {{ t(`priority.${priority}`) }}</button>
        </TaskPriorityPicker>
        <TaskAssigneePicker v-model="assignees">
          <button type="button" class="chip">
            <template v-if="assignees.length">
              <span class="stack"><UiAvatar v-for="a in assignees.slice(0, 3)" :key="a" :name="member(a)?.name" :src="member(a)?.avatar_url" :size="16" ring /></span>
              {{ assignees.length === 1 ? member(assignees[0])?.name : t('task.nAssignees', { count: assignees.length }) }}
            </template>
            <template v-else><UserRound :size="14" /> {{ t('task.assignee') }}</template>
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
        <TaskDatePicker v-model="dueDate">
          <button type="button" class="chip"><CalendarDays :size="14" /> {{ dueDate ? formatDate(dueDate) : t('task.dueDate') }}</button>
        </TaskDatePicker>
      </div>
      <div v-else class="ct-props"><span class="skeleton" style="width: 320px; height: 24px" /></div>
    </form>
    <template v-if="boardItems.length" #footer>
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
.ct-title :deep(input) { font-size: 18px; font-weight: 600; letter-spacing: -0.015em; }
.ct-title { border: none !important; box-shadow: none !important; background: transparent !important; padding: 0 !important; height: 34px !important; }
.ct-desc {
  border: none;
  outline: none;
  resize: none;
  background: transparent;
  font-size: var(--fs-lg);
  color: var(--text);
  min-height: 60px;
  line-height: 1.55;
}
.ct-desc::placeholder { color: var(--text-3); }
.ct-props { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 6px; }
.chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 26px;
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
.dot { width: 8px; height: 8px; border-radius: 50%; }
.stack { display: inline-flex; }
.stack > * + * { margin-left: -4px; }
.ct-more { display: flex; align-items: center; gap: 8px; color: var(--text-2); font-size: var(--fs-sm); cursor: pointer; }
.ct-kbd { margin-left: 4px; opacity: 0.8; }
.ct-kbd :deep(.kbd) { background: rgba(255, 255, 255, 0.16); border-color: transparent; color: #fff; }
</style>
