<script setup lang="ts">
import { clone } from '~/utils/clone'
import { VueDraggable } from 'vue-draggable-plus'
import { Plus, MoreHorizontal, Pencil, Trash2, CircleCheckBig, GripVertical } from 'lucide-vue-next'
import type { Board, Column, Task } from '~/types'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { columnProgress, positionBetween, allTasks } from '~/composables/useBoard'
import { useBreakpoint } from '~/composables/useBreakpoint'
import { useI18n } from '~/composables/useI18n'

const props = defineProps<{
  board: Board
  fullBoard: Board
  canWrite: boolean
  focusedId?: string | null
}>()
const emit = defineEmits<{
  (e: 'open', t: Task): void
  (e: 'toggle', t: Task, ev: MouseEvent): void
  (e: 'menu', t: Task, ev: MouseEvent): void
  (e: 'move', taskId: string, columnId: string, position: number): void
  (e: 'move-column', columnId: string, position: number): void
  (e: 'create', columnId: string, title: string, atTop: boolean): void
  (e: 'create-column', name: string): void
  (e: 'rename-column', id: string, name: string): void
  (e: 'delete-column', col: Column): void
  (e: 'set-done-column', id: string): void
}>()

const { isTouch } = useBreakpoint()
const { t } = useI18n()

// Copie locale manipulée par SortableJS ; resynchronisée hors glisser-déposer.
const cols = ref<Column[]>([])
const dragging = ref(false)
watch(() => props.board, (b) => { if (!dragging.value) cols.value = clone(b.columns) }, { immediate: true, deep: true })

const subtasksOf = computed(() => {
  const m = new Map<string, Task[]>()
  for (const tk of allTasks(props.fullBoard)) {
    if (!tk.parent_id) continue
    if (!m.has(tk.parent_id)) m.set(tk.parent_id, [])
    m.get(tk.parent_id)!.push(tk)
  }
  return m
})

function onTaskEnd(evt: any) {
  const id = evt.item?.dataset?.taskId
  dragging.value = false
  if (!id) return
  nextTick(() => {
    for (const col of cols.value) {
      const i = col.tasks.findIndex((x) => x.id === id)
      if (i === -1) continue
      const before = col.tasks[i - 1]?.position
      const after = col.tasks[i + 1]?.position
      const original = allTasks(props.fullBoard).find((x) => x.id === id)
      if (original && original.column_id === col.id && evt.oldIndex === evt.newIndex && evt.from === evt.to) return
      emit('move', id, col.id, positionBetween(before, after))
      return
    }
  })
}

function onColumnEnd(evt: any) {
  dragging.value = false
  const i = evt.newIndex
  if (i == null || evt.oldIndex === evt.newIndex) return
  const col = cols.value[i]
  emit('move-column', col.id, positionBetween(cols.value[i - 1]?.position, cols.value[i + 1]?.position))
}

// Renommage de colonne en ligne
const renaming = ref<string | null>(null)
const draft = ref('')
function startRename(c: Column) {
  renaming.value = c.id
  draft.value = c.name
  nextTick(() => (document.querySelector('.col-rename') as HTMLInputElement | null)?.select())
}
function commitRename() {
  const id = renaming.value
  renaming.value = null
  const c = cols.value.find((x) => x.id === id)
  if (id && c && draft.value.trim() && draft.value.trim() !== c.name) emit('rename-column', id, draft.value.trim())
}

const menuOpen = ref<Record<string, boolean>>({})
function columnMenu(c: Column): MenuItem[] {
  const isDone = props.board.automations?.on_complete_move_to === c.id
  return [
    { label: t('common.rename'), icon: Pencil, onSelect: () => startRename(c) },
    { label: t('board.markAsDone'), icon: CircleCheckBig, checked: isDone, onSelect: () => emit('set-done-column', c.id) },
    { divider: true },
    { label: t('common.delete'), icon: Trash2, danger: true, onSelect: () => emit('delete-column', c) },
  ]
}

const topAdd = ref<Record<string, boolean>>({})
const addingColumn = ref(false)
const newColumn = ref('')
function submitColumn() {
  if (newColumn.value.trim()) emit('create-column', newColumn.value.trim())
  newColumn.value = ''
  addingColumn.value = false
}

const sortableOpts = computed(() => ({
  animation: 180,
  delay: isTouch.value ? 220 : 0,
  delayOnTouchOnly: true,
  touchStartThreshold: 6,
  ghostClass: 'card-ghost',
  chosenClass: 'card-chosen',
  dragClass: 'card-drag',
  forceFallback: true,
  fallbackOnBody: true,
  scrollSensitivity: 90,
  bubbleScroll: true,
  emptyInsertThreshold: 48,
}))
</script>

<template>
  <div class="kanban">
   <div class="kanban-cols">
    <VueDraggable
      v-model="cols"
      class="kanban-sortable"
      :animation="200"
      handle=".col-handle"
      :disabled="!canWrite || isTouch"
      direction="horizontal"
      ghost-class="col-ghost"
      @start="dragging = true"
      @end="onColumnEnd"
    >
      <section v-for="col in cols" :key="col.id" class="col" :data-column-id="col.id">
        <header class="col-head">
          <span class="col-handle" :class="{ grab: canWrite && !isTouch }">
            <GripVertical v-if="canWrite && !isTouch" :size="12" class="col-grip" />
            <TaskStatusIcon v-bind="columnProgress(fullBoard, col.id)" :size="14" />
          </span>
          <input
            v-if="renaming === col.id"
            v-model="draft"
            class="col-rename"
            @keydown.enter="commitRename"
            @keydown.esc="renaming = null"
            @blur="commitRename"
          />
          <span v-else class="col-name truncate" @dblclick="canWrite && startRename(col)">{{ col.name }}</span>
          <span class="col-count tabular">{{ col.tasks.length }}</span>
          <span class="spacer" />
          <template v-if="canWrite">
            <UiMenu v-model:open="menuOpen[col.id]" :items="columnMenu(col)">
              <button class="col-act" :aria-label="$t('common.more')"><MoreHorizontal :size="14" /></button>
            </UiMenu>
            <button class="col-act" :aria-label="$t('board.addTask')" @click="topAdd[col.id] = true"><Plus :size="14" /></button>
          </template>
        </header>

        <div class="col-body">
          <BoardQuickAdd
            v-if="topAdd[col.id]"
            start-open
            :placeholder="$t('task.titlePlaceholder')"
            @create="(title) => emit('create', col.id, title, true)"
            @close="topAdd[col.id] = false"
          />
          <VueDraggable
            v-model="col.tasks"
            class="col-list"
            :data-column-id="col.id"
            group="tasks"
            :disabled="!canWrite"
            v-bind="sortableOpts"
            @start="dragging = true"
            @end="onTaskEnd"
          >
            <BoardCard
              v-for="tk in col.tasks"
              :key="tk.id"
              :task="tk"
              :subtasks="subtasksOf.get(tk.id) ?? []"
              :focused="focusedId === tk.id"
              :can-write="canWrite"
              @open="emit('open', $event)"
              @toggle="(task, ev) => emit('toggle', task, ev)"
              @menu="(task, ev) => emit('menu', task, ev)"
            />
          </VueDraggable>
          <BoardQuickAdd v-if="canWrite" :placeholder="$t('task.titlePlaceholder')" @create="(title) => emit('create', col.id, title, false)" />
        </div>
      </section>

    </VueDraggable>
        <div v-if="canWrite" class="col col-new">
          <form v-if="addingColumn" class="col-new-form" @submit.prevent="submitColumn">
            <UiInput v-model="newColumn" size="sm" :placeholder="$t('board.columnName')" autofocus @keydown.esc="addingColumn = false" @blur="submitColumn" />
          </form>
          <button v-else class="col-new-btn" @click="addingColumn = true"><Plus :size="14" /> {{ $t('board.addColumn') }}</button>
        </div>
   </div>
  </div>
</template>

<style scoped>
.kanban {
  flex: 1;
  min-height: 0;
  overflow-x: auto;
  overflow-y: hidden;
  padding: 12px 16px 0;
  scroll-snap-type: x proximity;
}
.kanban-cols, .kanban-sortable { display: flex; gap: 12px; height: 100%; align-items: flex-start; }
.kanban-cols { width: max-content; padding-right: 16px; }
.col {
  flex: none;
  width: 300px;
  max-height: 100%;
  display: flex;
  flex-direction: column;
  border-radius: var(--r-lg);
  background: var(--bg-column);
  scroll-snap-align: start;
}
.col-head {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 40px;
  padding: 0 8px 0 10px;
  flex: none;
}
.col-handle { display: flex; align-items: center; gap: 2px; position: relative; }
.col-handle.grab { cursor: grab; }
.col-grip { position: absolute; left: -12px; color: var(--text-4); opacity: 0; transition: opacity var(--dur-fast); }
.col-head:hover .col-grip { opacity: 1; }
.col-name { font-weight: 600; font-size: var(--fs-md); }
.col-count { color: var(--text-3); font-size: var(--fs-sm); }
.col-rename {
  flex: 1; min-width: 0; height: 24px; padding: 0 6px; font-weight: 600;
  border: 1px solid var(--border-focus); border-radius: var(--r-xs); background: var(--bg-input); outline: none;
}
.col-act {
  width: 24px; height: 24px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-3);
  opacity: 0; transition: opacity var(--dur-fast), background var(--dur-fast);
}
.col:hover .col-act { opacity: 1; }
.col-act:hover { background: var(--bg-active); color: var(--text); }
.col-body { flex: 1; min-height: 0; overflow-y: auto; padding: 0 8px 10px; display: flex; flex-direction: column; gap: 6px; }
.col-list { display: flex; flex-direction: column; gap: 6px; min-height: 56px; flex: 1 0 auto; }
.col-new { background: transparent; }
.col-new-btn {
  display: flex; align-items: center; gap: 6px; width: 100%; height: 40px; padding: 0 12px;
  border-radius: var(--r-lg); color: var(--text-3); font-weight: 500; border: 1px dashed var(--border-strong);
}
.col-new-btn:hover { color: var(--text); background: var(--bg-hover); }
.col-new-form { padding: 6px; }

:deep(.card-ghost) { opacity: 0.35; }
:deep(.card-ghost) > * { visibility: hidden; }
:deep(.card-ghost) { background: var(--accent-soft) !important; box-shadow: inset 0 0 0 1px var(--border-focus) !important; }
:deep(.card-drag) { transform: rotate(2deg) scale(1.02); box-shadow: var(--shadow-drag) !important; opacity: 1 !important; }
:deep(.col-ghost) { opacity: 0.4; }

@media (pointer: coarse) {
  .col-act { opacity: 1; }
}
@media (max-width: 639px) {
  .kanban { padding: 8px 10px 0; scroll-snap-type: x mandatory; }
  .col { width: calc(100vw - 44px); }
}
</style>
