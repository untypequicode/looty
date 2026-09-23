import { ref, computed } from 'vue'
import type { Ref } from 'vue'
import type { Board } from '../types'

export const useKanbanFilters = (activeBoard: Ref<Board | null>) => {
  const searchQuery = ref('')
  const filterLabels = ref<string[]>([])
  const filterAssignees = ref<string[]>([])
  const hideCompleted = ref(false)

  const toggleFilterLabel = (labelId: string) => {
    if (filterLabels.value.includes(labelId)) {
      filterLabels.value = filterLabels.value.filter(id => id !== labelId)
    } else {
      filterLabels.value.push(labelId)
    }
  }

  const toggleFilterAssignee = (userId: string) => {
    if (filterAssignees.value.includes(userId)) {
      filterAssignees.value = filterAssignees.value.filter(id => id !== userId)
    } else {
      filterAssignees.value.push(userId)
    }
  }

  const filteredBoard = computed(() => {
    if (!activeBoard.value) return null
    const board = activeBoard.value
    
    // Si aucun filtre n'est actif, on retourne le board tel quel
    if (!searchQuery.value && filterLabels.value.length === 0 && filterAssignees.value.length === 0 && !hideCompleted.value) {
      return board
    }
    
    const query = searchQuery.value.toLowerCase()
    
    const newCols = board.columns.map(col => {
      let filteredTasks = col.tasks
      
      if (hideCompleted.value) {
        filteredTasks = filteredTasks.filter(t => !t.is_completed)
      }
      if (query) {
        filteredTasks = filteredTasks.filter(t => t.title.toLowerCase().includes(query))
      }
      if (filterLabels.value.length > 0) {
        filteredTasks = filteredTasks.filter(t => t.labels && t.labels.some(l => filterLabels.value.includes(l.id)))
      }
      if (filterAssignees.value.length > 0) {
        filteredTasks = filteredTasks.filter(t => t.assignees && t.assignees.some(a => filterAssignees.value.includes(a)))
      }
      
      return { ...col, tasks: filteredTasks }
    })
    
    return { ...board, columns: newCols }
  })

  return {
    searchQuery,
    filterLabels,
    filterAssignees,
    hideCompleted,
    toggleFilterLabel,
    toggleFilterAssignee,
    filteredBoard
  }
}
