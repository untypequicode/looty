<script setup lang="ts">
import { useWebSocket } from '@vueuse/core'
import { VueDraggableNext } from 'vue-draggable-next'

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
const wsUrl = computed(() => activeWorkspaceId.value ? `ws://localhost:8081/api/ws?workspace_id=${activeWorkspaceId.value}` : null)

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
    const response = await fetch(`/api/boards/tasks/${task.id}/toggle_validation`, { method: 'PUT' })
    if (response.ok) {
      const updated = await response.json();
      Object.assign(task, updated);
    }
  } catch (e) {}
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
  } else {
    boards.value = []
    activeBoard.value = null
    workspaceMembers.value = []
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
  createTaskLabels.value = []
  createTaskAssignees.value = []
  showTaskDetailModal.value = true
}

function closeTaskModal() {
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
    
    const col = activeBoard.value.columns.find((c: any) => c.id === toColumnId)
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
    
    await $fetch(`/api/boards/tasks/${task.id}/move`, { method: 'PUT', body: { column_id: toColumnId, position: newPosition } })
  }
}

async function onColumnMove(evt: any) {
  if (evt.moved && activeBoard.value) {
    const column = evt.moved.element
    const newIndex = evt.moved.newIndex
    
    const cols = activeBoard.value.columns
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
        for (const col of activeBoard.value.columns) {
          const idx = col.tasks.findIndex((t: any) => t.id === event.task.id)
          if (idx !== -1) col.tasks.splice(idx, 1)
        }
        const col = activeBoard.value.columns.find((c: any) => c.id === event.task.column_id)
        if (col) {
          col.tasks.push(event.task)
          col.tasks.sort((a: any, b: any) => a.position - b.position)
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
         const col = activeBoard.value.columns.find((c: any) => c.id === event.task.task.column_id)
         if (col) {
            const idx = col.tasks.findIndex((t: any) => t.id === event.task.task.id)
            if (idx !== -1) {
               col.tasks.splice(idx, 1, event.task)
            }
         }
         // Update in taskStack
         for (let i = 0; i < taskStack.value.length; i++) {
            if (taskStack.value[i].id === event.task.task.id) {
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
  if (e.key === 'Escape') {
    showCreateModal.value = false
    showBoardModal.value = false
    showColumnModal.value = false
    showTaskDetailModal.value = false
    showRenameModal.value = false
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
            :class="['workspace-item', { active: activeBoard?.id === b.id }]" @click="selectBoard(b)">
            <span class="ws-name" style="flex: 1"># {{ b.name }}</span>
            <div class="item-actions" v-if="activeBoard?.id === b.id && (activeWorkspaceRole === 'owner' || activeWorkspaceRole === 'admin')">
              <button @click.stop="openRenameModal('board', b.id, b.name)" title="Renommer">✏️</button>
              <button @click.stop="deleteBoard(b.id)" title="Supprimer">🗑️</button>
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
        <div v-if="activeBoard">
          <h1>{{ activeBoard.name }}</h1>
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
        <div class="kanban-board">
          <VueDraggableNext
            v-model="activeBoard.columns"
            group="columns"
            @change="onColumnMove($event)"
            class="columns-container"
            ghost-class="ghost-column"
            handle=".column-header"
            :disabled="activeWorkspaceRole === 'member'"
          >
            <div v-for="col in activeBoard.columns" :key="col.id" class="kanban-column">
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
                <div v-for="task in getRootTasks(col)" :key="task.id" class="task-card" @click="openTaskDetail(task)">
                  <div class="task-labels" v-if="task.labels && task.labels.length > 0">
                    <span v-for="lbl in task.labels" :key="lbl.id" class="task-label-chip" :style="{ backgroundColor: lbl.color }">{{ lbl.name }}</span>
                  </div>
                  <h4>
                    <span v-if="task.is_completed" style="color: #10b981; margin-right: 4px;">✓</span>
                    <span :style="{ textDecoration: task.is_completed ? 'line-through' : 'none' }">{{ task.title }}</span>
                  </h4>
                  <div style="display: flex; justify-content: space-between; align-items: center; margin-top: 8px;">
                    <div v-if="task.due_date" class="task-due-date" :class="{ 'overdue': new Date(task.due_date) < new Date() }">
                      🕒 {{ new Date(task.due_date).toLocaleDateString() }}
                    </div>
                    <div v-else></div>
                    <div class="task-assignees" v-if="task.assignees && task.assignees.length > 0">
                      <span v-for="userId in task.assignees" :key="userId" class="assignee-avatar" :class="{'validated-avatar': task.is_per_person && task.validations?.includes(userId)}" :title="getMemberName(userId)">{{ getMemberInitials(userId) }}</span>
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
      </div>
    </main>

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
            <!-- Colonne Principale -->
            <div class="slideover-main">
              <label class="section-label">Description</label>
              <textarea v-model="editTaskDesc" placeholder="Ajouter une description plus détaillée..." class="modern-input desc-input" rows="4"></textarea>
              
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
            
            <!-- Colonne Latérale (Métadonnées) -->
            <div class="slideover-sidebar">
              <div class="meta-section">
                <label class="section-label">Dates</label>
                <div style="display: flex; gap: 8px; flex-direction: column;">
                  <div style="display: flex; align-items: center; justify-content: space-between; font-size: 0.8rem;">
                     <span style="color: #9ca3af;">Début</span>
                     <input v-model="editTaskStartDate" type="date" class="modern-input" style="width: 140px; margin: 0; padding: 4px 8px;">
                  </div>
                  <div style="display: flex; align-items: center; justify-content: space-between; font-size: 0.8rem;">
                     <span style="color: #9ca3af;">Fin</span>
                     <input v-model="editTaskDueDate" type="date" class="modern-input" style="width: 140px; margin: 0; padding: 4px 8px;">
                  </div>
                </div>
              </div>
              
              <div class="meta-section">
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
              
              <div class="meta-section">
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

/* Sidebar */
.sidebar { width: 260px; background-color: #17181c; display: flex; flex-direction: column; border-right: 1px solid #27282b; z-index: 10; }
.sidebar-header { padding: 1.25rem; }
.logo { font-size: 1rem; font-weight: 600; color: #fff; letter-spacing: -0.2px; }
.sidebar-content { flex: 1; padding: 0 1rem; overflow-y: auto; }
.section-title { display: flex; justify-content: space-between; align-items: center; font-size: 0.75rem; color: #8a8f98; margin-bottom: 0.5rem; margin-top: 1rem; font-weight: 500; }
.icon-btn { background: none; border: none; color: #8a8f98; cursor: pointer; padding: 4px; border-radius: 4px; display: flex; align-items: center; justify-content: center; }
.icon-btn:hover { background: #27282b; color: #eee; }
.workspace-list { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 2px; }
.loading-item, .empty-item { color: #8a8f98; font-size: 0.85rem; padding: 0.5rem; }
.workspace-item { display: flex; align-items: center; gap: 10px; padding: 6px 8px; border-radius: 4px; cursor: pointer; color: #8a8f98; font-size: 0.875rem; font-weight: 500; }
.workspace-item:hover { background: #27282b; color: #eee; }
.workspace-item.active { background: #27282b; color: #fff; }
.ws-avatar { width: 24px; height: 24px; border-radius: 6px; background: #3b82f6; color: white; display: flex; align-items: center; justify-content: center; font-size: 0.75rem; font-weight: 600; }
.workspace-item.active .ws-avatar { background: #5e6ad2; }
.ws-name { font-size: 0.9rem; font-weight: 500; }
.sidebar-footer { padding: 1rem; border-top: 1px solid #27282b; display: flex; flex-direction: column; gap: 0.75rem; }
.user-profile { display: flex; align-items: center; gap: 10px; }
.user-avatar { width: 24px; height: 24px; border-radius: 50%; background: #333; display: flex; align-items: center; justify-content: center; font-size: 0.75rem; color: #fff; }
.user-info p { margin: 0; }
.user-name { font-weight: 500; font-size: 0.875rem; color: #eee; }
.user-email { font-size: 0.75rem; color: #8a8f98; }
.logout-btn { display: flex; align-items: center; gap: 8px; text-decoration: none; color: #8a8f98; font-size: 0.85rem; }
.logout-btn:hover { color: #f87171; }
.item-actions { display: flex; gap: 4px; }
.item-actions button { background: none; border: none; color: #8a8f98; cursor: pointer; padding: 2px 4px; border-radius: 4px; font-size: 0.75rem; }
.item-actions button:hover { background: #333; color: #eee; }

/* Settings Modal */
.settings-modal { max-width: 500px; width: 100%; }
.members-list { display: flex; flex-direction: column; gap: 12px; margin-top: 15px; max-height: 400px; overflow-y: auto; }
.member-row { display: flex; align-items: center; gap: 12px; padding: 10px; background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.05); border-radius: 8px; }
.member-avatar { width: 36px; height: 36px; border-radius: 50%; background: #6366f1; color: white; display: flex; align-items: center; justify-content: center; font-weight: bold; }
.member-info { flex: 1; display: flex; flex-direction: column; }
.member-name { font-weight: 600; font-size: 0.9rem; color: #eee; }
.member-email { font-size: 0.75rem; color: #8a8f98; }
.modern-select { background: #111; color: #eee; border: 1px solid #333; padding: 4px 8px; border-radius: 6px; outline: none; cursor: pointer; }
.member-role-text { font-size: 0.85rem; color: #8a8f98; font-weight: 500; }
.kick-btn { background: none; border: none; cursor: pointer; opacity: 0.5; transition: 0.2s; }
.kick-btn:hover { opacity: 1; }

/* Main Content */
.main-content { flex: 1; display: flex; flex-direction: column; overflow-y: auto; background-color: #0e0e11; }
.main-header { padding: 2.5rem 3rem 1.5rem 3rem; border-bottom: 1px solid #27282b; }
.main-header h1 { margin: 0 0 0.5rem 0; font-size: 1.5rem; font-weight: 600; letter-spacing: -0.02em; }
.subtitle { margin: 0; color: #8a8f98; font-size: 0.9rem; }
.content-body { padding: 2rem 3rem; flex: 1; display: flex; flex-direction: column; }

/* Kanban Board */
.kanban-board { display: flex; gap: 1.5rem; align-items: flex-start; overflow-x: auto; flex: 1; padding-bottom: 1rem; }
.kanban-column { background: #17181c; border: 1px solid #27282b; border-radius: 6px; width: 300px; min-width: 300px; display: flex; flex-direction: column; max-height: 100%; }
.kanban-column.add-column { background: transparent; border: 1px dashed #27282b; color: #8a8f98; cursor: pointer; align-items: center; justify-content: center; padding: 1rem; font-size: 0.875rem; }
.kanban-column.add-column:hover { border-color: #5e6ad2; color: #eee; }
.column-header { padding: 1rem; border-bottom: 1px solid #27282b; display: flex; justify-content: space-between; align-items: center; }
.column-header h3 { margin: 0; font-size: 0.875rem; font-weight: 500; }
.task-count { background: #27282b; color: #8a8f98; font-size: 0.75rem; padding: 2px 6px; border-radius: 12px; }
.task-list { padding: 0.5rem; flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 0.5rem; min-height: 50px; }
.task-card { background: #222327; border: 1px solid #2f3136; border-radius: 4px; padding: 0.75rem; cursor: grab; }
.task-card:active { cursor: grabbing; }
.task-card h4 { margin: 0; font-size: 0.875rem; font-weight: 400; color: #eee; }
.ghost-task { opacity: 0.4; background: #2f3136; }
.add-task-btn { background: none; border: none; color: #8a8f98; cursor: pointer; padding: 0.75rem; text-align: left; font-size: 0.875rem; border-top: 1px solid #27282b; }
.add-task-btn:hover { color: #eee; background: #1c1d21; }

.columns-container { display: flex; gap: 1.5rem; align-items: flex-start; height: 100%; min-height: 100px; }
.ghost-column { opacity: 0.5; background: #2f3136; border: 1px dashed #5e6ad2; }
.task-due-date { font-size: 0.75rem; color: #8a8f98; display: inline-flex; align-items: center; background: #1c1d21; padding: 2px 6px; border-radius: 4px; border: 1px solid #2f3136; }
.task-due-date.overdue { color: #f87171; border-color: #f87171; background: rgba(248, 113, 113, 0.1); }
.task-labels { display: flex; flex-wrap: wrap; gap: 4px; margin-bottom: 6px; }
.task-label-chip { padding: 2px 6px; border-radius: 12px; font-size: 0.7rem; color: #fff; font-weight: 500; text-shadow: 0 1px 2px rgba(0,0,0,0.5); }
.clickable-chip { cursor: pointer; transition: transform 0.1s, opacity 0.2s; user-select: none; }
.clickable-chip:hover { transform: scale(1.05); }
.clickable-chip:active { transform: scale(0.95); }
.selected-chip { box-shadow: 0 0 0 2px rgba(94, 106, 210, 0.4); }
.task-assignees { display: flex; gap: -4px; margin-left: 4px; }
.assignee-avatar { width: 20px; height: 20px; border-radius: 50%; background: #5e6ad2; color: #fff; font-size: 0.65rem; display: flex; align-items: center; justify-content: center; font-weight: 600; border: 2px solid #222327; }
.assignee-chip { display: inline-flex; align-items: center; background: #27282b; padding: 4px 8px; border-radius: 4px; font-size: 0.8rem; color: #eee; }

.modern-select { flex: 1; padding: 8px 12px; border-radius: 4px; background: #0e0e11; border: 1px solid #27282b; color: #eee; font-family: inherit; font-size: 0.875rem; width: 100%; box-sizing: border-box; }
.modern-select:focus { border-color: #5e6ad2; outline: none; }

.modern-input { flex: 1; padding: 8px 12px; border-radius: 4px; background: #0e0e11; border: 1px solid #27282b; color: #eee; font-family: inherit; font-size: 0.875rem; width: 100%; box-sizing: border-box; margin-bottom: 1rem; }
.modern-input:focus { border-color: #5e6ad2; outline: none; }
.primary-btn { padding: 8px 16px; border-radius: 4px; border: none; cursor: pointer; background: #fff; color: #000; font-weight: 500; font-size: 0.875rem; }
.primary-btn:hover { background: #e0e0e0; }
.primary-btn:disabled { opacity: 0.5; cursor: not-allowed; }
.secondary-btn { padding: 8px 16px; border-radius: 4px; border: 1px solid #27282b; cursor: pointer; background: #17181c; color: #eee; font-weight: 500; font-size: 0.875rem; }
.secondary-btn:hover { background: #27282b; }

/* Modal */
.modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.8); display: flex; align-items: center; justify-content: center; z-index: 100; }
.modal { width: 100%; max-width: 400px; padding: 1.5rem; border-radius: 6px; background: #17181c; border: 1px solid #27282b; box-shadow: 0 10px 30px rgba(0,0,0,0.5); }
.modal h2 { margin: 0 0 0.5rem 0; font-size: 1.1rem; font-weight: 500; }
.modal-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 1.5rem; }

/* Slide-over Modal */
.slideover-overlay { position: fixed; top: 0; left: 0; width: 100vw; height: 100vh; background: rgba(0, 0, 0, 0.5); z-index: 100; display: flex; justify-content: flex-end; backdrop-filter: blur(2px); }
.slideover-panel { width: 700px; max-width: 100%; height: 100vh; background: #0e0e11; border-left: 1px solid #27282b; box-shadow: -4px 0 24px rgba(0,0,0,0.5); display: flex; flex-direction: column; animation: slideInRight 0.3s cubic-bezier(0.16, 1, 0.3, 1); }
@keyframes slideInRight { from { transform: translateX(100%); } to { transform: translateX(0); } }

.slideover-header { padding: 1.5rem 2rem; border-bottom: 1px solid #27282b; display: flex; justify-content: space-between; align-items: flex-start; }
.title-input { font-size: 1.25rem; font-weight: 600; padding: 0.5rem; background: transparent; border: 1px solid transparent; width: 100%; margin-right: 1rem; color: #fff; }
.title-input:focus { background: #17181c; border-color: #3b82f6; }
.close-btn { background: none; border: none; color: #8a8f98; font-size: 1.25rem; cursor: pointer; padding: 0.5rem; border-radius: 4px; transition: 0.2s; }
.close-btn:hover { background: #27282b; color: #eee; }

.slideover-body { display: flex; flex: 1; overflow: hidden; }
.slideover-main { flex: 1.8; padding: 2rem; overflow-y: auto; border-right: 1px solid #27282b; }
.slideover-sidebar { flex: 1; padding: 1.5rem; overflow-y: auto; background: #131418; display: flex; flex-direction: column; gap: 1.5rem; }

.section-label { display: block; font-size: 0.75rem; font-weight: 600; color: #8a8f98; text-transform: uppercase; letter-spacing: 0.5px; margin-bottom: 0.75rem; }
.desc-input { min-height: 120px; resize: vertical; line-height: 1.5; font-size: 0.95rem; }

/* Subtasks & Validations */
.subtask-row:hover { background: #3f3f46 !important; }
.subtask-checkbox { width: 24px; display: flex; align-items: center; justify-content: center; cursor: pointer; transition: transform 0.1s; }
.subtask-checkbox:hover { transform: scale(1.1); }
.validated-avatar { box-shadow: 0 0 0 2px #10b981; opacity: 1; }
.btn-success { background: #10b981; color: #fff; }
.btn-success:hover { background: #059669; }

.slideover-footer { padding: 1.5rem 2rem; border-top: 1px solid #27282b; display: flex; justify-content: space-between; align-items: center; gap: 1rem; background: #0e0e11; }

</style>
