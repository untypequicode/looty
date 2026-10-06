import { clone } from '~/utils/clone'
import { computed, toRaw, type Ref } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import type { Board, Column, Label, Task, Priority } from '~/types'
import { api } from './api'
import { boardKey, taskKey, upsertTaskInBoards, removeTaskFromBoards } from './useRealtime'
import { confirmDialog } from './useConfirm'
import { translate } from './useI18n'
import { toast } from './useToast'
import { trashedToast } from './useTrash'

export function useBoardQuery(boardId: Ref<string | null | undefined>) {
  return useQuery({
    queryKey: computed(() => boardKey(boardId.value || '')),
    queryFn: () => api<Board>(`/api/boards/${boardId.value}`, { silent: true }),
    enabled: computed(() => !!boardId.value),
  })
}

/** Toutes les tâches d'un tableau à plat. */
export function allTasks(board: Board | null | undefined): Task[] {
  return board ? board.columns.flatMap((c) => c.tasks) : []
}

/** Progression visuelle d'une colonne (0 = début du flux, 1 = fin). */
export function columnProgress(board: Board | null | undefined, columnId: string): { progress: number; done: boolean; backlog: boolean } {
  if (!board) return { progress: 0, done: false, backlog: false }
  const doneId = board.automations?.on_complete_move_to
  const idx = board.columns.findIndex((c) => c.id === columnId)
  const n = board.columns.length
  if (columnId === doneId) return { progress: 1, done: true, backlog: false }
  if (idx <= 0 && n > 2) return { progress: 0, done: false, backlog: true }
  const lastOpen = doneId ? n - 2 : n - 1
  return { progress: Math.min(0.85, idx / Math.max(1, lastOpen + 1)), done: false, backlog: false }
}

/** Position fractionnaire entre deux voisins. */
export function positionBetween(before?: number, after?: number): number {
  if (before == null && after == null) return 65536
  if (before == null) return after! / 2
  if (after == null) return before + 65536
  return (before + after) / 2
}

/**
 * Mutations d'un tableau. Toutes appliquent d'abord une mise à jour optimiste
 * du cache, puis réconcilient avec la réponse serveur (ou annulent en cas d'erreur).
 */
export function useBoardActions(boardId: Ref<string | null | undefined>) {
  const qc = useQueryClient()
  const board = computed(() => (boardId.value ? qc.getQueryData<Board>(boardKey(boardId.value)) : undefined))

  function snapshot() {
    return boardId.value ? clone(toRaw(qc.getQueryData<Board>(boardKey(boardId.value)))) : undefined
  }
  function restore(snap: Board | undefined) {
    if (snap && boardId.value) qc.setQueryData(boardKey(boardId.value), snap)
  }
  function patch(fn: (b: Board) => void) {
    if (!boardId.value) return
    const data = qc.getQueryData<Board>(boardKey(boardId.value))
    if (!data) return
    const copy = clone(data)
    fn(copy)
    qc.setQueryData(boardKey(boardId.value), copy)
  }
  function findTask(id: string): Task | undefined {
    return allTasks(qc.getQueryData<Board>(boardKey(boardId.value || ''))).find((t) => t.id === id)
  }

  async function withRollback<T>(mutate: () => void, request: () => Promise<T>): Promise<T> {
    const snap = snapshot()
    mutate()
    try {
      return await request()
    } catch (e) {
      restore(snap)
      throw e
    }
  }

  // ---- Tâches ----
  async function createTask(columnId: string, data: Partial<Task> & { title: string; labels?: string[]; assignees?: string[]; at_top?: boolean }) {
    const t = await api<Task>(`/api/boards/columns/${columnId}/tasks`, {
      method: 'POST',
      body: {
        title: data.title,
        description: data.description ?? null,
        priority: data.priority ?? 0,
        due_date: data.due_date ?? null,
        start_date: data.start_date ?? null,
        labels: data.labels ?? [],
        assignees: data.assignees ?? [],
        at_top: data.at_top ?? false,
      },
    })
    upsertTaskInBoards(qc, t)
    return t
  }

  async function updateTask(id: string, data: Partial<Pick<Task, 'title' | 'description' | 'priority' | 'due_date' | 'start_date' | 'is_per_person' | 'require_all_validations' | 'is_completed' | 'event_id' | 'event_start'>>) {
    const t = await withRollback(
      () => patch((b) => { for (const c of b.columns) for (const t of c.tasks) if (t.id === id) Object.assign(t, data) }),
      () => api<Task>(`/api/boards/tasks/${id}`, { method: 'PUT', body: data }),
    )
    upsertTaskInBoards(qc, t)
    return t
  }

  async function moveTask(id: string, columnId: string, position: number) {
    const t = await withRollback(
      () => patch((b) => {
        let task: Task | undefined
        for (const c of b.columns) {
          const i = c.tasks.findIndex((x) => x.id === id)
          if (i !== -1) task = c.tasks.splice(i, 1)[0]
        }
        const dest = b.columns.find((c) => c.id === columnId)
        if (task && dest) {
          task.column_id = columnId
          task.position = position
          dest.tasks.push(task)
          dest.tasks.sort((a, c) => a.position - c.position)
        }
      }),
      () => api<Task>(`/api/boards/tasks/${id}/move`, { method: 'PUT', body: { column_id: columnId, position } }),
    )
    upsertTaskInBoards(qc, t)
    return t
  }

  async function toggleDone(id: string, selfId?: string) {
    const t = await withRollback(
      () => patch((b) => {
        for (const c of b.columns) for (const t of c.tasks) {
          if (t.id !== id) continue
          if (t.is_per_person && selfId) {
            t.validations = t.validations.includes(selfId) ? t.validations.filter((v) => v !== selfId) : [...t.validations, selfId]
          } else t.is_completed = !t.is_completed
        }
      }),
      () => api<Task>(`/api/boards/tasks/${id}/toggle_validation`, { method: 'PUT' }),
    )
    upsertTaskInBoards(qc, t)
    return t
  }

  async function deleteTask(task: Pick<Task, 'id' | 'title'>, confirm = true) {
    if (confirm) {
      const ok = await confirmDialog({ title: translate('task.deleteTitle'), message: task.title, confirmLabel: translate('common.delete'), danger: true })
      if (!ok) return false
    }
    await withRollback(() => removeTaskFromBoards(qc, task.id), () => api(`/api/boards/tasks/${task.id}`, { method: 'DELETE' }))
    qc.removeQueries({ queryKey: taskKey(task.id) })
    trashedToast('tasks', task.id)
    return true
  }

  async function setLabel(taskId: string, label: Label, on: boolean) {
    await withRollback(
      () => patch((b) => {
        for (const c of b.columns) for (const t of c.tasks) {
          if (t.id !== taskId) continue
          t.labels = on ? [...t.labels.filter((l) => l.id !== label.id), label] : t.labels.filter((l) => l.id !== label.id)
        }
      }),
      () => api(`/api/boards/tasks/${taskId}/labels/${label.id}`, { method: on ? 'POST' : 'DELETE' }),
    )
  }

  async function setLabels(taskId: string, labelIds: string[], labels: Label[]) {
    const cur = findTask(taskId)?.labels.map((l) => l.id) ?? []
    const add = labelIds.filter((x) => !cur.includes(x))
    const remove = cur.filter((x) => !labelIds.includes(x))
    await Promise.all([
      ...add.map((id) => setLabel(taskId, labels.find((l) => l.id === id)!, true)),
      ...remove.map((id) => setLabel(taskId, labels.find((l) => l.id === id) ?? ({ id } as Label), false)),
    ])
  }

  async function setAssignee(taskId: string, userId: string, on: boolean) {
    await withRollback(
      () => patch((b) => {
        for (const c of b.columns) for (const t of c.tasks) {
          if (t.id !== taskId) continue
          t.assignees = on ? [...new Set([...t.assignees, userId])] : t.assignees.filter((a) => a !== userId)
        }
      }),
      () => api(`/api/boards/tasks/${taskId}/assignees/${userId}`, { method: on ? 'POST' : 'DELETE' }),
    )
  }

  async function setAssignees(taskId: string, userIds: string[]) {
    const cur = findTask(taskId)?.assignees ?? []
    await Promise.all([
      ...userIds.filter((u) => !cur.includes(u)).map((u) => setAssignee(taskId, u, true)),
      ...cur.filter((u) => !userIds.includes(u)).map((u) => setAssignee(taskId, u, false)),
    ])
  }

  async function createSubtask(parentId: string, title: string) {
    const t = await api<Task>(`/api/boards/tasks/${parentId}/subtasks`, { method: 'POST', body: { title } })
    upsertTaskInBoards(qc, t)
    return t
  }

  // ---- Colonnes ----
  async function createColumn(name: string) {
    const c = await api<Column>(`/api/boards/${boardId.value}/columns`, { method: 'POST', body: { name } })
    patch((b) => { if (!b.columns.some((x) => x.id === c.id)) b.columns.push({ ...c, tasks: [] }) })
    return c
  }
  async function renameColumn(id: string, name: string) {
    await withRollback(
      () => patch((b) => { const c = b.columns.find((x) => x.id === id); if (c) c.name = name }),
      () => api(`/api/boards/columns/${id}`, { method: 'PUT', body: { name } }),
    )
  }
  async function moveColumn(id: string, position: number) {
    await withRollback(
      () => patch((b) => {
        const c = b.columns.find((x) => x.id === id)
        if (c) c.position = position
        b.columns.sort((a, c2) => a.position - c2.position)
      }),
      () => api(`/api/boards/columns/${id}/move`, { method: 'PUT', body: { position } }),
    )
  }
  async function deleteColumn(col: Pick<Column, 'id' | 'name'> & { tasks?: Task[] }) {
    const ok = await confirmDialog({
      title: translate('board.deleteColumnTitle', { name: col.name }),
      message: col.tasks?.length ? translate('board.deleteColumnTasks', { count: col.tasks.length }) : undefined,
      confirmLabel: translate('common.delete'),
      danger: true,
    })
    if (!ok) return
    await withRollback(
      () => patch((b) => { b.columns = b.columns.filter((c) => c.id !== col.id) }),
      () => api(`/api/boards/columns/${col.id}`, { method: 'DELETE' }),
    )
  }

  // ---- Labels ----
  async function createLabel(name: string, color: string) {
    const l = await api<Label>(`/api/boards/${boardId.value}/labels`, { method: 'POST', body: { name, color } })
    patch((b) => { if (!b.labels.some((x) => x.id === l.id)) b.labels.push(l) })
    return l
  }
  async function updateLabel(id: string, name: string, color: string) {
    await api(`/api/boards/labels/${id}`, { method: 'PUT', body: { name, color } })
  }
  async function deleteLabel(l: Label) {
    const ok = await confirmDialog({ title: translate('board.deleteLabelTitle', { name: l.name }), confirmLabel: translate('common.delete'), danger: true })
    if (!ok) return
    await api(`/api/boards/labels/${l.id}`, { method: 'DELETE' })
  }

  async function saveAutomations(automations: Board['automations']) {
    patch((b) => { b.automations = automations })
    await api(`/api/boards/${boardId.value}`, { method: 'PUT', body: { automations } })
  }

  return {
    board, findTask,
    createTask, updateTask, moveTask, toggleDone, deleteTask, setLabel, setLabels, setAssignee, setAssignees, createSubtask,
    createColumn, renameColumn, moveColumn, deleteColumn,
    createLabel, updateLabel, deleteLabel, saveAutomations,
  }
}

export const PRIORITIES: Priority[] = [0, 1, 2, 3, 4]
export const LABEL_COLORS = ['#7170ff', '#4ea7fc', '#26b5ce', '#4cb782', '#f2c94c', '#f2994a', '#eb5757', '#e06ba8', '#b18cff', '#8a8f98']
