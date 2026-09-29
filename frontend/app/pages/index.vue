<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, watchEffect, nextTick } from 'vue'
import { useSessionStorage } from '@vueuse/core'
import gsap from 'gsap'

import { useAuth } from '../composables/useAuth'
import { useWorkspace } from '../composables/useWorkspace'
import { useBoard } from '../composables/useBoard'
import { useTask } from '../composables/useTask'
import { useKanbanWS } from '../composables/useKanbanWS'
import { useKanbanFilters } from '../composables/useKanbanFilters'

import type { Task, BoardAutomation, Label } from '../types'

import AppSidebar from '../components/AppSidebar.vue'
import FilterBar from '../components/FilterBar.vue'
import KanbanBoard from '../components/KanbanBoard.vue'
import TaskDetailSlideover from '../components/TaskDetailSlideover.vue'
import AppModal from '../components/AppModal.vue'
import BoardSettingsModal from '../components/BoardSettingsModal.vue'
import WorkspaceMembersModal from '../components/WorkspaceMembersModal.vue'
import TimelineView from '../components/TimelineView.vue'
import WorkspacePages from '../components/WorkspacePages.vue'
import WorkspaceDashboard from '../components/WorkspaceDashboard.vue'

// --- State ---
const { user, isLoading } = useAuth()
const {
  activeWorkspaceId, activeWorkspaceRole, workspacesData, isLoadingWorkspaces, 
  workspaceMembers, pages, isLoadingPages, boards, myTasks,
  selectWorkspace, createWorkspace, isCreatingWorkspace, renameWorkspace, deleteWorkspace, 
  createPage, updatePage, deletePage, changeMemberRole, kickMember, copyInviteLink
} = useWorkspace()

const {
  activeBoardId, activeBoard, selectBoard, createBoard, renameBoard, deleteBoard,
  createColumn, renameColumn, deleteColumn, moveColumn, saveBoardSettings,
  createLabel, editLabel, deleteLabel
} = useBoard()

const {
  createTask, updateTask, deleteTask, moveTask, toggleTaskValidation, createSubtask, toggleTaskLabel, toggleTaskAssignee
} = useTask()

const {
  searchQuery, filterLabels, filterAssignees, hideCompleted, toggleFilterLabel, toggleFilterAssignee, filteredBoard
} = useKanbanFilters(activeBoard)

const currentModule = useSessionStorage<'kanban' | 'wiki' | 'home' | 'dashboard'>('looty_current_module', 'dashboard')
const currentView = ref<'kanban' | 'timeline'>('kanban')
const activePageId = useSessionStorage<string | null>('looty_active_page', null)
const activePage = computed({
  get: () => pages.value?.find((p: any) => p.id === activePageId.value) || null,
  set: (page: any) => { activePageId.value = page ? page.id : null }
})

const taskStack = ref<any[]>([])
const editingTask = computed(() => taskStack.value[taskStack.value.length - 1] || null)
const targetColumnId = ref<string | null>(null)
const isSidebarOpen = ref(false)

// --- Modals State ---
const showCreateModal = ref(false)
const newWorkspaceName = ref('')
const showBoardModal = ref(false)
const newBoardName = ref('')
const showColumnModal = ref(false)
const newColumnName = ref('')
const showTaskDetailModal = ref(false)
const showBoardSettingsModal = ref(false)
const showSettingsModal = ref(false)
const showRenameModal = ref(false)
const renameType = ref<'workspace'|'board'|'column'|null>(null)
const renameId = ref<string | null>(null)
const renameName = ref('')
const boardSettingsForm = ref<BoardAutomation>({})

const confirmAction = (msg: string) => window.confirm(msg)

// --- WebSockets ---
const protocol = typeof window !== 'undefined' && window.location.protocol === 'https:' ? 'wss:' : 'ws:'
const isLocal = typeof window !== 'undefined' && (window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1')
const wsUrl = computed(() => {
  if (!activeWorkspaceId.value) return ''
  return isLocal 
    ? `ws://127.0.0.1:8080/api/ws?workspace_id=${activeWorkspaceId.value}`
    : `${protocol}//${typeof window !== 'undefined' ? window.location.host : ''}/api/ws?workspace_id=${activeWorkspaceId.value}`
})
useKanbanWS(wsUrl, activeBoard, boards, taskStack, pages, activePage)

// --- Watchers & Lifecycle ---
watch(() => boards.value, (newBoards) => {
  if (newBoards && newBoards.length > 0) {
    if (!newBoards.find(b => b.id === activeBoardId.value)) {
      if (currentModule.value === 'kanban') {
        selectBoard(newBoards[0].id)
      }
    }
  } else if (newBoards && newBoards.length === 0) {
    if (currentModule.value === 'kanban') {
      selectBoard(null)
    }
  }
}, { immediate: true })

watch(() => activeBoard.value, (newBoard) => {
  if (newBoard && taskStack.value.length > 0) {
    for (let i = 0; i < taskStack.value.length; i++) {
      const currentTask = taskStack.value[i]
      for (const col of newBoard.columns) {
        const updatedTask = col.tasks.find((t: any) => t.id === currentTask.id)
        if (updatedTask) {
          // Replace with the fresh reference so the slideover updates visually
          taskStack.value[i] = updatedTask
          break
        }
      }
    }
  }
}, { deep: true })

watch(activeWorkspaceId, (wsId) => {
  if (!wsId) {
    selectBoard(null)
    activePage.value = null
  }
})

watchEffect(() => {
  if (!isLoading.value && !user.value?.authenticated) navigateTo('/login')
})

const handleKeydown = (e: KeyboardEvent) => {
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

onMounted(() => { window.addEventListener('keydown', handleKeydown) })
onUnmounted(() => { window.removeEventListener('keydown', handleKeydown) })

// --- GSAP Animations ---
function animateSlideoverOpen() {
  nextTick(() => {
    const panel = document.querySelector('.slideover-panel')
    if (panel) gsap.fromTo(panel, { x: '100%', opacity: 0.5 }, { x: '0%', opacity: 1, duration: 0.35, ease: 'power3.out' })
  })
}

function animateModalOpen(selector: string) {
  nextTick(() => {
    const modal = document.querySelector(selector + ' .modal')
    if (modal) gsap.fromTo(modal, { scale: 0.95, opacity: 0, y: 10 }, { scale: 1, opacity: 1, y: 0, duration: 0.25, ease: 'power2.out' })
  })
}

watch(showTaskDetailModal, (v) => { if (v) animateSlideoverOpen() })
watch([showCreateModal, showBoardModal, showColumnModal, showRenameModal, showSettingsModal, showBoardSettingsModal], (vs) => {
  if (vs.some(v => v)) animateModalOpen('.modal-overlay')
})

// --- Handlers ---
const handleCreateWorkspace = async () => {
  if (!newWorkspaceName.value.trim()) return
  const ws = await createWorkspace(newWorkspaceName.value)
  selectWorkspace(ws.id)
  showCreateModal.value = false
  newWorkspaceName.value = ''
}

const handleCreateBoard = async () => {
  if (!newBoardName.value.trim() || !activeWorkspaceId.value) return
  const b = await createBoard({ workspaceId: activeWorkspaceId.value, name: newBoardName.value })
  selectBoard(b.id)
  showBoardModal.value = false
  newBoardName.value = ''
}

const handleCreateColumn = async () => {
  if (!newColumnName.value.trim() || !activeBoard.value) return
  await createColumn({ boardId: activeBoard.value.id, name: newColumnName.value })
  showColumnModal.value = false
  newColumnName.value = ''
}

const openRenameModal = (type: 'workspace'|'board'|'column', id: string, name: string) => {
  renameType.value = type; renameId.value = id; renameName.value = name; showRenameModal.value = true
}

const handleRename = async () => {
  if (!renameName.value.trim() || !renameId.value || !renameType.value) return
  if (renameType.value === 'workspace') await renameWorkspace({ id: renameId.value, name: renameName.value })
  else if (renameType.value === 'board') await renameBoard({ id: renameId.value, name: renameName.value })
  else if (renameType.value === 'column') await renameColumn({ id: renameId.value, name: renameName.value })
  showRenameModal.value = false
}

const openBoardSettingsModal = () => {
  if (!activeBoard.value) return
  boardSettingsForm.value = {
    on_complete_move_to: activeBoard.value.automations?.on_complete_move_to || '',
    apply_to_subtasks: activeBoard.value.automations?.apply_to_subtasks || false,
    auto_validate_on_move: activeBoard.value.automations?.auto_validate_on_move || false
  }
  showBoardSettingsModal.value = true
}

const handleSaveBoardSettings = async () => {
  if (!activeBoard.value) return
  await saveBoardSettings({ id: activeBoard.value.id, automations: boardSettingsForm.value })
  showBoardSettingsModal.value = false
}

const openTaskModal = (colId: string) => {
  taskStack.value = []
  targetColumnId.value = colId
  showTaskDetailModal.value = true
}

const handleOpenTask = (task: any) => {
  taskStack.value.push(task)
  showTaskDetailModal.value = true
}

const currentSubtasks = computed(() => {
  if (!editingTask.value || !activeBoard.value) return []
  const allTasks = activeBoard.value.columns.flatMap(c => c.tasks)
  return allTasks.filter(t => t.parent_id === editingTask.value!.id)
})

const closeTaskModal = () => {
  taskStack.value.pop()
  if (taskStack.value.length === 0) showTaskDetailModal.value = false
}

const handleSaveTask = async (taskData: any) => {
  if (editingTask.value) {
    await updateTask({ id: editingTask.value.id, taskData })
  } else if (targetColumnId.value) {
    await createTask({ columnId: targetColumnId.value, taskData })
  }
  closeTaskModal()
}

const handleUpdateTask = async (id: string, taskData: any) => {
  await updateTask({ id, taskData })
}

const handleDeleteTask = async () => {
  if (!editingTask.value) return
  if (confirmAction("Voulez-vous vraiment supprimer cette tâche ?")) {
    await deleteTask(editingTask.value.id)
    closeTaskModal()
  }
}

const handleTaskMove = async (evt: any, toColumnId: string) => {
  if (evt.added || evt.moved) {
    const task = evt.added ? evt.added.element : evt.moved.element
    const newIndex = evt.added ? evt.added.newIndex : evt.moved.newIndex
    
    const col = filteredBoard.value?.columns.find((c: any) => c.id === toColumnId)
    if (!col) return
    let newPosition = 65536.0
    if (col.tasks.length <= 1) {
      newPosition = 65536.0
    } else if (newIndex === 0) {
      newPosition = (col.tasks[1]?.position ?? 65536.0) / 2.0
    } else if (newIndex === col.tasks.length - 1) {
      newPosition = (col.tasks[newIndex - 1]?.position ?? 0) + 65536.0
    } else {
      newPosition = ((col.tasks[newIndex - 1]?.position ?? 0) + (col.tasks[newIndex + 1]?.position ?? 65536.0)) / 2.0
    }
    
    task.position = newPosition
    task.column_id = toColumnId
    
    let shouldToggle = false
    const auto = activeBoard.value?.automations
    if (evt.added && auto?.auto_validate_on_move && auto?.on_complete_move_to) {
      if (toColumnId === auto.on_complete_move_to && !task.is_completed) {
        task.is_completed = true; shouldToggle = true
      } else if (toColumnId !== auto.on_complete_move_to && task.is_completed) {
        task.is_completed = false; shouldToggle = true
      }
    }
    
    await moveTask({ id: task.id, columnId: toColumnId, position: newPosition })
    if (shouldToggle) await toggleTaskValidation(task.id)
  }
}



const handleColumnMove = async (evt: any) => {
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
    await moveColumn({ id: column.id, position: newPosition })
  }
}

const handleSelectPage = (page: any) => {
  currentModule.value = 'wiki'
  activeBoardId.value = null
  activePage.value = page
  isSidebarOpen.value = false
}

const handleSelectBoard = (board: any) => {
  if (board) selectBoard(board.id)
  else selectBoard(null)
  currentModule.value = 'kanban'
  activePage.value = null
  isSidebarOpen.value = false
}

const handleSelectWorkspaceDashboard = () => {
  currentModule.value = 'dashboard'
  activeBoardId.value = null
  activePage.value = null
  isSidebarOpen.value = false
}

const handleSelectWorkspace = (id: string) => {
  selectWorkspace(id)
  handleSelectWorkspaceDashboard()
}

const handleCreatePage = async () => {
  if (activeWorkspaceId.value) {
    const page = await createPage({ workspaceId: activeWorkspaceId.value })
    if (page) handleSelectPage(page)
  }
}

const handleCreateSubpage = async (parentId: string) => {
  if (activeWorkspaceId.value) {
    const page = await createPage({ workspaceId: activeWorkspaceId.value, parentId })
    if (page) handleSelectPage(page)
  }
}

const handleMovePage = async (payload: { pageId: string, parentId: string | null, position: number }) => {
  if (activeWorkspaceId.value) {
    await updatePage({
      workspaceId: activeWorkspaceId.value,
      pageId: payload.pageId,
      data: { parent_id: payload.parentId, position: payload.position }
    })
  }
}

const handleDeletePage = async (id: string) => {
  if (activeWorkspaceId.value && confirmAction('Supprimer cette page ?')) {
    await deletePage({ workspaceId: activeWorkspaceId.value, pageId: id })
    if (activePage.value?.id === id) activePage.value = null
  }
}

const handleToggleTaskValidation = async (task: any) => {
  if (activeWorkspaceRole.value === 'viewer') return
  const wasCompleted = task.is_completed;
  await toggleTaskValidation(task.id)
  const auto = activeBoard.value?.automations
  if (auto) {
    if (task.parent_id && !auto.apply_to_subtasks) return;
    if (!wasCompleted && auto.on_complete_move_to && task.column_id !== auto.on_complete_move_to) {
       await moveTask({ id: task.id, columnId: auto.on_complete_move_to, position: 65536.0, previousColumnId: task.column_id })
    } else if (wasCompleted && task.previous_column_id && task.column_id !== task.previous_column_id) {
       await moveTask({ id: task.id, columnId: task.previous_column_id, position: 65536.0, clearPreviousColumnId: true })
    }
  }
}

const handleToggleTaskLabel = async (task: Task | null, label: Label) => {
  if (task) {
    const action = task.labels?.some(l => l.id === label.id) ? 'remove' : 'add'
    
    // Optimistic update
    if (action === 'add') {
      task.labels = [...(task.labels || []), label]
    } else {
      task.labels = (task.labels || []).filter(l => l.id !== label.id)
    }

    await toggleTaskLabel({ taskId: task.id, labelId: label.id, action })
  }
}

const handleToggleTaskAssignee = async (task: Task | null, userId: string) => {
  if (task) {
    const action = task.assignees?.includes(userId) ? 'remove' : 'add'
    
    // Optimistic update
    if (action === 'add') {
      task.assignees = [...(task.assignees || []), userId]
    } else {
      task.assignees = (task.assignees || []).filter(id => id !== userId)
    }

    await toggleTaskAssignee({ taskId: task.id, userId, action })
  }
}
</script>

<template>
  <div v-if="isLoading" class="loading-screen">
    <div class="spinner"></div><p>Chargement de Looty...</p>
  </div>
  
  <div v-else-if="user?.authenticated" class="app-layout" :class="{ 'sidebar-open': isSidebarOpen }">
    <div class="mobile-overlay" v-if="isSidebarOpen" @click="isSidebarOpen = false"></div>
    <AppSidebar 
      class="app-sidebar"
      :class="{ 'is-open': isSidebarOpen }"
      :user="user"
      :workspacesData="workspacesData"
      :isLoadingWorkspaces="isLoadingWorkspaces"
      :activeWorkspaceId="activeWorkspaceId"
      :activeWorkspaceRole="activeWorkspaceRole"
      :boards="boards || []"
      :pages="pages || []"
      :isLoadingPages="isLoadingPages"
      :activeBoard="activeBoard || null"
      :activePage="activePage"
      :currentModule="currentModule"
      @select-workspace="handleSelectWorkspace"
      @go-dashboard="handleSelectWorkspaceDashboard"
      @select-board="handleSelectBoard"
      @select-page="handleSelectPage"
      @open-settings="showSettingsModal = true"
      @open-rename="openRenameModal"
      @delete-workspace="(id) => confirmAction('Supprimer cet espace ?') && deleteWorkspace(id)"
      @delete-board="(id) => confirmAction('Supprimer ce tableau ?') && deleteBoard(id)"
      @delete-page="handleDeletePage"
      @create-page="handleCreatePage"
      @create-subpage="handleCreateSubpage"
      @move-page="handleMovePage"
      @show-create-modal="showCreateModal = true"
      @show-board-modal="showBoardModal = true"
      @copy-invite-link="activeWorkspaceId && copyInviteLink(activeWorkspaceId)"
    />

    <main class="main-content">
      <header class="main-header">
        <div style="display: flex; align-items: center; width: 100%; gap: 12px; margin-bottom: 8px;">
          <button class="mobile-menu-btn" @click="isSidebarOpen = true">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M4 6h16M4 12h16M4 18h16"/></svg>
          </button>
          
          <template v-if="activeBoard">
            <div style="display: flex; justify-content: space-between; align-items: center; width: 100%;">
              <div style="display: flex; align-items: center; gap: 12px;">
                <h1 style="margin: 0; font-size: 1.2rem;">{{ activeBoard.name }}</h1>
                <button class="icon-btn" @click="openBoardSettingsModal" title="Paramètres du tableau" style="font-size: 1.2rem;">⚙️</button>
              </div>
              <div class="view-toggle">
                <button :class="{ active: currentView === 'kanban' }" @click="currentView = 'kanban'">Kanban</button>
                <button :class="{ active: currentView === 'timeline' }" @click="currentView = 'timeline'">Timeline</button>
              </div>
            </div>
          </template>
          <template v-else-if="activeWorkspaceId && currentModule === 'wiki'">
            <div style="width: 100%; display: flex; align-items: center; gap: 8px;">
              <div style="color: var(--text-color-3); font-size: 0.9rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">
                Wiki / {{ activePage ? (activePage.title || 'Nouvelle page') : 'Sélectionnez une page' }}
              </div>
              <div style="margin-left: auto; display: flex; align-items: center; gap: 8px;" id="wiki-header-actions">
              </div>
            </div>
          </template>
        </div>
        <div v-if="!activeBoard && (!activeWorkspaceId || currentModule !== 'wiki')" style="display: flex; align-items: center; gap: 12px;">
           <button v-if="!isSidebarOpen" class="mobile-menu-btn" @click="isSidebarOpen = true">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M4 6h16M4 12h16M4 18h16"/></svg>
          </button>
          <div v-if="activeWorkspaceId">
            <h1>Sélectionnez un tableau</h1>
            <p class="subtitle">Créez ou sélectionnez un tableau Kanban dans la barre latérale.</p>
          </div>
          <div v-else>
            <h1>Bienvenue sur Looty</h1>
            <p class="subtitle">Sélectionnez ou créez un espace de travail pour commencer.</p>
          </div>
        </div>
      </header>

      <div class="content-body" v-if="activeBoard">
        <FilterBar 
          v-model:searchQuery="searchQuery"
          v-model:hideCompleted="hideCompleted"
          :filterAssignees="filterAssignees"
          :filterLabels="filterLabels"
          :workspaceMembers="workspaceMembers || []"
          :boardLabels="activeBoard.labels || []"
          @toggle-assignee="toggleFilterAssignee"
          @toggle-label="toggleFilterLabel"
        />

        <KanbanBoard 
          v-if="currentView === 'kanban'"
          :board="filteredBoard"
          :workspaceMembers="workspaceMembers || []"
          :activeWorkspaceRole="activeWorkspaceRole"
          @column-move="handleColumnMove"
          @task-move="handleTaskMove"
          @open-task="handleOpenTask"
          @toggle-validation="handleToggleTaskValidation"
          @open-rename="openRenameModal"
          @delete-column="(id) => confirmAction('Supprimer colonne ?') && deleteColumn(id)"
          @open-task-modal="openTaskModal"
          @show-column-modal="showColumnModal = true"
        />
        
        <TimelineView 
          v-else-if="currentView === 'timeline'" 
          :board="filteredBoard" 
          :openTaskDetail="handleOpenTask" 
        />
      </div>

      <div v-if="activeWorkspaceId && currentModule === 'wiki'" style="flex: 1; overflow-y: auto;">
        <WorkspacePages 
          v-if="activePage" 
          :workspace-id="activeWorkspaceId" 
          :page="activePage" 
          :current-user="user?.user"
          @update-title="(t) => { activePage.title = t; /* Note: could trigger query invalidation if needed */ }"
        />
        <div v-else style="display: flex; height: 100%; align-items: center; justify-content: center; color: #8a8f98;">
          Sélectionnez une page dans le menu de gauche ou créez-en une nouvelle.
        </div>
      </div>
      
      <div v-if="activeWorkspaceId && currentModule === 'dashboard'" style="flex: 1; overflow-y: auto;">
        <WorkspaceDashboard 
          :boards="boards || []"
          :pages="pages || []"
          :myTasks="myTasks || []"
          :user-id="user?.id"
          @open-task="(task) => handleOpenTask(task)"
          @open-page="handleSelectPage"
        />
      </div>
    </main>
    
    <!-- Modals -->
    <AppModal 
      v-if="showCreateModal" 
      title="Nouvel espace de travail" 
      v-model="newWorkspaceName" 
      placeholder="Ex: Projet Refonte UI" 
      @close="showCreateModal = false" 
      @submit="handleCreateWorkspace" 
      :disabled="isCreatingWorkspace"
    />
    
    <AppModal 
      v-if="showBoardModal" 
      title="Nouveau tableau Kanban" 
      v-model="newBoardName" 
      placeholder="Nom du tableau" 
      @close="showBoardModal = false" 
      @submit="handleCreateBoard" 
    />
    
    <AppModal 
      v-if="showColumnModal" 
      title="Nouvelle colonne" 
      v-model="newColumnName" 
      placeholder="Nom de la colonne" 
      @close="showColumnModal = false" 
      @submit="handleCreateColumn" 
    />
    
    <AppModal 
      v-if="showRenameModal" 
      :title="`Renommer ${renameType === 'workspace' ? 'l\'espace' : renameType === 'board' ? 'le tableau' : 'la colonne'}`" 
      v-model="renameName" 
      placeholder="Nouveau nom" 
      @close="showRenameModal = false" 
      @submit="handleRename" 
    />

    <BoardSettingsModal 
      v-if="showBoardSettingsModal"
      :activeBoard="activeBoard"
      v-model:boardSettings="boardSettingsForm"
      @close="showBoardSettingsModal = false"
      @save="handleSaveBoardSettings"
    />

    <WorkspaceMembersModal 
      v-if="showSettingsModal"
      :workspaceMembers="workspaceMembers || []"
      :activeWorkspaceRole="activeWorkspaceRole"
      :currentUser="user?.user"
      @close="showSettingsModal = false"
      @change-role="(id, r) => activeWorkspaceId && changeMemberRole({workspaceId: activeWorkspaceId, memberId: id, role: r})"
      @kick-member="(id) => confirmAction('Exclure ?') && activeWorkspaceId && kickMember({workspaceId: activeWorkspaceId, memberId: id})"
    />

    <TaskDetailSlideover 
      v-if="showTaskDetailModal"
      :editingTask="editingTask"
      :workspaceMembers="workspaceMembers || []"
      :activeWorkspaceRole="activeWorkspaceRole"
      :taskSubtasks="currentSubtasks"
      :filteredLabels="activeBoard?.labels || []"
      :filteredAssignees="workspaceMembers || []"
      :targetColumnId="targetColumnId"
      @close="closeTaskModal"
      @save="handleSaveTask"
      @delete="handleDeleteTask"
      @create-label="(name, color, id) => activeBoard && (id ? editLabel({labelId: id, name, color}) : createLabel({boardId: activeBoard.id, name, color}))"
      @delete-label="(id) => confirmAction('Supprimer label ?') && deleteLabel(id)"
      @toggle-validation="handleToggleTaskValidation"
      @create-subtask="(id, title) => createSubtask({parentId: id, title})"
      @open-task="handleOpenTask"
      @update-task="handleUpdateTask"
      @toggle-task-label="handleToggleTaskLabel"
      @toggle-task-assignee="handleToggleTaskAssignee"
    />
  </div>
</template>

<style scoped>
@import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600&display=swap');

:global(body) {
  -webkit-font-smoothing: antialiased;
}

.loading-screen { display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100vh; color: #888; }
.spinner { width: 24px; height: 24px; border: 2px solid #333; border-radius: 50%; border-top-color: #eee; animation: spin 1s linear infinite; margin-bottom: 1rem; }
@keyframes spin { to { transform: rotate(360deg); } }

.app-layout { display: flex; height: 100vh; overflow: hidden; position: relative; }
.main-content { flex: 1; display: flex; flex-direction: column; overflow: hidden; background-color: var(--bg-color-main); }
.main-header { padding: 1.2rem 1.5rem 0.8rem; border-bottom: 1px solid var(--border-color); display: flex; flex-direction: column; }
.main-header h1 { margin: 0; font-size: 1.1rem; font-weight: 600; letter-spacing: -0.02em; }
.subtitle { margin: 0; color: var(--text-color-3); font-size: 0.85rem; }
.content-body { flex: 1; display: flex; flex-direction: column; overflow: hidden; }

.mobile-menu-btn { display: none; background: none; border: none; cursor: pointer; color: var(--text-color-2); padding: 4px; border-radius: 4px; }
.mobile-menu-btn:hover { background: var(--bg-color-hover); color: var(--text-color); }
.mobile-overlay { display: none; position: fixed; inset: 0; background: rgba(0,0,0,0.4); z-index: 40; backdrop-filter: blur(2px); }

@media (max-width: 768px) {
  .mobile-menu-btn { display: flex; align-items: center; justify-content: center; }
  .mobile-overlay { display: block; }
  .app-sidebar { position: fixed; left: 0; top: 0; bottom: 0; z-index: 50; transform: translateX(-100%); transition: transform 0.3s cubic-bezier(0.4, 0, 0.2, 1); box-shadow: var(--shadow-lg); }
  .app-sidebar.is-open { transform: translateX(0); }
  .view-toggle { display: none; }
  .main-header { padding: 1rem; }
}

.icon-btn { background: none; border: none; color: var(--text-color-3); cursor: pointer; padding: 2px 4px; border-radius: 4px; display: flex; align-items: center; justify-content: center; font-size: 0.85rem; transition: all 0.15s; }
.icon-btn:hover { background: var(--border-color); color: var(--text-color); }

.view-toggle { display: flex; background: var(--bg-color-2); border-radius: 6px; border: 1px solid var(--border-color); padding: 2px; gap: 2px; }
.view-toggle button { background: transparent; border: none; padding: 4px 10px; border-radius: 4px; color: var(--text-color-3); font-size: 0.8rem; font-weight: 500; cursor: pointer; transition: 0.12s; }
.view-toggle button:hover { color: #ccc; }
.view-toggle button.active { background: var(--border-color); color: #fff; }

.secondary-btn { padding: 4px 12px; border-radius: 6px; border: 1px solid var(--border-color); cursor: pointer; background: transparent; color: var(--text-color-3); font-weight: 500; font-size: 0.85rem; transition: all 0.12s; }
.secondary-btn:hover { background: var(--border-color); color: #eee; }
</style>
