import { watch, triggerRef } from 'vue'
import type { Ref } from 'vue'
import { useWebSocket } from '@vueuse/core'
import { useQueryClient } from '@tanstack/vue-query'
import type { Board } from '../types'

export const useKanbanWS = (
  wsUrl: Ref<string>, 
  activeBoard: Ref<Board | null>, 
  boards: Ref<Board[]>,
  taskStackIds: Ref<string[]>,
  pages?: Ref<any[]>,
  activePage?: Ref<any | null>
) => {
  const queryClient = useQueryClient()
  const { data: wsData } = useWebSocket(wsUrl, { autoReconnect: true })
  
  watch(wsData, (msg) => {
    if (msg) {
      try {
        const event = JSON.parse(msg)
        
        if (event.type === 'BOARD_CREATED' || event.type === 'BOARD_DELETED' || event.type === 'BOARD_UPDATED') {
          queryClient.invalidateQueries({ queryKey: ['workspaces'] })
        } else if (event.type === 'PAGE_CREATED' && pages) {
          if (pages.value && !pages.value.find(p => p.id === event.page.id)) {
            pages.value.push(event.page)
          }
        } else if (event.type === 'PAGE_UPDATED' && pages) {
          if (pages.value) {
            const idx = pages.value.findIndex(p => p.id === event.page.id)
            if (idx !== -1) pages.value[idx] = event.page
            if (activePage && activePage.value?.id === event.page.id) {
               activePage.value.title = event.page.title
               activePage.value.icon = event.page.icon
               activePage.value.parent_id = event.page.parent_id
               activePage.value.updated_by = event.page.updated_by
               activePage.value.updated_at = event.page.updated_at
            }
          }
        } else if (event.type === 'PAGE_DELETED' && pages) {
          if (pages.value) {
            const idx = pages.value.findIndex(p => p.id === event.page_id)
            if (idx !== -1) pages.value.splice(idx, 1)
          }
        }
        
        if (!activeBoard.value) {
           triggerRef(boards)
           if (pages) triggerRef(pages)
           return
        }
        
        if (event.type === 'COLUMN_CREATED') {
          if (event.column.board_id === activeBoard.value.id) {
            if (!activeBoard.value.columns.find((c: any) => c.id === event.column.id)) {
              activeBoard.value.columns.push({ ...event.column, tasks: [] })
            }
          }
        } else if (event.type === 'TASK_CREATED') {
          const col = activeBoard.value.columns.find((c: any) => c.id === event.task.column_id)
          if (col && !col.tasks.find((t: any) => t.id === event.task.id)) {
            col.tasks.push(event.task)
            col.tasks.sort((a: any, b: any) => a.position - b.position)
          }
        } else if (event.type === 'TASK_MOVED') {
          let existingTask = null;
          for (const col of activeBoard.value.columns) {
            const idx = col.tasks.findIndex((t: any) => t.id === event.task.id)
            if (idx !== -1) {
              existingTask = col.tasks[idx]
              if (col.id !== event.task.column_id) {
                 col.tasks.splice(idx, 1)
              }
            }
          }
          const destCol = activeBoard.value.columns.find((c: any) => c.id === event.task.column_id)
          if (destCol) {
            if (existingTask) {
               existingTask.position = event.task.position
               existingTask.column_id = event.task.column_id
               if (!destCol.tasks.some((t: any) => t.id === existingTask.id)) {
                  destCol.tasks.push(existingTask)
               }
            } else {
               destCol.tasks.push(event.task)
            }
            destCol.tasks.sort((a: any, b: any) => a.position - b.position)
          }
        } else if (event.type === 'TASK_UPDATED') {
          if (event.task) {
            const col = activeBoard.value.columns.find((c: any) => c.id === event.task.column_id)
            if (col) {
              const idx = col.tasks.findIndex((t: any) => t.id === event.task.id)
              if (idx !== -1) {
                // Preserve existing enrichment as backend update doesn't return them
                event.task.labels = event.task.labels || col.tasks[idx].labels || []
                event.task.assignees = event.task.assignees || col.tasks[idx].assignees || []
                event.task.validations = event.task.validations || col.tasks[idx].validations || []
                col.tasks.splice(idx, 1, event.task)
                // Update in taskStack is no longer needed because it's computed from activeBoard
              }
            }
          } else if (event.task_id) {
            queryClient.invalidateQueries({ queryKey: ['boards', activeBoard.value.id] })
          }
        } else if (event.type === 'TASK_DELETED') {
          const col = activeBoard.value.columns.find((c: any) => c.id === event.column_id)
          if (col) {
            const idx = col.tasks.findIndex((t: any) => t.id === event.task_id)
            if (idx !== -1) col.tasks.splice(idx, 1)
          }
        } else if (event.type === 'COLUMN_UPDATED') {
          const col = activeBoard.value.columns.find((c: any) => c.id === event.column.id)
          if (col) col.name = event.column.name
        } else if (event.type === 'COLUMN_MOVED') {
          const col = activeBoard.value.columns.find((c: any) => c.id === event.column.id)
          if (col) col.position = event.column.position
          activeBoard.value.columns.sort((a: any, b: any) => a.position - b.position)
        } else if (event.type === 'COLUMN_DELETED') {
          const idx = activeBoard.value.columns.findIndex((c: any) => c.id === event.column_id)
          if (idx !== -1) activeBoard.value.columns.splice(idx, 1)
        } else if (event.type === 'TASK_VALIDATION_UPDATED') {
           const col = activeBoard.value.columns.find((c: any) => c.id === event.task.column_id)
           if (col) {
              const idx = col.tasks.findIndex((t: any) => t.id === event.task.id)
              if (idx !== -1) {
                 col.tasks.splice(idx, 1, event.task)
              }
           }
           }
        
        // Invalidate active board query so VueQuery fetches the fresh data
        if (activeBoard.value?.id) {
           queryClient.invalidateQueries({ queryKey: ['boards', activeBoard.value.id] })
        }
        
        triggerRef(boards)
        triggerRef(taskStackIds)
        if (pages) triggerRef(pages)
        if (activePage) triggerRef(activePage)

      } catch (e) {
        console.error("Erreur WS:", e)
      }
    }
  })

  return { wsData }
}
