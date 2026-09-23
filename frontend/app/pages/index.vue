<script setup lang="ts">
import { useWebSocket } from '@vueuse/core'
import { VueDraggableNext } from 'vue-draggable-next'
import gsap from 'gsap'

const { user, isLoading } = useAuth()

const vFocus = {
  mounted: (el: HTMLElement) => el.focus()
}

// Workspaces
const { data: workspacesData, pending: isLoadingWorkspaces, refresh: refreshWorkspaces } = useFetch('/api/workspaces', { lazy: true, server: false })
const isCreatingWorkspace = ref(false)
const newWorkspaceName = ref('')
const showCreateModal = ref(false)
const activeWorkspaceId = ref<string | null>(null)
const currentModule = ref('kanban')

async function createWorkspace() {
  if (!newWorkspaceName.value.trim()) return;
  isCreatingWorkspace.value = true
  try {
    await $fetch('/api/workspaces', { method: 'POST', body: { name: newWorkspaceName.value } })
    newWorkspaceName.value = ''
    showCreateModal.value = false
    await refreshWorkspaces()
  } catch(e) { console.error(e) } finally { isCreatingWorkspace.value = false }
}

watch(workspacesData, (newVal: any) => {
  if (newVal?.workspaces?.length > 0 && !activeWorkspaceId.value) {
    selectWorkspace(newVal.workspaces[0].id)
  }
})

// Kanban State
const boards = ref<any[]>([])
const activeBoard = ref<any>(null)
const currentView = ref<'kanban' | 'timeline'>('kanban')
const wsUrl = computed(() => {
  if (!activeWorkspaceId.value) return null
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${protocol}//${window.location.host}/api/ws?workspace_id=${activeWorkspaceId.value}`
})

// Wiki State
const pages = ref<any[]>([])
const activePage = ref<any>(null)
const isLoadingPages = ref(false)

// Filter State
const searchQuery = ref('')
const filterLabels = ref<string[]>([])
const filterAssignees = ref<string[]>([])
const hideCompleted = ref(false)

function toggleFilterAssignee(userId: string) {
  if (filterAssignees.value.includes(userId)) {
    filterAssignees.value = filterAssignees.value.filter(id => id !== userId)
  } else {
    filterAssignees.value.push(userId)
  }
}

function toggleFilterLabel(labelId: string) {
  if (filterLabels.value.includes(labelId)) {
    filterLabels.value = filterLabels.value.filter(id => id !== labelId)
  } else {
    filterLabels.value.push(labelId)
  }
}

const filteredBoard = computed(() => {
  if (!activeBoard.value) return null
  const newCols = activeBoard.value.columns.map((col: any) => {
    let filteredTasks = col.tasks.filter((t: any) => {
      if (searchQuery.value) {
        const q = searchQuery.value.toLowerCase()
        const titleMatch = t.title.toLowerCase().includes(q)
        const descMatch = t.description && t.description.toLowerCase().includes(q)
        if (!titleMatch && !descMatch) return false
      }
      if (filterLabels.value.length > 0) {
        if (!t.labels || !t.labels.some((l: any) => filterLabels.value.includes(l.id))) return false
      }
      if (filterAssignees.value.length > 0) {
        if (!t.assignees || !t.assignees.some((a: any) => filterAssignees.value.includes(a))) return false
      }
      if (hideCompleted.value && t.is_completed) return false
      return true
    })

    filteredTasks = filteredTasks.filter((t: any) => {
      if (!t.parent_id) return true
      const parentInSameCol = col.tasks.some((p: any) => p.id === t.parent_id)
      return !parentInSameCol
    })

    return { ...col, tasks: filteredTasks }
  })
  
  return { ...activeBoard.value, columns: newCols }
})

// Modals State
const showBoardModal = ref(false)
const newBoardName = ref('')
const showColumnModal = ref(false)
const newColumnName = ref('')
const showTaskModal = ref(false)
const newTaskTitle = ref('')
const targetColumnId = ref<string | null>(null)

// Task Detail / Edit Modal
const showTaskDetailModal = ref(false)
const taskStack = ref<any[]>([])
const editingTask = computed(() => taskStack.value[taskStack.value.length - 1] || null)
const editTaskTitle = ref('')
const editTaskDesc = ref('')
const editTaskStartDate = ref('')
const editTaskDueDate = ref('')
const editTaskPriority = ref(0)

const newSubtaskTitle = ref('')

async function createSubtask() {
  if (!newSubtaskTitle.value.trim() || !editingTask.value) return;
  try {
    const response = await fetch(`/api/boards/tasks/${editingTask.value.id}/subtasks`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ title: newSubtaskTitle.value })
    })
    if (response.ok) {
      newSubtaskTitle.value = '';
      const subtask = await response.json();
      // Optimistic or WS will handle it. But we can push to taskStack.value[last].subtasks if needed.
    }
  } catch (e) { console.error(e) }
}

async function toggleTaskValidation(task: any) {
  try {
    const wasCompleted = task.is_completed;
    const response = await fetch(`/api/boards/tasks/${task.id}/toggle_validation`, { method: 'PUT' })
    if (response.ok) {
      const updated = await response.json();
      Object.assign(task, updated);
      
      // Automation logic
      if (activeBoard.value && activeBoard.value.automations) {
        const auto = activeBoard.value.automations;
        
        // Skip subtasks if apply_to_subtasks is false
        if (task.parent_id && !auto.apply_to_subtasks) return;
        
        if (!wasCompleted && updated.is_completed && auto.on_complete_move_to) {
          // It became completed, move to target column
          if (task.column_id !== auto.on_complete_move_to) {
            await $fetch(`/api/boards/tasks/${task.id}/move`, { 
              method: 'PUT', 
              body: { 
                column_id: auto.on_complete_move_to, 
                position: 65536.0,
                previous_column_id: task.column_id
              } 
            });
          }
        } else if (wasCompleted && !updated.is_completed && task.previous_column_id) {
          // It was un-completed, move back to previous column
          if (task.column_id !== task.previous_column_id) {
            await $fetch(`/api/boards/tasks/${task.id}/move`, { 
              method: 'PUT', 
              body: { 
                column_id: task.previous_column_id, 
                position: 65536.0,
                clear_previous_column_id: true
              } 
            });
          }
        }
      }
    }
  } catch (e) {}
}

// Board Settings Modal
const showBoardSettingsModal = ref(false)
const boardSettings = ref({
  on_complete_move_to: '',
  apply_to_subtasks: false,
  auto_validate_on_move: false
})

function openBoardSettingsModal() {
  if (!activeBoard.value) return;
  boardSettings.value = {
    on_complete_move_to: activeBoard.value.automations?.on_complete_move_to || '',
    apply_to_subtasks: activeBoard.value.automations?.apply_to_subtasks || false,
    auto_validate_on_move: activeBoard.value.automations?.auto_validate_on_move || false
  };
  showBoardSettingsModal.value = true;
}

async function saveBoardSettings() {
  if (!activeBoard.value) return;
  try {
    const res = await $fetch(`/api/boards/${activeBoard.value.id}`, {
      method: 'PUT',
      body: { automations: boardSettings.value }
    });
    activeBoard.value.automations = boardSettings.value;
    showBoardSettingsModal.value = false;
  } catch (e) { console.error(e) }
}

// Rename Modal
const showRenameModal = ref(false)
const renameType = ref<'workspace'|'board'|'column'|null>(null)
const renameId = ref<string | null>(null)
const renameName = ref('')

// Settings Modal & Role
const activeWorkspaceRole = computed(() => {
  const ws = workspacesData.value?.workspaces?.find((w: any) => w.id === activeWorkspaceId.value)
  return ws ? ws.role : null
})

const showSettingsModal = ref(false)
const workspaceMembers = ref<any[]>([])

async function openSettingsModal() {
  if (!activeWorkspaceId.value) return
  try {
    const res = await $fetch<any>(`/api/workspaces/${activeWorkspaceId.value}/members`)
    workspaceMembers.value = res.members || []
    showSettingsModal.value = true
  } catch(e) { console.error(e) }
}

async function changeMemberRole(memberId: string, newRole: string) {
  try {
    await $fetch(`/api/workspaces/${activeWorkspaceId.value}/members/${memberId}`, { method: 'PUT', body: { role: newRole } })
    await openSettingsModal()
  } catch(e) { alert("Erreur lors de la modification du rôle") }
}

async function kickMember(memberId: string) {
  if (!confirm("Voulez-vous vraiment exclure ce membre ?")) return
  try {
    await $fetch(`/api/workspaces/${activeWorkspaceId.value}/members/${memberId}`, { method: 'DELETE' })
    await openSettingsModal()
  } catch(e) { alert("Erreur lors de l'exclusion") }
}

// Load boards and members when workspace changes
watch(activeWorkspaceId, async (wsId) => {
  if (wsId) {
    try {
      const res = await $fetch<any>(`/api/workspaces/${wsId}/members`)
      workspaceMembers.value = res.members || []
    } catch(e) {}
    
    boards.value = await $fetch(`/api/workspaces/${wsId}/boards`)
    if (boards.value.length > 0) selectBoard(boards.value[0])
    else activeBoard.value = null

    isLoadingPages.value = true
    try {
      pages.value = await $fetch(`/api/workspaces/${wsId}/pages`)
    } catch(e) {}
    isLoadingPages.value = false
  } else {
    boards.value = []
    activeBoard.value = null
    workspaceMembers.value = []
    pages.value = []
    activePage.value = null
  }
}, { immediate: true })

function getRootTasks(column: any) {
  if (!column.tasks) return []
  return column.tasks.filter((t: any) => {
    if (!t.parent_id) return true
    // Also root if parent is in another column
    const parentInSameCol = column.tasks.some((p: any) => p.id === t.parent_id)
    return !parentInSameCol
  })
}

function getSubtasks(parentTask: any) {
  if (!activeBoard.value) return []
  const subtasks = []
  for (const col of activeBoard.value.columns) {
    if (col.tasks) {
      subtasks.push(...col.tasks.filter((t: any) => t.parent_id === parentTask.id))
    }
  }
  return subtasks
}

function selectWorkspace(workspaceId: string) { activeWorkspaceId.value = workspaceId }

async function selectBoard(board: any) {
  const fullBoard = await $fetch(`/api/boards/${board.id}`)
  activeBoard.value = fullBoard
  activePage.value = null
}

async function createPage() {
  if (!activeWorkspaceId.value) return
  try {
    const page = await $fetch(`/api/workspaces/${activeWorkspaceId.value}/pages`, {
      method: 'POST',
      body: { title: 'Nouvelle page', parent_id: null, content: '' }
    })
    pages.value.push(page)
    selectPage(page)
  } catch(e) {
    console.error('Erreur creation page:', e)
  }
}

function selectPage(page: any) {
  currentModule.value = 'wiki'
  activeBoard.value = null
  activePage.value = page
}

async function deletePage(id: string) {
  if (!confirm('Supprimer cette page ?')) return
  try {
    await $fetch(`/api/workspaces/${activeWorkspaceId.value}/pages/${id}`, { method: 'DELETE' })
    pages.value = pages.value.filter(p => p.id !== id)
    if (activePage.value?.id === id) activePage.value = null
  } catch(e) {
    console.error('Erreur suppression page:', e)
  }
}

function updatePageTitleInList(pageId: string, newTitle: string) {
  const p = pages.value.find(p => p.id === pageId)
  if (p) p.title = newTitle
}

async function createBoard() {
  if (!newBoardName.value.trim() || !activeWorkspaceId.value) return
  const newBoard = await $fetch(`/api/workspaces/${activeWorkspaceId.value}/boards`, { method: 'POST', body: { name: newBoardName.value } })
  // On ne push plus manuellement : le WebSocket (BOARD_CREATED) s'en charge pour tout le monde, y compris nous !
  selectBoard(newBoard) // Par contre, on le sélectionne directement
  showBoardModal.value = false
  newBoardName.value = ''
}

async function createColumn() {
  if (!newColumnName.value.trim() || !activeBoard.value) return
  await $fetch(`/api/boards/${activeBoard.value.id}/columns`, { method: 'POST', body: { name: newColumnName.value } })
  // Le WebSocket (COLUMN_CREATED) l'ajoutera dans l'interface
  showColumnModal.value = false
  newColumnName.value = ''
}

function openTaskModal(colId: string) {
  taskStack.value = []
  targetColumnId.value = colId
  editTaskTitle.value = ''
  editTaskDesc.value = ''
  editTaskStartDate.value = ''
  editTaskDueDate.value = ''
  editTaskPriority.value = 0
  createTaskLabels.value = []
  createTaskAssignees.value = []
  showTaskDetailModal.value = true
}

function closeTaskModal() {
  if (editingTask.value) {
    updateTask()
  }
  taskStack.value.pop()
  if (taskStack.value.length === 0) {
    showTaskDetailModal.value = false
  } else {
    // Sync inputs for the previous task in stack
    const task = editingTask.value;
    if (task) {
      editTaskTitle.value = task.title
      editTaskDesc.value = task.description || ''
      editTaskStartDate.value = task.start_date ? task.start_date.split('T')[0] : ''
      editTaskDueDate.value = task.due_date ? task.due_date.split('T')[0] : ''
    }
  }
}

function openTaskDetail(task: any) {
  taskStack.value.push(task)
  editTaskTitle.value = task.title
  editTaskDesc.value = task.description || ''
  editTaskStartDate.value = task.start_date ? task.start_date.split('T')[0] : ''
  editTaskDueDate.value = task.due_date ? task.due_date.split('T')[0] : ''
  editTaskPriority.value = task.priority || 0
  if (!task.labels) task.labels = []
  if (!task.assignees) task.assignees = []
  showTaskDetailModal.value = true
}

// Labels and Assignees
const createTaskLabels = ref<any[]>([])
const createTaskAssignees = ref<string[]>([])
const labelSearch = ref('')
const assigneeSearch = ref('')
const showCreateLabel = ref(false)
const newLabelName = ref('')
const newLabelColor = ref('#5e6ad2')
const editingLabelId = ref<string | null>(null)

const filteredLabels = computed(() => {
  if (!activeBoard.value?.labels) return []
  return activeBoard.value.labels.filter((l: any) => l.name.toLowerCase().includes(labelSearch.value.toLowerCase()))
})

const filteredAssignees = computed(() => {
  return workspaceMembers.value.filter((m: any) => m.name.toLowerCase().includes(assigneeSearch.value.toLowerCase()))
})

function isLabelSelected(labelId: string) {
  if (editingTask.value) {
    return editingTask.value.labels.some((l: any) => l.id === labelId)
  }
  return createTaskLabels.value.some((l: any) => l.id === labelId)
}

function isAssigneeSelected(userId: string) {
  if (editingTask.value) {
    return editingTask.value.assignees.includes(userId)
  }
  return createTaskAssignees.value.includes(userId)
}

async function toggleLabel(lbl: any) {
  if (isLabelSelected(lbl.id)) {
    if (editingTask.value) {
      await $fetch(`/api/boards/tasks/${editingTask.value.id}/labels/${lbl.id}`, { method: 'DELETE' })
      editingTask.value.labels = editingTask.value.labels.filter((l: any) => l.id !== lbl.id)
    } else {
      createTaskLabels.value = createTaskLabels.value.filter((l: any) => l.id !== lbl.id)
    }
  } else {
    if (editingTask.value) {
      await $fetch(`/api/boards/tasks/${editingTask.value.id}/labels/${lbl.id}`, { method: 'POST' })
      editingTask.value.labels.push(lbl)
    } else {
      createTaskLabels.value.push(lbl)
    }
  }
}

async function toggleAssignee(userId: string) {
  if (isAssigneeSelected(userId)) {
    if (editingTask.value) {
      await $fetch(`/api/boards/tasks/${editingTask.value.id}/assignees/${userId}`, { method: 'DELETE' })
      editingTask.value.assignees = editingTask.value.assignees.filter((u: any) => u !== userId)
    } else {
      createTaskAssignees.value = createTaskAssignees.value.filter((u: any) => u !== userId)
    }
  } else {
    if (editingTask.value) {
      await $fetch(`/api/boards/tasks/${editingTask.value.id}/assignees/${userId}`, { method: 'POST' })
      editingTask.value.assignees.push(userId)
    } else {
      createTaskAssignees.value.push(userId)
    }
  }
}

async function deleteBoardLabel(labelId: string) {
  if (!confirm("Voulez-vous vraiment supprimer définitivement ce label ?")) return
  await $fetch(`/api/boards/labels/${labelId}`, { method: 'DELETE' })
  if (activeBoard.value) {
    activeBoard.value.labels = activeBoard.value.labels.filter((l: any) => l.id !== labelId)
  }
  // Remove from editing task or create task if it was selected
  if (editingTask.value) {
    editingTask.value.labels = editingTask.value.labels.filter((l: any) => l.id !== labelId)
  }
  createTaskLabels.value = createTaskLabels.value.filter((l: any) => l.id !== labelId)
}

async function createLabel() {
  if (!newLabelName.value || !activeBoard.value) return
  
  if (editingLabelId.value) {
    const label = await $fetch(`/api/boards/labels/${editingLabelId.value}`, {
      method: 'PUT',
      body: { name: newLabelName.value, color: newLabelColor.value }
    })
    
    // Update local references
    if (activeBoard.value) {
      const idx = activeBoard.value.labels.findIndex((l: any) => l.id === label.id)
      if (idx !== -1) activeBoard.value.labels.splice(idx, 1, label)
      
      // Update in existing tasks
      for (const col of activeBoard.value.columns) {
        for (const task of col.tasks) {
          if (task.labels) {
            const lIdx = task.labels.findIndex((l: any) => l.id === label.id)
            if (lIdx !== -1) task.labels.splice(lIdx, 1, label)
          }
        }
      }
    }
    
    // Update local editing arrays
    if (editingTask.value && editingTask.value.labels) {
      const idx = editingTask.value.labels.findIndex((l: any) => l.id === label.id)
      if (idx !== -1) editingTask.value.labels.splice(idx, 1, label)
    }
    const cIdx = createTaskLabels.value.findIndex((l: any) => l.id === label.id)
    if (cIdx !== -1) createTaskLabels.value.splice(cIdx, 1, label)
    
  } else {
    const label = await $fetch(`/api/boards/${activeBoard.value.id}/labels`, {
      method: 'POST',
      body: { name: newLabelName.value, color: newLabelColor.value }
    })
    if (activeBoard.value) {
       if (!activeBoard.value.labels) activeBoard.value.labels = []
       activeBoard.value.labels.push(label)
    }
  }
  
  showCreateLabel.value = false
  newLabelName.value = ''
  editingLabelId.value = null
}

function startEditLabel(lbl: any) {
  editingLabelId.value = lbl.id
  newLabelName.value = lbl.name
  newLabelColor.value = lbl.color
  showCreateLabel.value = true
}

function toggleCreateLabel() {
  if (showCreateLabel.value && !editingLabelId.value) {
    showCreateLabel.value = false
  } else {
    editingLabelId.value = null
    newLabelName.value = ''
    newLabelColor.value = '#5e6ad2'
    showCreateLabel.value = true
  }
}

// Old explicit add/remove methods replaced by toggleLabel and toggleAssignee

function getMemberName(userId: string) {
  const m = workspaceMembers.value.find(x => x.user_id === userId)
  return m ? m.name : 'Inconnu'
}

function getMemberInitials(userId: string) {
  return getMemberName(userId).charAt(0).toUpperCase()
}

async function updateTask() {
  if (!editTaskTitle.value.trim()) return
  
  if (editingTask.value) {
    const body: any = { 
      title: editTaskTitle.value, 
      description: editTaskDesc.value,
      priority: editTaskPriority.value,
      is_per_person: editingTask.value.is_per_person
    }
    body.start_date = editTaskStartDate.value ? new Date(editTaskStartDate.value).toISOString() : null
    body.due_date = editTaskDueDate.value ? new Date(editTaskDueDate.value).toISOString() : null

    await $fetch(`/api/boards/tasks/${editingTask.value.id}`, { method: 'PUT', body })
  } else if (targetColumnId.value) {
    try {
      await $fetch(`/api/boards/columns/${targetColumnId.value}/tasks`, {
        method: 'POST',
        body: { 
          title: editTaskTitle.value, 
          description: editTaskDesc.value,
          priority: editTaskPriority.value,
          start_date: editTaskStartDate.value ? new Date(editTaskStartDate.value).toISOString() : null,
          due_date: editTaskDueDate.value ? new Date(editTaskDueDate.value).toISOString() : null,
          labels: createTaskLabels.value.map(l => l.id),
          assignees: createTaskAssignees.value
        }
      })
      if (activeBoard.value) {
        await selectBoard(activeBoard.value)
      }
    } catch (e) {
      console.error("Erreur lors de la création de la tâche:", e)
      alert("Impossible de créer la tâche, vérifie les informations saisies.")
    }
  }
  showTaskDetailModal.value = false
}

async function saveTaskSettings() {
  if (!editingTask.value) return
  const body: any = { 
    title: editingTask.value.title, 
    description: editingTask.value.description,
    is_per_person: editingTask.value.is_per_person,
    start_date: editingTask.value.start_date,
    due_date: editingTask.value.due_date
  }
  await $fetch(`/api/boards/tasks/${editingTask.value.id}`, { method: 'PUT', body })
}

async function deleteTask() {
  if (!editingTask.value) return
  if (!confirm("Voulez-vous vraiment supprimer cette tâche ?")) return
  await $fetch(`/api/boards/tasks/${editingTask.value.id}`, { method: 'DELETE' })
  showTaskDetailModal.value = false
}

function copyInviteLink() {
  if (!activeWorkspaceId.value) return
  const url = `${window.location.origin}/invite/${activeWorkspaceId.value}`
  navigator.clipboard.writeText(url)
  alert("Lien d'invitation copié : " + url)
}

function openRenameModal(type: 'workspace'|'board'|'column', id: string, currentName: string) {
  renameType.value = type
  renameId.value = id
  renameName.value = currentName
  showRenameModal.value = true
}

async function submitRename() {
  if (!renameName.value.trim() || !renameId.value || !renameType.value) return
  
  const id = renameId.value
  const name = renameName.value
  
  if (renameType.value === 'workspace') {
    await $fetch(`/api/workspaces/${id}`, { method: 'PUT', body: { name } })
    await refreshWorkspaces()
  } else if (renameType.value === 'board') {
    await $fetch(`/api/boards/${id}`, { method: 'PUT', body: { name } })
  } else if (renameType.value === 'column') {
    await $fetch(`/api/boards/columns/${id}`, { method: 'PUT', body: { name } })
  }
  
  showRenameModal.value = false
}

async function deleteWorkspace(id: string) {
  if (!confirm("Voulez-vous vraiment supprimer cet espace de travail ?")) return
  await $fetch(`/api/workspaces/${id}`, { method: 'DELETE' })
  if (activeWorkspaceId.value === id) activeWorkspaceId.value = null
  await refreshWorkspaces()
}

async function deleteBoard(id: string) {
  if (!confirm("Voulez-vous vraiment supprimer ce tableau ?")) return
  await $fetch(`/api/boards/${id}`, { method: 'DELETE' })
}

async function deleteColumn(id: string) {
  if (!confirm("Voulez-vous vraiment supprimer cette colonne et toutes ses tâches ?")) return
  await $fetch(`/api/boards/columns/${id}`, { method: 'DELETE' })
}

// Drag and drop event
async function onTaskMove(evt: any, toColumnId: string) {
  if (evt.added || evt.moved) {
    const task = evt.added ? evt.added.element : evt.moved.element
    const newIndex = evt.added ? evt.added.newIndex : evt.moved.newIndex
    
    const col = filteredBoard.value?.columns.find((c: any) => c.id === toColumnId)
    if (!col) return
    let newPosition = 65536.0
    if (col.tasks.length === 1) {
      newPosition = 65536.0
    } else if (newIndex === 0) {
      newPosition = col.tasks[1].position / 2.0
    } else if (newIndex === col.tasks.length - 1) {
      newPosition = col.tasks[newIndex - 1].position + 65536.0
    } else {
      newPosition = (col.tasks[newIndex - 1].position + col.tasks[newIndex + 1].position) / 2.0
    }
    
    task.position = newPosition
    task.column_id = toColumnId
    
    // Optimistic UI updates
    let shouldToggle = false
    if (evt.added && boardSettings.value?.auto_validate_on_move && boardSettings.value?.on_complete_move_to) {
      if (toColumnId === boardSettings.value.on_complete_move_to && !task.is_completed) {
        task.is_completed = true
        shouldToggle = true
      } else if (toColumnId !== boardSettings.value.on_complete_move_to && task.is_completed) {
        task.is_completed = false
        shouldToggle = true
      }
    }
    
    await $fetch(`/api/boards/tasks/${task.id}/move`, { method: 'PUT', body: { column_id: toColumnId, position: newPosition } })
    
    if (shouldToggle) {
      await $fetch(`/api/boards/tasks/${task.id}/toggle_validation`, { method: 'PUT' })
    }
  }
}

async function onColumnMove(evt: any) {
  if (evt.moved && activeBoard.value) {
    const column = evt.moved.element
    const newIndex = evt.moved.newIndex
    
    const cols = filteredBoard.value?.columns
    if (!cols) return
    let newPosition = 65536.0
    if (cols.length === 1) {
      newPosition = 65536.0
    } else if (newIndex === 0) {
      newPosition = cols[1].position / 2.0
    } else if (newIndex === cols.length - 1) {
      newPosition = cols[cols.length - 1].position + 65536.0
    } else {
      newPosition = (cols[newIndex - 1].position + cols[newIndex + 1].position) / 2.0
    }
    
    column.position = newPosition
    await $fetch(`/api/boards/columns/${column.id}/move`, { method: 'PUT', body: { position: newPosition } })
  }
}

// WebSockets for Real-time
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
          if (boards.value.length > 0) selectBoard(boards.value[0])
          else activeBoard.value = null
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

function handleKeydown(e: KeyboardEvent) {
  const target = e.target as HTMLElement
  const isInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable
  
  if (e.key === 'Escape') {
    showCreateModal.value = false
    showBoardModal.value = false
    showColumnModal.value = false
    showTaskDetailModal.value = false
    showRenameModal.value = false
    showSettingsModal.value = false
    showBoardSettingsModal.value = false
  } else if (e.key.toLowerCase() === 'c' && !isInput && activeBoard.value && activeBoard.value.columns.length > 0) {
    e.preventDefault()
    openTaskModal(activeBoard.value.columns[0].id)
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleKeydown)
})

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeydown)
})

watchEffect(() => {
  if (!isLoading.value && !user.value?.authenticated) navigateTo('/login')
})

const priorityIcons: Record<number, {label: string, color: string, icon: string}> = {
  0: { label: 'Sans priorité', color: '#6b7280', icon: '○' },
  1: { label: 'Urgente', color: '#ef4444', icon: '🔴' },
  2: { label: 'Haute', color: '#f97316', icon: '🟠' },
  3: { label: 'Moyenne', color: '#eab308', icon: '🟡' },
  4: { label: 'Basse', color: '#6b7280', icon: '🔵' },
}

function getPriority(p: number) { return priorityIcons[p] || priorityIcons[0] }

// GSAP animations for modals
function animateSlideoverOpen() {
  nextTick(() => {
    const panel = document.querySelector('.slideover-panel')
    if (panel) {
      gsap.fromTo(panel, { x: '100%', opacity: 0.5 }, { x: '0%', opacity: 1, duration: 0.35, ease: 'power3.out' })
    }
  })
}

function animateModalOpen(selector: string) {
  nextTick(() => {
    const modal = document.querySelector(selector + ' .modal')
    if (modal) {
      gsap.fromTo(modal, { scale: 0.95, opacity: 0, y: 10 }, { scale: 1, opacity: 1, y: 0, duration: 0.25, ease: 'power2.out' })
    }
  })
}

watch(showTaskDetailModal, (v) => { if (v) animateSlideoverOpen() })
watch(showCreateModal, (v) => { if (v) animateModalOpen('.modal-overlay') })
watch(showBoardModal, (v) => { if (v) animateModalOpen('.modal-overlay') })
watch(showColumnModal, (v) => { if (v) animateModalOpen('.modal-overlay') })
</script>

<template>
  <div v-if="isLoading" class="loading-screen">
    <div class="spinner"></div><p>Chargement de Looty...</p>
  </div>
  
  <div v-else-if="user?.authenticated" class="app-layout">
    <!-- Sidebar -->
    <aside class="sidebar">
      <div class="sidebar-header"><div class="logo">Looty</div></div>
      
      <div class="sidebar-content">
        <div class="section-title">
          <span>Espaces de Travail</span>
          <div style="display: flex; gap: 0.5rem;">
            <button v-if="activeWorkspaceId" @click="copyInviteLink" class="icon-btn" title="Inviter des membres">🔗</button>
            <button @click="showCreateModal = true" class="icon-btn" title="Créer un espace">+</button>
          </div>
        </div>
        <ul class="workspace-list">
          <li v-if="isLoadingWorkspaces" class="loading-item">Chargement...</li>
          <li v-else-if="!workspacesData?.workspaces?.length" class="empty-item">Aucun espace</li>
          <li v-for="ws in workspacesData?.workspaces" :key="ws.id"
            :class="['workspace-item', { active: activeWorkspaceId === ws.id }]" @click="selectWorkspace(ws.id)">
            <div style="display: flex; align-items: center; gap: 10px; flex: 1">
              <div class="ws-avatar">{{ ws.name.charAt(0).toUpperCase() }}</div>
              <span class="ws-name">{{ ws.name }}</span>
            </div>
            <div class="item-actions" v-if="activeWorkspaceId === ws.id">
              <button @click.stop="openSettingsModal()" title="Paramètres (Membres)">⚙️</button>
              <button v-if="ws.role === 'owner' || ws.role === 'admin'" @click.stop="openRenameModal('workspace', ws.id, ws.name)" title="Renommer">✏️</button>
              <button v-if="ws.role === 'owner'" @click.stop="deleteWorkspace(ws.id)" title="Supprimer">🗑️</button>
            </div>
          </li>
        </ul>

        <div v-if="activeWorkspaceId" class="section-title" style="margin-top:2rem;">
          <span>Tableaux</span>
          <button v-if="activeWorkspaceRole === 'owner' || activeWorkspaceRole === 'admin'" @click="showBoardModal = true" class="icon-btn" title="Créer un tableau">+</button>
        </div>
        <ul v-if="activeWorkspaceId" class="workspace-list">
          <li v-if="boards.length === 0" class="empty-item">Aucun tableau</li>
          <li v-for="b in boards" :key="b.id"
            :class="['workspace-item', { active: activeBoard?.id === b.id && currentModule === 'kanban' }]" @click="selectBoard(b); currentModule = 'kanban'">
            <span class="ws-name" style="flex: 1"># {{ b.name }}</span>
            <div class="item-actions" v-if="activeBoard?.id === b.id && (activeWorkspaceRole === 'owner' || activeWorkspaceRole === 'admin')">
              <button @click.stop="openRenameModal('board', b.id, b.name)" title="Renommer">✏️</button>
              <button @click.stop="deleteBoard(b.id)" title="Supprimer">🗑️</button>
            </div>
          </li>
        </ul>

        <div v-if="activeWorkspaceId" class="section-title" style="margin-top:2rem;">
          <span>Wiki (Pages)</span>
          <button @click="createPage" class="icon-btn" title="Créer une page">+</button>
        </div>
        <ul v-if="activeWorkspaceId" class="workspace-list">
          <li v-if="isLoadingPages" class="loading-item">Chargement...</li>
          <li v-else-if="pages.length === 0" class="empty-item">Aucune page</li>
          <li v-for="page in pages" :key="page.id"
            :class="['workspace-item', { active: activePage?.id === page.id && currentModule === 'wiki' }]" 
            @click="selectPage(page)">
            <span class="ws-name" style="flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">📄 {{ page.title || 'Nouvelle page' }}</span>
            <div class="item-actions" v-if="activePage?.id === page.id">
              <button @click.stop="deletePage(page.id)" title="Supprimer">🗑️</button>
            </div>
          </li>
        </ul>
      </div>
      
      <div class="sidebar-footer">
        <div class="user-profile">
          <div class="user-avatar">{{ user.user.name.charAt(0).toUpperCase() }}</div>
          <div class="user-info">
            <p class="user-name">{{ user.user.name }}</p>
            <p class="user-email">{{ user.user.email }}</p>
          </div>
        </div>
        <a href="/api/auth/logout" class="logout-btn">Déconnexion</a>
      </div>
    </aside>

    <!-- Main Content -->
    <main class="main-content">
      <header class="main-header">
        <div v-if="activeBoard" style="display: flex; justify-content: space-between; align-items: center; width: 100%;">
          <div style="display: flex; align-items: center; gap: 12px;">
            <h1 style="margin: 0;">{{ activeBoard.name }}</h1>
            <button class="icon-btn" @click="openBoardSettingsModal" title="Paramètres du tableau" style="font-size: 1.2rem;">⚙️</button>
          </div>
          <div class="view-toggle">
            <button :class="{ active: currentView === 'kanban' }" @click="currentView = 'kanban'">Kanban</button>
            <button :class="{ active: currentView === 'timeline' }" @click="currentView = 'timeline'">Timeline</button>
          </div>
        </div>
        <div v-else-if="activeWorkspaceId && currentModule === 'wiki'" style="width: 100%; display: flex; align-items: center; gap: 8px;">
          <div style="color: #8a8f98; font-size: 0.9rem;">
            Wiki / {{ activePage ? (activePage.title || 'Nouvelle page') : 'Sélectionnez une page' }}
          </div>
          <div style="margin-left: auto;">
            <button v-if="activePage" class="secondary-btn" style="padding: 4px 12px; font-size: 0.85rem;">Édition terminée</button>
          </div>
        </div>
        <div v-else-if="activeWorkspaceId">
          <h1>Sélectionnez un tableau</h1>
          <p class="subtitle">Créez ou sélectionnez un tableau Kanban dans la barre latérale.</p>
        </div>
        <div v-else>
          <h1>Bienvenue sur Looty</h1>
          <p class="subtitle">Sélectionnez ou créez un espace de travail pour commencer.</p>
        </div>
      </header>

      <div class="content-body" v-if="activeBoard">
        <div class="filter-bar" style="display: flex; gap: 16px; padding: 12px 24px; background: #131418; border-bottom: 1px solid #27282b; align-items: center; flex-wrap: wrap;">
          <input v-model="searchQuery" type="text" class="modern-input" placeholder="Rechercher une tâche..." style="max-width: 250px; margin: 0; padding: 6px 12px; font-size: 0.85rem;">
          
          <div style="display: flex; gap: 8px; align-items: center;" v-if="workspaceMembers && workspaceMembers.length > 0">
            <span style="font-size: 0.8rem; color: #8a8f98;">Membres :</span>
            <div style="display: flex; gap: 4px;">
              <span v-for="m in workspaceMembers" :key="m.user_id"
                    class="assignee-avatar small clickable-chip"
                    :class="{ 'validated-avatar': filterAssignees.includes(m.user_id) }"
                    :style="{ opacity: filterAssignees.includes(m.user_id) ? 1 : 0.5 }"
                    @click="toggleFilterAssignee(m.user_id)"
                    :title="m.name">
                {{ getMemberInitials(m.user_id) }}
              </span>
            </div>
          </div>

          <div style="display: flex; gap: 8px; align-items: center;" v-if="activeBoard.labels && activeBoard.labels.length > 0">
            <span style="font-size: 0.8rem; color: #8a8f98;">Labels :</span>
            <div style="display: flex; gap: 4px; flex-wrap: wrap;">
              <span v-for="l in activeBoard.labels" :key="l.id"
                    class="task-label-chip clickable-chip"
                    :style="{ backgroundColor: filterLabels.includes(l.id) ? l.color : '#2f3136', border: filterLabels.includes(l.id) ? 'none' : '1px solid #4a4d55', color: filterLabels.includes(l.id) ? '#fff' : '#9ca3af' }"
                    @click="toggleFilterLabel(l.id)">
                {{ l.name }}
              </span>
            </div>
          </div>

          <label style="display: flex; align-items: center; font-size: 0.85rem; color: #a1a1aa; cursor: pointer; margin-left: auto;">
            <input type="checkbox" v-model="hideCompleted" style="margin-right: 6px;">
            Masquer terminées
          </label>
        </div>

        <div class="kanban-board" v-if="currentView === 'kanban'">
          <VueDraggableNext
            v-model="filteredBoard.columns"
            group="columns"
            @change="onColumnMove($event)"
            class="columns-container"
            ghost-class="ghost-column"
            handle=".column-header"
            :disabled="activeWorkspaceRole === 'member'"
          >
            <div v-for="col in filteredBoard.columns" :key="col.id" class="kanban-column">
              <div class="column-header">
                <h3 style="flex: 1; cursor: grab" @dblclick="activeWorkspaceRole !== 'member' ? openRenameModal('column', col.id, col.name) : null" :title="activeWorkspaceRole !== 'member' ? 'Double-clic pour renommer' : ''">{{ col.name }}</h3>
                <div style="display: flex; gap: 6px; align-items: center;">
                  <span class="task-count">{{ col.tasks.length }}</span>
                  <button v-if="activeWorkspaceRole === 'owner' || activeWorkspaceRole === 'admin'" class="icon-btn" style="font-size: 0.75rem" @click="deleteColumn(col.id)" title="Supprimer la colonne">🗑️</button>
                </div>
              </div>
              
              <VueDraggableNext
                v-model="col.tasks"
                group="tasks"
                @change="onTaskMove($event, col.id)"
                class="task-list"
                ghost-class="ghost-task"
              >
                <div v-for="task in col.tasks" :key="task.id" class="task-card" @click="openTaskDetail(task)">
                  <div class="task-card-top">
                    <span class="priority-dot" :style="{ color: getPriority(task.priority).color }" :title="getPriority(task.priority).label">{{ getPriority(task.priority).icon }}</span>
                    <div class="task-labels" v-if="task.labels && task.labels.length > 0">
                      <span v-for="lbl in task.labels" :key="lbl.id" class="task-label-dot" :style="{ backgroundColor: lbl.color }" :title="lbl.name"></span>
                    </div>
                  </div>
                  <h4>
                    <span v-if="task.is_completed" class="status-icon completed">✓</span>
                    <span :class="{ 'task-title-done': task.is_completed }">{{ task.title }}</span>
                  </h4>
                  <div class="task-card-footer">
                    <div v-if="task.due_date" class="task-due-date" :class="{ 'overdue': new Date(task.due_date) < new Date() }">
                      {{ new Date(task.due_date).toLocaleDateString('fr-FR', { day: 'numeric', month: 'short' }) }}
                    </div>
                    <div v-else></div>
                    <div class="task-assignees" v-if="task.assignees && task.assignees.length > 0">
                      <span v-for="userId in task.assignees.slice(0, 3)" :key="userId" class="assignee-avatar" :class="{'validated-avatar': task.is_per_person && task.validations?.includes(userId)}" :title="getMemberName(userId)">{{ getMemberInitials(userId) }}</span>
                      <span v-if="task.assignees.length > 3" class="assignee-avatar more">+{{ task.assignees.length - 3 }}</span>
                    </div>
                  </div>
                  <!-- Nested Subtasks -->
                  <SubtaskTree v-if="col.tasks.some((t: any) => t.parent_id === task.id)" :parentTask="task" :tasks="col.tasks" :openTaskDetail="openTaskDetail" :toggleTaskValidation="toggleTaskValidation" />
                </div>
              </VueDraggableNext>
              
              <button class="add-task-btn" @click="openTaskModal(col.id)">+ Nouvelle tâche</button>
            </div>
          </VueDraggableNext>
          
          <div v-if="activeWorkspaceRole === 'owner' || activeWorkspaceRole === 'admin'" class="kanban-column add-column" @click="showColumnModal = true">
            + Ajouter une colonne
          </div>
        </div>
        
        <TimelineView v-else-if="currentView === 'timeline'" :board="filteredBoard" :openTaskDetail="openTaskDetail" />
      </div>

      <div v-if="activeWorkspaceId && currentModule === 'wiki'" style="flex: 1; overflow-y: auto;">
        <WorkspacePages 
          v-if="activePage" 
          :workspace-id="activeWorkspaceId" 
          :page="activePage" 
          :current-user="user?.user"
          @update-title="updatePageTitleInList(activePage.id, $event)"
        />
        <div v-else style="display: flex; height: 100%; align-items: center; justify-content: center; color: #8a8f98;">
          Sélectionnez une page dans le menu de gauche ou créez-en une nouvelle.
        </div>
      </div>
    </main>

    <!-- Modale Paramètres du Tableau (Automatisations) -->
    <div v-if="showBoardSettingsModal" class="modal-overlay" @click.self="showBoardSettingsModal = false">
      <div class="modal">
        <h2>Paramètres du Tableau</h2>
        <form @submit.prevent="saveBoardSettings">
          
          <div style="margin-top: 1.5rem; background: #131418; padding: 1rem; border-radius: 8px; border: 1px solid #27282b;">
            <h3 style="margin-top: 0; font-size: 1rem; color: #eee; margin-bottom: 1rem;">Automatisations</h3>
            
            <label style="display: flex; align-items: center; gap: 8px; cursor: pointer; color: #d1d5db; font-size: 0.9rem; margin-bottom: 1rem;">
              <input type="checkbox" :checked="!!boardSettings.on_complete_move_to" @change="e => boardSettings.on_complete_move_to = e.target.checked ? activeBoard?.columns[0]?.id || '' : ''">
              Déplacer automatiquement les tâches terminées
            </label>
            
            <div v-if="boardSettings.on_complete_move_to" style="margin-left: 24px; display: flex; flex-direction: column; gap: 1rem;">
              <div>
                <label style="display: block; font-size: 0.8rem; color: #8a8f98; margin-bottom: 6px;">Colonne de destination :</label>
                <select v-model="boardSettings.on_complete_move_to" class="modern-input" style="margin-bottom: 0;">
                  <option v-for="col in activeBoard?.columns" :key="col.id" :value="col.id">{{ col.name }}</option>
                </select>
              </div>
              
              <label style="display: flex; align-items: center; gap: 8px; cursor: pointer; color: #d1d5db; font-size: 0.85rem;">
                <input type="checkbox" v-model="boardSettings.apply_to_subtasks">
                Appliquer également aux sous-tâches
              </label>

              <label style="display: flex; align-items: center; gap: 8px; cursor: pointer; color: #d1d5db; font-size: 0.85rem;">
                <input type="checkbox" v-model="boardSettings.auto_validate_on_move">
                Valider / Décocher automatiquement lors du déplacement manuel
              </label>
            </div>
          </div>

          <div class="modal-actions" style="margin-top: 1.5rem;">
            <button type="button" class="secondary-btn" @click="showBoardSettingsModal = false">Annuler</button>
            <button type="submit" class="primary-btn">Enregistrer</button>
          </div>
        </form>
      </div>
    </div>

    <!-- Modale de Création Espace -->
    <div v-if="showCreateModal" class="modal-overlay" @click.self="showCreateModal = false">
      <div class="modal">
        <h2>Nouvel espace de travail</h2>
        <form @submit.prevent="createWorkspace">
          <input v-focus v-model="newWorkspaceName" type="text" placeholder="Ex: Projet Refonte UI" class="modern-input" required>
          <div class="modal-actions">
            <button type="button" class="secondary-btn" @click="showCreateModal = false">Annuler</button>
            <button type="submit" class="primary-btn" :disabled="isCreatingWorkspace">Créer</button>
          </div>
        </form>
      </div>
    </div>
    
    <!-- Modale de Création Tableau -->
    <div v-if="showBoardModal" class="modal-overlay" @click.self="showBoardModal = false">
      <div class="modal">
        <h2>Nouveau tableau Kanban</h2>
        <form @submit.prevent="createBoard">
          <input v-focus v-model="newBoardName" type="text" placeholder="Nom du tableau" class="modern-input" required>
          <div class="modal-actions">
            <button type="button" class="secondary-btn" @click="showBoardModal = false">Annuler</button>
            <button type="submit" class="primary-btn">Créer</button>
          </div>
        </form>
      </div>
    </div>
    
    <!-- Modale de Création Colonne -->
    <div v-if="showColumnModal" class="modal-overlay" @click.self="showColumnModal = false">
      <div class="modal">
        <h2>Nouvelle colonne</h2>
        <form @submit.prevent="createColumn">
          <input v-focus v-model="newColumnName" type="text" placeholder="Nom de la colonne" class="modern-input" required>
          <div class="modal-actions">
            <button type="button" class="secondary-btn" @click="showColumnModal = false">Annuler</button>
            <button type="submit" class="primary-btn">Créer</button>
          </div>
        </form>
      </div>
    </div>
    
        <!-- Modale de Détail/Edition de Tâche (Slide-over) -->
    <div v-if="showTaskDetailModal" class="slideover-overlay" @click.self="showTaskDetailModal = false">
      <div class="slideover-panel">
        <form @submit.prevent="updateTask" style="display: flex; flex-direction: column; height: 100%;">
          <div class="slideover-header">
            <input v-focus v-model="editTaskTitle" type="text" placeholder="Titre de la tâche" class="modern-input title-input" required>
            <button type="button" class="close-btn" @click="showTaskDetailModal = false">✕</button>
          </div>
          
          <div class="slideover-body">
            <div class="slideover-main">
              <!-- Meta horizontale -->
              <div class="slideover-meta-horizontal">
                <div class="meta-field">
                  <span class="meta-label">Priorité</span>
                  <select v-model="editTaskPriority" class="modern-select meta-select">
                    <option :value="0">Sans priorité</option>
                    <option :value="1">🔴 Urgente</option>
                    <option :value="2">🟠 Haute</option>
                    <option :value="3">🟡 Moyenne</option>
                    <option :value="4">🔵 Basse</option>
                  </select>
                </div>
                <div class="meta-field">
                  <span class="meta-label">Début</span>
                  <input v-model="editTaskStartDate" type="date" class="modern-input meta-input">
                </div>
                <div class="meta-field">
                  <span class="meta-label">Échéance</span>
                  <input v-model="editTaskDueDate" type="date" class="modern-input meta-input">
                </div>
              </div>
              <div class="slideover-meta-horizontal" style="margin-top: 1rem;">
                <div class="meta-section" style="flex: 1; min-width: 200px;">
                  <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;">
                    <label class="section-label" style="margin: 0;">Labels</label>
                    <button type="button" @click="toggleCreateLabel" style="background:none; border:none; color:#5e6ad2; cursor:pointer; font-size: 0.75rem;">+ Créer</button>
                  </div>
                  
                  <div v-if="showCreateLabel" style="margin-bottom: 8px; display: flex; flex-direction: column; gap: 8px; background: #222327; padding: 8px; border-radius: 4px;">
                    <input v-model="newLabelName" type="text" class="modern-input" placeholder="Nom du label" style="margin-bottom: 0; padding: 6px;">
                    <div style="display: flex; gap: 8px; align-items: center;">
                       <input v-model="newLabelColor" type="color" style="height: 28px; width: 40px; border:none; background:none; cursor:pointer;">
                       <button type="button" class="primary-btn small-btn" @click="createLabel" :disabled="!newLabelName" style="flex: 1;">{{ editingLabelId ? 'Sauver' : 'Ajouter' }}</button>
                       <button type="button" class="secondary-btn small-btn" @click="showCreateLabel = false">✕</button>
                    </div>
                  </div>

                  <input v-model="labelSearch" type="text" class="modern-input" placeholder="Rechercher..." style="margin-bottom: 8px; padding: 6px; font-size: 0.8rem;" />
                  <div style="display: flex; gap: 4px; flex-wrap: wrap;">
                     <span v-for="lbl in filteredLabels" :key="lbl.id" 
                       class="task-label-chip clickable-chip" 
                       :class="{ 'selected-chip': isLabelSelected(lbl.id) }"
                       :style="{ backgroundColor: isLabelSelected(lbl.id) ? lbl.color : '#2f3136', border: isLabelSelected(lbl.id) ? 'none' : '1px solid #4a4d55', color: isLabelSelected(lbl.id) ? '#fff' : '#9ca3af' }"
                       @click="toggleLabel(lbl)">
                       {{ lbl.name }}
                       <button type="button" @click.stop="startEditLabel(lbl)" title="Modifier ce label" style="background:none; border:none; color:inherit; cursor:pointer; margin-left:4px; font-size:10px; opacity:0.7">✏️</button>
                       <button type="button" @click.stop="deleteBoardLabel(lbl.id)" title="Supprimer définitivement" style="background:none; border:none; color:inherit; cursor:pointer; font-size:10px; opacity:0.7">🗑</button>
                     </span>
                  </div>
                </div>

                <div class="meta-section" style="flex: 1; min-width: 200px;">
                  <label class="section-label">Membres assignés</label>
                  <input v-model="assigneeSearch" type="text" class="modern-input" placeholder="Rechercher..." style="margin-bottom: 8px; padding: 6px; font-size: 0.8rem;" />
                  <div style="display: flex; gap: 6px; flex-wrap: wrap;">
                     <span v-for="m in filteredAssignees" :key="m.user_id" 
                       class="assignee-chip clickable-chip"
                       :class="{ 'selected-chip': isAssigneeSelected(m.user_id) }"
                       :style="{ backgroundColor: isAssigneeSelected(m.user_id) ? '#5e6ad2' : '#2f3136', border: isAssigneeSelected(m.user_id) ? 'none' : '1px solid #4a4d55', color: isAssigneeSelected(m.user_id) ? '#fff' : '#9ca3af' }"
                       @click="toggleAssignee(m.user_id)">
                       {{ m.name }}
                     </span>
                  </div>
                </div>
              </div>

              <label class="section-label">Description</label>
              <SimpleEditor v-model="editTaskDesc" />
              
              <div v-if="editingTask" style="margin-top: 2rem;">
                <TaskComments :task-id="editingTask.id" :workspace-members="workspaceMembers" />
              </div>

              <div v-if="editingTask" style="margin-top: 2rem;">
                <TaskAttachments :task-id="editingTask.id" />
              </div>

              <div v-if="editingTask" style="margin-top: 2rem;">
                <TaskActivities :task-id="editingTask.id" :workspace-members="workspaceMembers" />
              </div>
              
              <div v-if="editingTask" style="margin-top: 2rem;">
                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;">
                  <h3 style="margin: 0; font-size: 1rem; font-weight: 500;">Sous-tâches</h3>
                </div>
                
                <div v-for="subtask in getSubtasks(editingTask)" :key="subtask.id" class="subtask-row" @click="openTaskDetail(subtask)" style="display: flex; align-items: center; justify-content: space-between; padding: 10px; background: #2f3136; border-radius: 6px; margin-bottom: 8px; cursor: pointer; transition: background 0.2s;">
                  <div style="display: flex; align-items: center; gap: 10px;">
                    <div class="subtask-checkbox" :style="{ color: subtask.is_completed ? '#10b981' : '#52525b', fontSize: '1.2rem' }" @click.stop="toggleTaskValidation(subtask)">
                       {{ subtask.is_completed ? '☑' : '☐' }}
                    </div>
                    <span :style="{ textDecoration: subtask.is_completed ? 'line-through' : 'none', color: subtask.is_completed ? '#9ca3af' : '#eee' }">{{ subtask.title }}</span>
                  </div>
                  <span style="font-size: 0.8rem; color: #a1a1aa;">></span>
                </div>
                
                <div style="margin-top: 10px; display: flex; gap: 8px;">
                   <input v-model="newSubtaskTitle" type="text" class="modern-input" placeholder="Nouvelle sous-tâche..." @keydown.enter.prevent="createSubtask">
                   <button type="button" class="primary-btn" @click="createSubtask">Ajouter</button>
                </div>

                <div style="margin-top: 2rem; padding: 16px; background: #1e1e22; border-radius: 8px; border: 1px solid #3f3f46;">
                  <h3 style="margin: 0 0 12px 0; font-size: 1rem; font-weight: 500;">Validation</h3>
                  
                  <label style="display: flex; align-items: center; margin-bottom: 16px; font-size: 0.9rem; color: #e4e4e7; cursor: pointer;">
                    <input type="checkbox" v-model="editingTask.is_per_person" @change="saveTaskSettings" style="margin-right: 8px; width: 16px; height: 16px;">
                    Validation requise par chaque personne assignée
                  </label>

                  <div v-if="editingTask.is_per_person" class="per-person-validations" style="display: flex; gap: 8px; margin-bottom: 16px; flex-wrap: wrap;">
                    <span v-for="assignee in editingTask.assignees" :key="assignee" 
                          class="assignee-avatar" 
                          :class="{ 'validated-avatar': editingTask.validations?.includes(assignee) }"
                          :title="getMemberName(assignee) + (editingTask.validations?.includes(assignee) ? ' a validé' : ' en attente')">
                      {{ getMemberInitials(assignee) }}
                    </span>
                    <span v-if="!editingTask.assignees || editingTask.assignees.length === 0" style="color: #9ca3af; font-size: 0.85rem;">Aucun membre assigné</span>
                  </div>
                  
                  <button type="button" class="primary-btn" style="width: 100%; justify-content: center; padding: 12px; font-size: 1rem;" 
                          :class="{'btn-success': editingTask.is_completed}" 
                          :disabled="getSubtasks(editingTask).some(t => !t.is_completed)"
                          @click="toggleTaskValidation(editingTask)"
                          :title="getSubtasks(editingTask).some(t => !t.is_completed) ? 'Toutes les sous-tâches doivent être terminées' : ''">
                    <span v-if="editingTask.is_completed">✓ Tâche terminée</span>
                    <span v-else>Terminer la tâche</span>
                  </button>
                  <p v-if="getSubtasks(editingTask).some(t => !t.is_completed)" style="margin: 8px 0 0 0; font-size: 0.75rem; color: #f87171; text-align: center;">
                    Toutes les sous-tâches doivent être cochées avant de valider.
                  </p>
                </div>
              </div>
            </div>
          </div>
          
          <div class="slideover-footer">
            <button type="button" v-if="editingTask" class="secondary-btn" style="color: #f87171" @click="deleteTask">Supprimer la tâche</button>
            <div style="flex: 1"></div>
            <button type="button" class="secondary-btn" @click="showTaskDetailModal = false">Annuler</button>
            <button type="submit" class="primary-btn">{{ editingTask ? 'Enregistrer' : 'Créer' }}</button>
          </div>
        </form>
      </div>
    </div>
    <!-- Modale de Renommage -->
    <div v-if="showRenameModal" class="modal-overlay" @click.self="showRenameModal = false">
      <div class="modal">
        <h2>Renommer {{ renameType === 'workspace' ? "l'espace" : renameType === 'board' ? "le tableau" : "la colonne" }}</h2>
        <form @submit.prevent="submitRename">
          <input v-focus v-model="renameName" type="text" placeholder="Nouveau nom" class="modern-input" required>
          <div class="modal-actions">
            <button type="button" class="secondary-btn" @click="showRenameModal = false">Annuler</button>
            <button type="submit" class="primary-btn">Renommer</button>
          </div>
        </form>
      </div>
    </div>

    <!-- Modale Paramètres (Dashboard) -->
    <div v-if="showSettingsModal" class="modal-overlay" @click.self="showSettingsModal = false">
      <div class="modal settings-modal">
        <h2>Paramètres de l'espace</h2>
        <div class="members-list">
          <div v-for="m in workspaceMembers" :key="m.user_id" class="member-row">
            <div class="member-avatar">{{ m.name.charAt(0).toUpperCase() }}</div>
            <div class="member-info">
              <div class="member-name">{{ m.name }}</div>
              <div class="member-email">{{ m.email }}</div>
            </div>
            <div class="member-role" v-if="activeWorkspaceRole === 'owner'">
              <select class="modern-select" :value="m.role" @change="(e) => changeMemberRole(m.user_id, (e.target as HTMLSelectElement).value)">
                <option value="owner">Owner</option>
                <option value="admin">Admin</option>
                <option value="member">Membre</option>
              </select>
            </div>
            <div class="member-role-text" v-else>
              {{ m.role.charAt(0).toUpperCase() + m.role.slice(1) }}
            </div>
            <button v-if="activeWorkspaceRole === 'owner' && user.id !== m.user_id" @click="kickMember(m.user_id)" class="kick-btn" title="Exclure">❌</button>
          </div>
        </div>
        <div class="modal-actions" style="margin-top: 20px">
          <button type="button" class="primary-btn" @click="showSettingsModal = false">Fermer</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
@import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600&display=swap');

:global(body) {
  margin: 0; padding: 0;
  font-family: 'Inter', -apple-system, sans-serif;
  background-color: #0e0e11; color: #eeeeee;
  min-height: 100vh; -webkit-font-smoothing: antialiased;
}

.loading-screen { display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100vh; color: #888; }
.spinner { width: 24px; height: 24px; border: 2px solid #333; border-radius: 50%; border-top-color: #eee; animation: spin 1s linear infinite; margin-bottom: 1rem; }
@keyframes spin { to { transform: rotate(360deg); } }

.app-layout { display: flex; height: 100vh; overflow: hidden; }

/* Sidebar - Linear style */
.sidebar { width: 240px; background-color: #111113; display: flex; flex-direction: column; border-right: 1px solid #1e1f23; z-index: 10; }
.sidebar-header { padding: 1rem 1rem 0.5rem; }
.logo { font-size: 0.9rem; font-weight: 600; color: #fff; letter-spacing: -0.3px; opacity: 0.9; }
.sidebar-content { flex: 1; padding: 0 0.5rem; overflow-y: auto; }
.section-title { display: flex; justify-content: space-between; align-items: center; font-size: 0.7rem; color: #5a5d65; margin-bottom: 0.25rem; margin-top: 1.25rem; font-weight: 500; text-transform: uppercase; letter-spacing: 0.5px; padding: 0 0.5rem; }
.icon-btn { background: none; border: none; color: #5a5d65; cursor: pointer; padding: 2px 4px; border-radius: 4px; display: flex; align-items: center; justify-content: center; font-size: 0.85rem; transition: all 0.15s; }
.icon-btn:hover { background: #1e1f23; color: #eee; }
.workspace-list { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 1px; }
.loading-item, .empty-item { color: #5a5d65; font-size: 0.8rem; padding: 0.5rem; }
.workspace-item { display: flex; align-items: center; gap: 8px; padding: 5px 8px; border-radius: 6px; cursor: pointer; color: #8a8f98; font-size: 0.85rem; font-weight: 400; transition: all 0.12s; }
.workspace-item:hover { background: #1a1b1e; color: #ccc; }
.workspace-item.active { background: #1e1f23; color: #fff; }
.ws-avatar { width: 20px; height: 20px; border-radius: 4px; background: linear-gradient(135deg, #5e6ad2, #7c3aed); color: white; display: flex; align-items: center; justify-content: center; font-size: 0.65rem; font-weight: 600; }
.workspace-item.active .ws-avatar { background: linear-gradient(135deg, #818cf8, #a78bfa); }
.ws-name { font-size: 0.85rem; font-weight: 500; }
.sidebar-footer { padding: 0.75rem; border-top: 1px solid #1e1f23; display: flex; flex-direction: column; gap: 0.5rem; }
.user-profile { display: flex; align-items: center; gap: 8px; }
.user-avatar { width: 22px; height: 22px; border-radius: 50%; background: #333; display: flex; align-items: center; justify-content: center; font-size: 0.7rem; color: #fff; }
.user-info p { margin: 0; }
.user-name { font-weight: 500; font-size: 0.8rem; color: #ccc; }
.user-email { font-size: 0.7rem; color: #5a5d65; }
.logout-btn { display: flex; align-items: center; gap: 8px; text-decoration: none; color: #5a5d65; font-size: 0.8rem; }
.logout-btn:hover { color: #f87171; }
.item-actions { display: flex; gap: 2px; }
.item-actions button { background: none; border: none; color: #5a5d65; cursor: pointer; padding: 2px 3px; border-radius: 4px; font-size: 0.7rem; transition: all 0.12s; }
.item-actions button:hover { background: #27282b; color: #eee; }

/* Settings Modal */
.settings-modal { max-width: 500px; width: 100%; }
.members-list { display: flex; flex-direction: column; gap: 8px; margin-top: 12px; max-height: 400px; overflow-y: auto; }
.member-row { display: flex; align-items: center; gap: 10px; padding: 8px; background: rgba(255,255,255,0.02); border: 1px solid #1e1f23; border-radius: 6px; }
.member-avatar { width: 30px; height: 30px; border-radius: 50%; background: linear-gradient(135deg, #5e6ad2, #7c3aed); color: white; display: flex; align-items: center; justify-content: center; font-weight: bold; font-size: 0.8rem; }
.member-info { flex: 1; display: flex; flex-direction: column; }
.member-name { font-weight: 500; font-size: 0.85rem; color: #eee; }
.member-email { font-size: 0.7rem; color: #5a5d65; }
.modern-select { background: #111; color: #eee; border: 1px solid #1e1f23; padding: 4px 8px; border-radius: 6px; outline: none; cursor: pointer; font-family: inherit; font-size: 0.85rem; }
.member-role-text { font-size: 0.8rem; color: #5a5d65; font-weight: 500; }
.kick-btn { background: none; border: none; cursor: pointer; opacity: 0.4; transition: 0.15s; }
.kick-btn:hover { opacity: 1; }

/* Main Content */
.main-content { flex: 1; display: flex; flex-direction: column; overflow-y: auto; background-color: #0e0e11; }
.main-header { padding: 1.5rem 2rem 1rem; border-bottom: 1px solid #1e1f23; }
.main-header h1 { margin: 0; font-size: 1.1rem; font-weight: 600; letter-spacing: -0.02em; }
.subtitle { margin: 0; color: #5a5d65; font-size: 0.85rem; }
.content-body { flex: 1; display: flex; flex-direction: column; }

/* Filter Bar - Linear style */
.filter-bar { display: flex; gap: 12px; padding: 8px 2rem; background: transparent; border-bottom: 1px solid #1e1f23; align-items: center; flex-wrap: wrap; }

/* Kanban Board - Linear style */
.kanban-board { display: flex; gap: 1rem; align-items: flex-start; overflow-x: auto; flex: 1; padding: 1rem 2rem; }
.kanban-column { background: transparent; border: none; border-radius: 0; width: 280px; min-width: 280px; display: flex; flex-direction: column; max-height: 100%; }
.kanban-column.add-column { background: transparent; border: 1px dashed #1e1f23; color: #5a5d65; cursor: pointer; align-items: center; justify-content: center; padding: 1rem; font-size: 0.85rem; border-radius: 8px; }
.kanban-column.add-column:hover { border-color: #5e6ad2; color: #eee; }
.column-header { padding: 0 4px 10px; display: flex; justify-content: space-between; align-items: center; }
.column-header h3 { margin: 0; font-size: 0.8rem; font-weight: 500; color: #8a8f98; text-transform: uppercase; letter-spacing: 0.3px; }
.task-count { color: #5a5d65; font-size: 0.7rem; padding: 0; }
.task-list { padding: 0; flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 2px; min-height: 50px; }

/* Task Card - Linear style */
.task-card { background: transparent; border: none; border-radius: 6px; padding: 6px 8px; cursor: pointer; transition: background 0.1s; }
.task-card:hover { background: #17181c; }
.task-card:active { cursor: grabbing; }
.task-card-top { display: flex; align-items: center; gap: 6px; margin-bottom: 2px; }
.priority-dot { font-size: 0.65rem; line-height: 1; }
.task-label-dot { width: 8px; height: 8px; border-radius: 50%; display: inline-block; }
.task-card h4 { margin: 0; font-size: 0.85rem; font-weight: 400; color: #e4e4e7; line-height: 1.4; display: flex; align-items: center; gap: 6px; }
.status-icon { font-size: 0.75rem; flex-shrink: 0; }
.status-icon.completed { color: #10b981; }
.task-title-done { text-decoration: line-through; color: #6b7280; }
.task-card-footer { display: flex; justify-content: space-between; align-items: center; margin-top: 4px; }
.ghost-task { opacity: 0.4; background: #1e1f23; border-radius: 6px; }
.add-task-btn { background: none; border: none; color: #5a5d65; cursor: pointer; padding: 8px 4px; text-align: left; font-size: 0.8rem; border-radius: 4px; transition: all 0.12s; }
.add-task-btn:hover { color: #eee; background: #17181c; }

.columns-container { display: flex; gap: 1rem; align-items: flex-start; height: 100%; min-height: 100px; }
.ghost-column { opacity: 0.5; background: #1e1f23; border-radius: 6px; }
.task-due-date { font-size: 0.7rem; color: #5a5d65; }
.task-due-date.overdue { color: #f87171; }
.task-labels { display: flex; gap: 3px; }
.task-label-chip { padding: 2px 6px; border-radius: 12px; font-size: 0.7rem; color: #fff; font-weight: 500; text-shadow: 0 1px 2px rgba(0,0,0,0.5); }
.clickable-chip { cursor: pointer; transition: all 0.12s; user-select: none; }
.clickable-chip:hover { filter: brightness(1.15); transform: scale(1.03); }
.clickable-chip:active { transform: scale(0.97); }
.selected-chip { box-shadow: 0 0 0 2px rgba(94, 106, 210, 0.4); }
.task-assignees { display: flex; margin-left: 4px; }
.assignee-avatar { width: 18px; height: 18px; border-radius: 50%; background: linear-gradient(135deg, #5e6ad2, #7c3aed); color: #fff; font-size: 0.6rem; display: flex; align-items: center; justify-content: center; font-weight: 600; margin-left: -4px; border: 1.5px solid #0e0e11; }
.assignee-avatar.more { background: #27282b; color: #8a8f98; font-size: 0.55rem; }
.assignee-chip { display: inline-flex; align-items: center; background: #1e1f23; padding: 4px 8px; border-radius: 4px; font-size: 0.8rem; color: #eee; }

.modern-input { flex: 1; padding: 6px 10px; border-radius: 6px; background: #111113; border: 1px solid #1e1f23; color: #eee; font-family: inherit; font-size: 0.85rem; width: 100%; box-sizing: border-box; margin-bottom: 0.75rem; transition: border-color 0.15s; }
.modern-input:focus { border-color: #5e6ad2; outline: none; }
.primary-btn { padding: 6px 14px; border-radius: 6px; border: none; cursor: pointer; background: #5e6ad2; color: #fff; font-weight: 500; font-size: 0.85rem; transition: all 0.12s; }
.primary-btn:hover { background: #6d79e0; }
.primary-btn:disabled { opacity: 0.4; cursor: not-allowed; }
.secondary-btn { padding: 6px 14px; border-radius: 6px; border: 1px solid #1e1f23; cursor: pointer; background: transparent; color: #8a8f98; font-weight: 500; font-size: 0.85rem; transition: all 0.12s; }
.secondary-btn:hover { background: #1e1f23; color: #eee; }

/* Modal - Linear style */
.modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.7); display: flex; align-items: center; justify-content: center; z-index: 100; backdrop-filter: blur(4px); }
.modal { width: 100%; max-width: 420px; padding: 1.25rem; border-radius: 8px; background: #17181c; border: 1px solid #1e1f23; box-shadow: 0 20px 60px rgba(0,0,0,0.6); }
.modal h2 { margin: 0 0 0.75rem 0; font-size: 1rem; font-weight: 500; }
.modal-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 1rem; }

/* Slide-over - Linear style */
.slideover-overlay { position: fixed; top: 0; left: 0; width: 100vw; height: 100vh; background: rgba(0, 0, 0, 0.4); z-index: 100; display: flex; justify-content: flex-end; backdrop-filter: blur(2px); }
.slideover-panel { width: 720px; max-width: 100%; height: 100vh; background: #111113; border-left: 1px solid #1e1f23; box-shadow: -8px 0 40px rgba(0,0,0,0.5); display: flex; flex-direction: column; }

.slideover-header { padding: 1.25rem 1.5rem; border-bottom: 1px solid #1e1f23; display: flex; justify-content: space-between; align-items: center; }
.title-input { font-size: 1.1rem; font-weight: 600; padding: 0.4rem; background: transparent; border: 1px solid transparent; width: 100%; margin-right: 1rem; color: #fff; border-radius: 4px; }
.title-input:focus { background: #17181c; border-color: #5e6ad2; }
.close-btn { background: none; border: none; color: #5a5d65; font-size: 1rem; cursor: pointer; padding: 0.4rem; border-radius: 4px; transition: 0.15s; }
.close-btn:hover { background: #1e1f23; color: #eee; }

.slideover-body { display: flex; flex: 1; flex-direction: column; overflow: hidden; }
.slideover-main { flex: 1; padding: 1.5rem; overflow-y: auto; }

.slideover-meta-horizontal { display: flex; gap: 1rem; flex-wrap: wrap; margin-bottom: 1.25rem; padding: 12px; background: #17181c; border-radius: 8px; border: 1px solid #1e1f23; }
.meta-field { display: flex; align-items: center; gap: 8px; }
.meta-label { font-size: 0.75rem; color: #5a5d65; font-weight: 500; white-space: nowrap; }
.meta-input { width: 130px; margin: 0; padding: 4px 8px; font-size: 0.8rem; }
.meta-select { margin: 0; padding: 4px 8px; font-size: 0.8rem; min-width: 120px; }

.section-label { display: block; font-size: 0.7rem; font-weight: 500; color: #5a5d65; text-transform: uppercase; letter-spacing: 0.5px; margin-bottom: 0.5rem; }
.desc-input { min-height: 120px; resize: vertical; line-height: 1.5; font-size: 0.9rem; }

/* Subtasks & Validations */
.subtask-row:hover { background: #1e1f23 !important; }
.subtask-checkbox { width: 24px; display: flex; align-items: center; justify-content: center; cursor: pointer; transition: transform 0.1s; }
.subtask-checkbox:hover { transform: scale(1.1); }
.validated-avatar { box-shadow: 0 0 0 2px #10b981; opacity: 1; }
.btn-success { background: #10b981; color: #fff; }
.btn-success:hover { background: #059669; }

.slideover-footer { padding: 1rem 1.5rem; border-top: 1px solid #1e1f23; display: flex; justify-content: space-between; align-items: center; gap: 1rem; background: #111113; }

.view-toggle { display: flex; background: #111113; border-radius: 6px; border: 1px solid #1e1f23; padding: 2px; gap: 2px; }
.view-toggle button { background: transparent; border: none; padding: 4px 10px; border-radius: 4px; color: #5a5d65; font-size: 0.8rem; font-weight: 500; cursor: pointer; transition: 0.12s; }
.view-toggle button:hover { color: #ccc; }
.view-toggle button.active { background: #1e1f23; color: #fff; }

</style>
