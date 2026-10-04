import { computed, type Ref } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from './api'
import { toast } from './useToast'
import { translate } from './useI18n'

export interface TrashedTask {
  id: string
  title: string
  board_id: string
  board_name: string
  deleted_at: string
  deleted_by?: string | null
  deleted_by_name?: string | null
  subtasks: number
  expires_at: string
}
export interface TrashedPage {
  id: string
  title: string
  icon?: string | null
  parent_title?: string | null
  deleted_at: string
  deleted_by?: string | null
  deleted_by_name?: string | null
  subpages: number
  expires_at: string
}
export interface TrashList { retention_days: number; can_manage: boolean; tasks: TrashedTask[]; pages: TrashedPage[] }

export const trashKey = (ws: string) => ['ws', ws, 'trash'] as const

export function useTrash(ws: Ref<string>) {
  return useQuery({ queryKey: computed(() => trashKey(ws.value)), queryFn: () => api<TrashList>(`/api/workspaces/${ws.value}/trash`), enabled: computed(() => !!ws.value) })
}

/** Restauration ; le temps réel remet l'élément dans les listes. */
export async function restoreFromTrash(kind: 'tasks' | 'pages', id: string) {
  await api(`/api/trash/${kind}/${id}/restore`, { method: 'POST' })
}

/** Après une suppression : message « Déplacé dans la corbeille » avec « Annuler ». */
export function trashedToast(kind: 'tasks' | 'pages', id: string, onRestored?: () => void) {
  toast(translate('trash.moved'), {
    duration: 7000,
    action: {
      label: translate('trash.undo'),
      run: async () => {
        await restoreFromTrash(kind, id)
        toast.success(translate('trash.restored'))
        onRestored?.()
      },
    },
  })
}

export function useTrashInvalidate() {
  const qc = useQueryClient()
  return (ws: string) => qc.invalidateQueries({ queryKey: trashKey(ws) })
}
