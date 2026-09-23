import { watch } from 'vue'
import type { Ref } from 'vue'
import { useWebSocket } from '@vueuse/core'
import type { Board } from '../types'

export const useKanbanWS = (
  wsUrl: string, 
  activeBoard: Ref<Board | null>, 
  boards: Ref<Board[]>,
  taskStack: Ref<any[]>
) => {
  const { data: wsData } = useWebSocket(wsUrl, { autoReconnect: true })
  
  watch(wsData, (msg) => {
    if (msg) {
      try {
        const event = JSON.parse(msg)
        
        if (event.type === 'BOARD_CREATED') {
          if (!boards.value.find(b => b.id === event.board.id)) {
            boards.value.push(event.board)
          }
          return;
        }
        
        if (!activeBoard.value) return
        
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
          const col = activeBoard.value.columns.find((c: any) => c.id === event.task.column_id)
          if (col) {
            const idx = col.tasks.findIndex((t: any) => t.id === event.task.id)
            if (idx !== -1) {
              // Preserve existing enrichment as backend update doesn't return them
              event.task.labels = event.task.labels || col.tasks[idx].labels || []
              event.task.assignees = event.task.assignees || col.tasks[idx].assignees || []
              event.task.validations = event.task.validations || col.tasks[idx].validations || []
              col.tasks.splice(idx, 1, event.task)
              // Update in taskStack if it is currently open
              for (let i = 0; i < taskStack.value.length; i++) {
                if (taskStack.value[i].id === event.task.id) {
                  Object.assign(taskStack.value[i], event.task)
                }
              }
            }
          }
        } else if (event.type === 'TASK_DELETED') {
          const col = activeBoard.value.columns.find((c: any) => c.id === event.column_id)
          if (col) {
            const idx = col.tasks.findIndex((t: any) => t.id === event.task_id)
            if (idx !== -1) col.tasks.splice(idx, 1)
          }
        } else if (event.type === 'BOARD_UPDATED') {
          const idx = boards.value.findIndex(b => b.id === event.board.id)
          if (idx !== -1) boards.value[idx] = event.board
          if (activeBoard.value?.id === event.board.id) activeBoard.value.name = event.board.name
        } else if (event.type === 'BOARD_DELETED') {
          const idx = boards.value.findIndex(b => b.id === event.board_id)
          if (idx !== -1) boards.value.splice(idx, 1)
          if (activeBoard.value?.id === event.board_id) {
            // Can't automatically call selectBoard(boards.value[0]) easily here without exposing it.
            // Component should watch for activeBoardId being deleted and update.
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
           // Update in taskStack
           for (let i = 0; i < taskStack.value.length; i++) {
              if (taskStack.value[i].id === event.task.id) {
                 Object.assign(taskStack.value[i], event.task)
              }
           }
        }
      } catch (e) {
        console.error("Erreur WS:", e)
      }
    }
  })

  return { wsData }
}
