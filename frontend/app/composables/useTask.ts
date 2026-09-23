import { ref } from 'vue'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import type { Task } from '../types'

export const useTask = () => {
  const queryClient = useQueryClient()
  
  const createTaskMutation = useMutation({
    mutationFn: async ({ columnId, taskData }: { columnId: string, taskData: Partial<Task> }) => {
      return await $fetch(`/api/boards/columns/${columnId}/tasks`, {
        method: 'POST',
        body: taskData
      }) as Task
    }
  })

  const updateTaskMutation = useMutation({
    mutationFn: async ({ id, taskData }: { id: string, taskData: Partial<Task> }) => {
      return await $fetch(`/api/boards/tasks/${id}`, {
        method: 'PUT',
        body: taskData
      }) as Task
    }
  })

  const deleteTaskMutation = useMutation({
    mutationFn: async (id: string) => {
      return await $fetch(`/api/boards/tasks/${id}`, {
        method: 'DELETE'
      })
    }
  })

  const moveTaskMutation = useMutation({
    mutationFn: async ({ id, columnId, position, previousColumnId, clearPreviousColumnId }: { 
      id: string, 
      columnId: string, 
      position: number,
      previousColumnId?: string | null,
      clearPreviousColumnId?: boolean
    }) => {
      return await $fetch(`/api/boards/tasks/${id}/move`, {
        method: 'PUT',
        body: { 
          column_id: columnId, 
          position,
          previous_column_id: previousColumnId,
          clear_previous_column_id: clearPreviousColumnId
        }
      })
    }
  })

  const toggleTaskValidationMutation = useMutation({
    mutationFn: async (id: string) => {
      return await $fetch(`/api/boards/tasks/${id}/toggle_validation`, {
        method: 'PUT'
      }) as Task
    }
  })

  const createSubtaskMutation = useMutation({
    mutationFn: async ({ parentId, title }: { parentId: string, title: string }) => {
      return await $fetch(`/api/boards/tasks/${parentId}/subtasks`, {
        method: 'POST',
        body: { title }
      }) as Task
    }
  })

  const toggleTaskLabelMutation = useMutation({
    mutationFn: async ({ taskId, labelId, action }: { taskId: string, labelId: string, action: 'add' | 'remove' }) => {
      if (action === 'add') {
        return await $fetch(`/api/boards/tasks/${taskId}/labels/${labelId}`, { method: 'POST' })
      } else {
        return await $fetch(`/api/boards/tasks/${taskId}/labels/${labelId}`, { method: 'DELETE' })
      }
    }
  })

  const toggleTaskAssigneeMutation = useMutation({
    mutationFn: async ({ taskId, userId, action }: { taskId: string, userId: string, action: 'add' | 'remove' }) => {
      if (action === 'add') {
        return await $fetch(`/api/boards/tasks/${taskId}/assignees/${userId}`, { method: 'POST' })
      } else {
        return await $fetch(`/api/boards/tasks/${taskId}/assignees/${userId}`, { method: 'DELETE' })
      }
    }
  })

  return {
    createTask: createTaskMutation.mutateAsync,
    updateTask: updateTaskMutation.mutateAsync,
    deleteTask: deleteTaskMutation.mutateAsync,
    moveTask: moveTaskMutation.mutateAsync,
    toggleTaskValidation: toggleTaskValidationMutation.mutateAsync,
    createSubtask: createSubtaskMutation.mutateAsync,
    toggleTaskLabel: toggleTaskLabelMutation.mutateAsync,
    toggleTaskAssignee: toggleTaskAssigneeMutation.mutateAsync
  }
}
