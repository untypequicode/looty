import { useQueryClient } from '@tanstack/vue-query'
import type { BoardSummary } from '~/types'
import { api } from './api'
import { useWorkspaceId, wsKeys, links } from './useWorkspace'
import { confirmDialog } from './useConfirm'
import { translate } from './useI18n'
import { toast } from './useToast'

/** Opérations sur la liste des tableaux du workspace. */
export function useBoards() {
  const qc = useQueryClient()
  const wsId = useWorkspaceId()
  const route = useRoute()

  async function createBoard(name: string, withDefaultColumns = true, ws = wsId.value) {
    const b = await api<BoardSummary>(`/api/workspaces/${ws}/boards`, {
      method: 'POST',
      body: { name, with_default_columns: withDefaultColumns },
    })
    qc.setQueryData<BoardSummary[]>(wsKeys.boards(ws), (old) => (old && !old.some((x) => x.id === b.id) ? [...old, b] : old))
    await navigateTo(links.board(ws, b.id))
    return b
  }

  async function renameBoard(id: string, name: string) {
    qc.setQueryData<BoardSummary[]>(wsKeys.boards(wsId.value), (old) => old?.map((b) => (b.id === id ? { ...b, name } : b)))
    await api(`/api/boards/${id}`, { method: 'PUT', body: { name } })
  }

  async function deleteBoard(board: Pick<BoardSummary, 'id' | 'name'>) {
    const ok = await confirmDialog({
      title: translate('board.deleteTitle', { name: board.name }),
      message: translate('board.deleteMessage'),
      confirmLabel: translate('common.delete'),
      danger: true,
      typeToConfirm: board.name,
    })
    if (!ok) return false
    await api(`/api/boards/${board.id}`, { method: 'DELETE' })
    qc.setQueryData<BoardSummary[]>(wsKeys.boards(wsId.value), (old) => old?.filter((b) => b.id !== board.id))
    qc.removeQueries({ queryKey: ['board', board.id] })
    toast.success(translate('board.deleted'))
    if (route.path.includes(board.id)) await navigateTo(links.home(wsId.value))
    return true
  }

  return { createBoard, renameBoard, deleteBoard }
}
