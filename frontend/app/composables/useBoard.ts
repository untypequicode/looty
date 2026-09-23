import { ref, computed } from 'vue'
import { useQuery, useMutation, useQueryClient } from '@tanstack/vue-query'
import { useSessionStorage } from '@vueuse/core'
import type { Board, Column } from '../types'

export const useBoard = () => {
  const queryClient = useQueryClient()
  const activeBoardId = useSessionStorage<string | null>('looty_active_board', null)

  const { data: activeBoard, isLoading: isLoadingBoard } = useQuery({
    queryKey: ['boards', activeBoardId],
    queryFn: async () => {
      if (!activeBoardId.value) return null
      const response = await $fetch(`/api/boards/${activeBoardId.value}`)
      return response as Board
    },
    enabled: computed(() => !!activeBoardId.value)
  })

  const selectBoard = (id: string | null) => {
    activeBoardId.value = id
  }

  const createBoardMutation = useMutation({
    mutationFn: async ({ workspaceId, name }: { workspaceId: string, name: string }) => {
      return await $fetch(`/api/workspaces/${workspaceId}/boards`, {
        method: 'POST',
        body: { name }
      }) as Board
    },
    // Invalidation will be handled globally or via websocket
  })

  const renameBoardMutation = useMutation({
    mutationFn: async ({ id, name }: { id: string, name: string }) => {
      return await $fetch(`/api/boards/${id}`, {
        method: 'PUT',
        body: { name }
      }) as Board
    }
  })

  const deleteBoardMutation = useMutation({
    mutationFn: async (id: string) => {
      return await $fetch(`/api/boards/${id}`, {
        method: 'DELETE'
      })
    },
    onSuccess: (_, id) => {
      if (activeBoardId.value === id) {
        activeBoardId.value = null
      }
    }
  })

  const createColumnMutation = useMutation({
    mutationFn: async ({ boardId, name }: { boardId: string, name: string }) => {
      return await $fetch(`/api/boards/${boardId}/columns`, {
        method: 'POST',
        body: { name }
      }) as Column
    }
  })

  const renameColumnMutation = useMutation({
    mutationFn: async ({ id, name }: { id: string, name: string }) => {
      return await $fetch(`/api/boards/columns/${id}`, {
        method: 'PUT',
        body: { name }
      }) as Column
    }
  })

  const deleteColumnMutation = useMutation({
    mutationFn: async (id: string) => {
      return await $fetch(`/api/boards/columns/${id}`, {
        method: 'DELETE'
      })
    }
  })
  
  const moveColumnMutation = useMutation({
    mutationFn: async ({ id, position }: { id: string, position: number }) => {
      return await $fetch(`/api/boards/columns/${id}/move`, {
        method: 'PUT',
        body: { position }
      })
    }
  })

  const saveBoardSettingsMutation = useMutation({
    mutationFn: async ({ id, automations }: { id: string, automations: any }) => {
      return await $fetch(`/api/boards/${id}`, {
        method: 'PUT',
        body: { automations }
      })
    }
  })

  const createLabelMutation = useMutation({
    mutationFn: async ({ boardId, name, color }: { boardId: string, name: string, color: string }) => {
      return await $fetch(`/api/boards/${boardId}/labels`, {
        method: 'POST',
        body: { name, color }
      })
    }
  })
  
  const editLabelMutation = useMutation({
    mutationFn: async ({ labelId, name, color }: { labelId: string, name: string, color: string }) => {
      return await $fetch(`/api/boards/labels/${labelId}`, {
        method: 'PUT',
        body: { name, color }
      })
    }
  })

  const deleteLabelMutation = useMutation({
    mutationFn: async (labelId: string) => {
      return await $fetch(`/api/boards/labels/${labelId}`, {
        method: 'DELETE'
      })
    }
  })

  return {
    activeBoardId,
    activeBoard,
    isLoadingBoard,
    selectBoard,
    createBoard: createBoardMutation.mutateAsync,
    renameBoard: renameBoardMutation.mutateAsync,
    deleteBoard: deleteBoardMutation.mutateAsync,
    createColumn: createColumnMutation.mutateAsync,
    renameColumn: renameColumnMutation.mutateAsync,
    deleteColumn: deleteColumnMutation.mutateAsync,
    moveColumn: moveColumnMutation.mutateAsync,
    saveBoardSettings: saveBoardSettingsMutation.mutateAsync,
    createLabel: createLabelMutation.mutateAsync,
    editLabel: editLabelMutation.mutateAsync,
    deleteLabel: deleteLabelMutation.mutateAsync
  }
}
