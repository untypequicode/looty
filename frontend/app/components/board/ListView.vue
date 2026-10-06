<script setup lang="ts">
import { ChevronRight, Plus, ListTree } from 'lucide-vue-next'
import type { Board, Task } from '~/types'
import { columnProgress, allTasks } from '~/composables/useBoard'
import { useWorkspace } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'

const props = defineProps<{ board: Board; fullBoard: Board; canWrite: boolean; canValidate?: boolean; focusedId?: string | null }>()
const emit = defineEmits<{
  (e: 'open', t: Task): void
  (e: 'toggle', t: Task, ev: MouseEvent): void
  (e: 'menu', t: Task, ev: MouseEvent): void
  (e: 'create', columnId: string, title: string, atTop: boolean): void
}>()
const { member } = useWorkspace()
const { formatDate } = useI18n()
const collapsed = ref<Record<string, boolean>>({})
const adding = ref<string | null>(null)

const subCount = computed(() => {
  const m = new Map<string, [number, number]>()
  for (const tk of allTasks(props.fullBoard)) {
    if (!tk.parent_id) continue
    const cur = m.get(tk.parent_id) ?? [0, 0]
    m.set(tk.parent_id, [cur[0] + (tk.is_completed ? 1 : 0), cur[1] + 1])
  }
  return m
})

function dueTone(tk: Task) {
  if (!tk.due_date || tk.is_completed) return ''
  const diff = new Date(tk.due_date).getTime() - Date.now()
  return diff < 0 ? 'overdue' : diff < 2 * 86400000 ? 'soon' : ''
}
</script>

<template>
  <div class="list-view">
    <section v-for="col in board.columns" :key="col.id" class="lv-group">
      <header class="lv-head" @click="collapsed[col.id] = !collapsed[col.id]">
        <ChevronRight :size="13" class="lv-chev" :class="{ open: !collapsed[col.id] }" />
        <TaskStatusIcon v-bind="columnProgress(fullBoard, col.id)" :size="14" />
        <span class="lv-name">{{ col.name }}</span>
        <span class="lv-count tabular">{{ col.tasks.length }}</span>
        <span class="spacer" />
        <button v-if="canWrite" class="lv-add" @click.stop="adding = col.id; collapsed[col.id] = false"><Plus :size="14" /></button>
      </header>
      <div v-show="!collapsed[col.id]">
        <div v-if="adding === col.id" class="lv-quick">
          <BoardQuickAdd start-open :placeholder="$t('task.titlePlaceholder')" @create="(title) => emit('create', col.id, title, true)" @close="adding = null" />
        </div>
        <div
          v-for="tk in col.tasks"
          :key="tk.id"
          class="lv-row"
          :class="{ focused: focusedId === tk.id, done: tk.is_completed }"
          :data-task-id="tk.id"
          @click="emit('open', tk)"
          @contextmenu.prevent="emit('menu', tk, $event)"
        >
          <span class="lv-prio"><TaskPriorityIcon :priority="tk.priority" :size="14" /></span>
          <button class="lv-status" :disabled="!(canValidate ?? canWrite)" @click.stop="emit('toggle', tk, $event)">
            <TaskStatusIcon v-bind="tk.is_completed ? { done: true } : columnProgress(fullBoard, col.id)" :size="15" />
          </button>
          <span class="lv-title truncate">{{ tk.title }}</span>
          <span v-if="subCount.get(tk.id)" class="lv-sub"><ListTree :size="12" />{{ subCount.get(tk.id)![0] }}/{{ subCount.get(tk.id)![1] }}</span>
          <span class="lv-labels">
            <span v-for="l in tk.labels.slice(0, 3)" :key="l.id" class="lv-label"><span class="dot" :style="{ background: l.color }" />{{ l.name }}</span>
          </span>
          <span class="lv-due" :class="dueTone(tk)">{{ tk.due_date ? formatDate(tk.due_date) : '' }}</span>
          <span class="lv-assignees">
            <UiAvatar v-for="a in tk.assignees.slice(0, 3)" :key="a" :name="member(a)?.name" :src="member(a)?.avatar_url" :size="18" ring />
          </span>
        </div>
        <div v-if="!col.tasks.length && adding !== col.id" class="lv-empty">{{ $t('board.emptyColumn') }}</div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.list-view { flex: 1; overflow-y: auto; padding-bottom: 48px; }
.lv-group + .lv-group { margin-top: 2px; }
.lv-head {
  position: sticky;
  top: 0;
  z-index: 1;
  display: flex;
  align-items: center;
  gap: 8px;
  height: 36px;
  padding: 0 16px 0 12px;
  background: var(--bg-hover);
  backdrop-filter: blur(8px);
  cursor: pointer;
  user-select: none;
}
.lv-chev { color: var(--text-3); transition: transform var(--dur) var(--ease); }
.lv-chev.open { transform: rotate(90deg); }
.lv-name { font-weight: 600; }
.lv-count { color: var(--text-3); font-size: var(--fs-sm); }
.lv-add { width: 24px; height: 24px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-3); }
.lv-add:hover { background: var(--bg-active); color: var(--text); }
.lv-quick { padding: 6px 16px; }
.lv-row {
  display: flex;
  align-items: center;
  gap: 10px;
  height: 40px;
  padding: 0 16px 0 20px;
  border-bottom: 1px solid var(--border);
  cursor: pointer;
  transition: background var(--dur-fast);
}
.lv-row:hover { background: var(--bg-hover); }
.lv-row.focused { background: var(--bg-selected); box-shadow: inset 2px 0 0 var(--accent); }
.lv-row.done .lv-title { color: var(--text-3); }
.lv-prio { flex: none; display: grid; }
.lv-status { flex: none; display: grid; place-items: center; width: 20px; height: 20px; border-radius: 50%; }
.lv-status:hover:not(:disabled) { background: var(--bg-active); }
.lv-title { flex: 1; min-width: 0; font-weight: 500; }
.lv-sub { display: flex; align-items: center; gap: 3px; font-size: var(--fs-xs); color: var(--text-3); flex: none; }
.lv-labels { display: flex; gap: 4px; flex: none; }
.lv-label {
  display: inline-flex; align-items: center; gap: 4px; height: 20px; padding: 0 7px; border-radius: var(--r-full);
  border: 1px solid var(--border); font-size: 11.5px; color: var(--text-2); max-width: 120px; overflow: hidden; white-space: nowrap;
}
.lv-label .dot { width: 7px; height: 7px; border-radius: 50%; flex: none; }
.lv-due { font-size: var(--fs-sm); color: var(--text-2); width: 64px; text-align: right; flex: none; font-variant-numeric: tabular-nums; }
.lv-due.overdue { color: var(--red); }
.lv-due.soon { color: var(--orange); }
.lv-assignees { display: flex; width: 56px; justify-content: flex-end; flex: none; }
.lv-assignees > * + * { margin-left: -5px; }
.lv-empty { padding: 10px 20px; color: var(--text-4); font-size: var(--fs-sm); }
@media (max-width: 639px) {
  .lv-labels, .lv-sub { display: none; }
  .lv-row { padding: 0 12px; height: 46px; }
  .lv-due { width: auto; }
}
</style>
