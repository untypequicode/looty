<script setup lang="ts">
import { NodeViewWrapper, nodeViewProps } from '@tiptap/vue-3'
import { Kanban, List, ArrowUpRight, Plus, GripVertical, Database } from 'lucide-vue-next'
import type { Board, Task } from '~/types'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useAuth } from '~/composables/useAuth'
import { useAccess } from '~/composables/useAccess'
import { useWorkspace, links, wsKeys } from '~/composables/useWorkspace'
import { useBoardQuery, useBoardActions } from '~/composables/useBoard'
import { promptDialog } from '~/composables/useConfirm'
import { useQueryClient } from '@tanstack/vue-query'

/**
 * Bloc « tableau » d'un document : un vrai tableau de l'espace, en kanban ou en liste,
 * modifiable sur place (tâches cochées, déplacées, ajoutées) et tenu à jour en direct.
 */
const props = defineProps(nodeViewProps)
// En lecture : choix de vue et de filtre gardés localement (la page n'est pas modifiée).
const local = ref<Record<string, unknown>>({})
const attr = (k: string) => local.value[k] ?? props.node.attrs[k]
const { t } = useI18n()
const { user } = useAuth()
const { wsId, boards } = useWorkspace()
const { allows } = useAccess(wsId)
const qc = useQueryClient()
const boardId = computed(() => (props.node.attrs.boardId as string) || null)
const { data: board, isError } = useBoardQuery(boardId)
const actions = useBoardActions(boardId)
const view = computed(() => (attr('view') === 'list' ? 'list' : 'kanban'))
const filter = computed(() => (attr('filter') as 'all' | 'open' | 'mine') ?? 'all')
// `isEditable` n'est pas réactif : relu à chaque mise à jour de l'éditeur.
const editable = ref(props.editor.isEditable)
const syncEditable = () => { editable.value = props.editor.isEditable }
props.editor.on('update', syncEditable)
props.editor.on('transaction', syncEditable)
onBeforeUnmount(() => { props.editor.off('update', syncEditable); props.editor.off('transaction', syncEditable) })
const canWrite = computed(() => editable.value && allows('boards', 'edit'))

/** Tâches filtrées (ouvertes, ou les miennes) ; sous-tâches masquées comme sur le tableau. */
const visible = computed<Board | null>(() => {
  const b = board.value
  if (!b) return null
  const me = user.value?.id
  const keep = (x: Task) => (filter.value === 'all' || (filter.value === 'open' ? !x.is_completed : !!me && x.assignees.includes(me)))
  return { ...b, columns: b.columns.map((c) => ({ ...c, tasks: c.tasks.filter((x) => !x.parent_id && keep(x)) })) }
})
const count = computed(() => visible.value?.columns.reduce((n, c) => n + c.tasks.length, 0) ?? 0)

function set(attrs: Record<string, unknown>) {
  if (editable.value) props.updateAttributes(attrs)
  else local.value = { ...local.value, ...attrs }
}
const open = (task: Task) => navigateTo(links.task(wsId.value, boardId.value!, task.id))
function onCreate(columnId: string, title: string, atTop: boolean) {
  actions.createTask(columnId, { title, at_top: atTop, assignees: filter.value === 'mine' && user.value ? [user.value.id] : [] })
}
async function newBoard() {
  const name = await promptDialog({ title: t('blocks.newBoard'), placeholder: t('blocks.boardName'), confirmLabel: t('common.create') })
  if (!name) return
  const b = await api<{ id: string }>(`/api/workspaces/${wsId.value}/boards`, { method: 'POST', body: { name, with_default_columns: true } })
  qc.invalidateQueries({ queryKey: wsKeys.boards(wsId.value) })
  set({ boardId: b.id })
}
</script>

<template>
  <NodeViewWrapper class="bb" :class="{ selected }">
    <div class="bb-card" contenteditable="false">
      <!-- Choix du tableau -->
      <div v-if="!boardId" class="bb-pick">
        <div class="bb-pick-title"><Database :size="16" /> {{ t('blocks.pickBoard') }}</div>
        <div class="bb-pick-list">
          <button v-for="b in boards.data.value ?? []" :key="b.id" class="bb-pick-item" :disabled="!editable" @click="set({ boardId: b.id })"><Kanban :size="14" /> {{ b.name }}</button>
          <button v-if="editable && allows('boards', 'manage')" class="bb-pick-item new" @click="newBoard"><Plus :size="14" /> {{ t('blocks.newBoard') }}</button>
        </div>
      </div>
      <p v-else-if="isError" class="bb-empty">{{ t('canvas.cardUnavailable') }}</p>
      <template v-else-if="visible">
        <header class="bb-head">
          <span v-if="editable" class="bb-grip" data-drag-handle><GripVertical :size="14" /></span>
          <NuxtLink :to="links.board(wsId, boardId!)" class="bb-name"><Kanban :size="15" /> {{ visible.name }}</NuxtLink>
          <span class="bb-count">{{ t('blocks.taskCount', { count }) }}</span>
          <span class="bb-space" />
          <UiSegmented :model-value="filter" :options="[{ value: 'all', label: t('blocks.all') }, { value: 'open', label: t('blocks.open') }, { value: 'mine', label: t('blocks.mine') }]" @update:model-value="(v) => set({ filter: v })" />
          <UiSegmented :model-value="view" :options="[{ value: 'kanban', icon: Kanban, title: 'Kanban' }, { value: 'list', icon: List, title: t('blocks.list') }]" @update:model-value="(v) => set({ view: v })" />
          <UiTooltip :text="t('blocks.openBoard')"><UiButton size="sm" variant="ghost" :icon="ArrowUpRight" :to="links.board(wsId, boardId!)" :aria-label="t('blocks.openBoard')" /></UiTooltip>
        </header>
        <div class="bb-body" :class="view">
          <BoardKanban
            v-if="view === 'kanban'"
            :board="visible"
            :full-board="board!"
            :can-write="canWrite"
            :can-validate="canWrite"
            @open="open"
            @toggle="(tk) => actions.toggleDone(tk.id, user?.id)"
            @move="(id, col, pos) => actions.moveTask(id, col, pos)"
            @create="onCreate"
          />
          <BoardListView v-else :board="visible" :full-board="board!" :can-write="canWrite" :can-validate="canWrite" @open="open" @toggle="(tk) => actions.toggleDone(tk.id, user?.id)" @create="onCreate" />
        </div>
      </template>
      <div v-else class="bb-loading"><UiSpinner /></div>
    </div>
  </NodeViewWrapper>
</template>

<style scoped>
.bb { margin: 1.2em 0; }
.bb-card { border-radius: var(--r-lg); background: var(--bg-column, var(--bg-panel)); box-shadow: var(--shadow-card); overflow: hidden; }
.bb.selected .bb-card { box-shadow: 0 0 0 1.5px var(--border-focus); }
.bb-head { display: flex; align-items: center; gap: 8px; padding: 8px 10px; border-bottom: 1px solid var(--border); flex-wrap: wrap; }
.bb-grip { color: var(--text-3); cursor: grab; display: grid; place-items: center; }
.bb-name { display: inline-flex; align-items: center; gap: 6px; font-weight: 600; color: var(--text); text-decoration: none; }
.bb-name:hover { color: var(--accent-text); }
.bb-count { color: var(--text-3); font-size: var(--fs-sm); }
.bb-space { flex: 1; }
.bb-body { max-height: 560px; overflow: auto; }
.bb-body.kanban { padding: 10px 6px; }
.bb-body.kanban :deep(.kanban) { height: auto; min-height: 220px; }
.bb-pick { padding: 16px; display: flex; flex-direction: column; gap: 10px; }
.bb-pick-title { display: flex; align-items: center; gap: 8px; font-weight: 600; }
.bb-pick-list { display: flex; flex-wrap: wrap; gap: 6px; }
.bb-pick-item { display: inline-flex; align-items: center; gap: 6px; height: 30px; padding: 0 10px; border-radius: var(--r-md); background: var(--bg-panel); box-shadow: inset 0 0 0 1px var(--border); color: var(--text); }
.bb-pick-item:hover:not(:disabled) { background: var(--bg-hover); }
.bb-pick-item.new { color: var(--accent-text); box-shadow: inset 0 0 0 1px dashed var(--border-strong); }
.bb-empty, .bb-loading { padding: 20px; color: var(--text-3); display: grid; place-items: center; }
</style>
