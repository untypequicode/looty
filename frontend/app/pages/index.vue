<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, watchEffect, nextTick } from 'vue'
import { useSessionStorage, useOnline } from '@vueuse/core'
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
import PresenceDock from '../components/PresenceDock.vue'

// --- State ---
import { useQueryClient } from '@tanstack/vue-query'
const queryClient = useQueryClient()
const isOnline = useOnline()
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

const taskStackIds = ref<string[]>([])
const editingTask = computed(() => {
  if (taskStackIds.value.length === 0) return null
  const currentId = taskStackIds.value[taskStackIds.value.length - 1]
  if (!activeBoard.value) return null
  for (const col of activeBoard.value.columns) {
    const t = col.tasks.find((t: any) => t.id === currentId)
    if (t) return t
  }
  return null
})
const targetColumnId = ref<string | null>(null)
const isSidebarOpen = ref(false)
const showLocalSearch = ref(false)
const localSearchQuery = ref('')

const handleLocalSearch = (direction: 'forward' | 'backward' = 'forward', isInput = false) => {
  if (typeof window !== 'undefined' && localSearchQuery.value) {
    if (isInput) {
      const sel = window.getSelection()
      if (sel && sel.rangeCount > 0) {
        sel.collapseToStart()
      }
    }
    window.find(localSearchQuery.value, false, direction === 'backward', true, false, false, false)
  }
}

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
    ? `ws://${window.location.hostname}:8080/api/ws?workspace_id=${activeWorkspaceId.value}`
    : `${protocol}//${typeof window !== 'undefined' ? window.location.host : ''}/api/ws?workspace_id=${activeWorkspaceId.value}`
})
const { send, onlineUsers, activeMentions, status } = useKanbanWS(wsUrl, activeBoard, boards, taskStackIds, pages, activePage)

const currentLocationStr = computed(() => {
  if (currentModule.value === 'wiki' && activePageId.value) return `page:${activePageId.value}`
  if (currentModule.value === 'kanban' && activeBoardId.value) return `board:${activeBoardId.value}`
  return 'home'
})

watch([currentLocationStr, status], ([loc, s]) => {
  if (send && s === 'OPEN') {
    send(JSON.stringify({ type: 'USER_LOCATION', location: loc }))
  }
}, { immediate: true })


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
  } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') {
    e.preventDefault()
    if (currentModule.value === 'kanban') {
      const searchInput = document.querySelector('.filter-bar .search-input') as HTMLInputElement
      if (searchInput) {
        searchInput.focus()
        searchInput.select()
      }
    } else if (currentModule.value === 'wiki') {
      showLocalSearch.value = true
      nextTick(() => {
        const localInput = document.getElementById('local-search-input') as HTMLInputElement
        if (localInput) {
          localInput.focus()
          localInput.select()
        }
      })
    }
  }
}

const syncHash = () => {
  if (typeof window === 'undefined') return
  const hash = window.location.hash
  if (hash.startsWith('#wiki-')) {
    const pageId = hash.replace('#wiki-', '')
    const page = pages.value?.find((p: any) => p.id === pageId)
    if (page && (!activePage.value || activePage.value.id !== pageId)) {
      handleSelectPage(page)
    }
  } else if (hash.startsWith('#kanban-')) {
    const boardId = hash.replace('#kanban-', '')
    const board = boards.value?.find((b: any) => b.id === boardId)
    if (board && activeBoardId.value !== boardId) {
      handleSelectBoard(board)
    }
  } else if (hash === '#dashboard') {
    if (currentModule.value !== 'dashboard') handleSelectWorkspaceDashboard()
  }
}

let initialHashSynced = false
watch([pages, boards], () => {
  if (initialHashSynced) return
  if (pages.value && boards.value) {
    syncHash()
    initialHashSynced = true
  }
}, { immediate: true })

onMounted(() => { 
  window.addEventListener('keydown', handleKeydown) 
  window.addEventListener('hashchange', syncHash)
})
onUnmounted(() => { 
  window.removeEventListener('keydown', handleKeydown) 
  window.removeEventListener('hashchange', syncHash)
})

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
  taskStackIds.value = []
  targetColumnId.value = colId
  showTaskDetailModal.value = true
}

const handleOpenTask = (task: any) => {
  taskStackIds.value.push(task.id)
  showTaskDetailModal.value = true
}

const currentSubtasks = computed(() => {
  if (!editingTask.value || !activeBoard.value) return []
  const allTasks = activeBoard.value.columns.flatMap(c => c.tasks)
  return allTasks.filter(t => t.parent_id === editingTask.value!.id)
})

const closeTaskModal = () => {
  taskStackIds.value.pop()
  if (taskStackIds.value.length === 0) showTaskDetailModal.value = false
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
  if (typeof window !== 'undefined') window.location.hash = `#wiki-${page.id}`
}

const handleSelectBoard = (board: any) => {
  if (board) selectBoard(board.id)
  else selectBoard(null)
  currentModule.value = 'kanban'
  activePage.value = null
  isSidebarOpen.value = false
  if (typeof window !== 'undefined' && board) window.location.hash = `#kanban-${board.id}`
}

const handleSelectWorkspaceDashboard = () => {
  currentModule.value = 'dashboard'
  activeBoardId.value = null
  activePage.value = null
  isSidebarOpen.value = false
  if (typeof window !== 'undefined') window.location.hash = `#dashboard`
}

const handleSelectWorkspace = (id: string) => {
  if (typeof window !== 'undefined' && 'Notification' in window && Notification.permission === 'default') {
    Notification.requestPermission()
  }
  selectWorkspace(id)
  if (typeof window !== 'undefined' && (!window.location.hash || window.location.hash === '#dashboard')) {
    handleSelectWorkspaceDashboard()
  }
}

const handleCreatePage = async () => {
  if (activeWorkspaceId.value) {
    const page = await createPage({ workspaceId: activeWorkspaceId.value })
    if (page) handleSelectPage(page)
  }
}

const handleMention = (userId: string) => {
  if (send) {
    send(JSON.stringify({
      type: 'USER_MENTIONED',
      target_user_id: userId,
      location: currentLocationStr.value,
      author_id: user.value?.user?.id || '',
    }))
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

const handleOpenTaskFromSearch = (result: any) => {
  if (result.parent_id && boards.value) {
    const board = boards.value.find((b: any) => b.id === result.parent_id)
    if (board) {
      handleSelectBoard(board)
      nextTick(() => {
        handleOpenTask({ id: result.id })
      })
    }
  }
}

const handleToggleTaskValidation = async (task: any) => {
  if (activeWorkspaceRole.value === 'viewer') return
  const wasCompleted = task.is_completed;
  
  queryClient.setQueryData(['boards', activeBoardId.value], (oldBoard: any) => {
    if (!oldBoard) return oldBoard
    const newBoard = JSON.parse(JSON.stringify(oldBoard))
    for (const col of newBoard.columns) {
      const t = col.tasks.find((t: any) => t.id === task.id)
      if (t) {
        if (t.is_per_person) {
          const hasValidated = t.validations?.includes(user.value?.user?.id)
          if (hasValidated) {
            t.validations = (t.validations || []).filter((id: string) => id !== user.value?.user?.id)
          } else {
            t.validations = [...(t.validations || []), user.value?.user?.id]
          }
          
          if (t.require_all_validations !== false) { // Default true
            if (!t.assignees || t.assignees.length === 0) {
              t.is_completed = t.validations.length > 0
            } else {
              t.is_completed = t.assignees.every((id: string) => t.validations.includes(id))
            }
          } else {
            t.is_completed = t.validations.length > 0
          }
        } else {
          t.is_completed = !t.is_completed
        }
      }
    }
    return newBoard
  })

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
    
    queryClient.setQueryData(['boards', activeBoardId.value], (oldBoard: any) => {
      if (!oldBoard) return oldBoard
      const newBoard = JSON.parse(JSON.stringify(oldBoard))
      for (const col of newBoard.columns) {
        const t = col.tasks.find((t: any) => t.id === task.id)
        if (t) {
          if (action === 'add') {
            t.labels = [...(t.labels || []), label]
          } else {
            t.labels = (t.labels || []).filter((l: any) => l.id !== label.id)
          }
        }
      }
      return newBoard
    })

    await toggleTaskLabel({ taskId: task.id, labelId: label.id, action })
  }
}

const handleToggleTaskAssignee = async (task: Task | null, userId: string) => {
  if (task) {
    const action = task.assignees?.includes(userId) ? 'remove' : 'add'
    
    queryClient.setQueryData(['boards', activeBoardId.value], (oldBoard: any) => {
      if (!oldBoard) return oldBoard
      const newBoard = JSON.parse(JSON.stringify(oldBoard))
      for (const col of newBoard.columns) {
        const t = col.tasks.find((t: any) => t.id === task.id)
        if (t) {
          if (action === 'add') {
            t.assignees = [...(t.assignees || []), userId]
          } else {
            t.assignees = (t.assignees || []).filter((id: string) => id !== userId)
          }
        }
      }
      return newBoard
    })

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
      @open-task-from-search="handleOpenTaskFromSearch"
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
              <div style="margin-left: auto; display: flex; align-items: center; gap: 8px;" id="wiki-header-actions">
              </div>
            </div>
          </template>
        </div>
        <div v-if="!activeBoard && (!activeWorkspaceId || currentModule === 'kanban' || currentModule === 'home')" style="display: flex; align-items: center; gap: 12px;">
          <div v-if="activeWorkspaceId && currentModule === 'kanban'">
            <h1>Sélectionnez un tableau</h1>
            <p class="subtitle">Créez ou sélectionnez un tableau Kanban dans la barre latérale.</p>
          </div>
          <div v-else-if="!activeWorkspaceId">
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
          :workspace-members="workspaceMembers || []"
          @update-title="(t) => { activePage.title = t; /* Note: could trigger query invalidation if needed */ }"
          @mention="handleMention"
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

    <PresenceDock 
      v-if="activeWorkspaceId"
      :users="onlineUsers || []" 
      :current-location="currentLocationStr" 
      :workspace-members="workspaceMembers || []"
      :pages="pages || []"
      :boards="boards || []"
      :active-mentions="activeMentions"
      @clear-mention="(id) => activeMentions = activeMentions.filter(m => m.id !== id)"
    />
    
    <!-- Offline Indicator -->
    <transition name="fade">
      <div v-if="!isOnline" class="offline-indicator">
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M8.5 16.5a5 5 0 0 1 7 0"/><path d="M11 11.5a8.5 8.5 0 0 1 7 0"/><path d="M3 3l18 18"/></svg>
        <span>Vous êtes hors ligne. Vos modifications locales seront synchronisées au retour de la connexion.</span>
      </div>
    </transition>

    <!-- Local Search UI for Wiki -->
    <transition name="fade">
      <div v-if="showLocalSearch" class="local-search-widget">
        <div class="local-search-inner">
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" class="local-search-icon"><path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"/></svg>
          <input 
            id="local-search-input"
            type="text" 
            v-model="localSearchQuery" 
            @input="handleLocalSearch('forward', true)"
            @keydown.enter="handleLocalSearch('forward')"
            @keydown.shift.enter="handleLocalSearch('backward')"
            @keydown.esc="showLocalSearch = false"
            placeholder="Rechercher dans la page..." 
            class="local-search-input"
          />
          <button @click="handleLocalSearch('backward')" class="local-search-btn" title="Précédent (Shift+Enter)">↑</button>
          <button @click="handleLocalSearch('forward')" class="local-search-btn" title="Suivant (Enter)">↓</button>
          <div style="width: 1px; height: 16px; background: var(--border-color); margin: 0 4px;"></div>
          <button @click="showLocalSearch = false" class="local-search-btn" title="Fermer (Esc)">✕</button>
        </div>
      </div>
    </transition>

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
:global(body) {
  -webkit-font-smoothing: antialiased;
}

.loading-screen { display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100vh; color: var(--text-color-3); }
.spinner { width: 20px; height: 20px; border: 2px solid var(--border-color); border-radius: 50%; border-top-color: var(--primary-color); animation: spin 0.6s cubic-bezier(0.5, 0, 0.5, 1) infinite; margin-bottom: 1rem; }
@keyframes spin { to { transform: rotate(360deg); } }

.app-layout { display: flex; height: 100vh; overflow: hidden; position: relative; }
.main-content { flex: 1; display: flex; flex-direction: column; overflow: hidden; background-color: var(--bg-color-main); }
.main-header { padding: 12px 20px 10px; border-bottom: 1px solid var(--border-color); display: flex; flex-direction: column; }
.main-header h1 { margin: 0; font-size: 0.9375rem; font-weight: 600; letter-spacing: -0.02em; color: var(--text-color); }
.subtitle { margin: 0; color: var(--text-color-3); font-size: 0.8125rem; }
.content-body { flex: 1; display: flex; flex-direction: column; overflow: hidden; }

.mobile-menu-btn { display: none; background: none; border: none; cursor: pointer; color: var(--text-color-2); padding: 4px; border-radius: var(--radius-sm); }
.mobile-menu-btn:hover { background: var(--bg-color-hover); color: var(--text-color); }
.mobile-overlay { display: none; position: fixed; inset: 0; background: rgba(0,0,0,0.5); z-index: 40; backdrop-filter: blur(4px); }

@media (max-width: 768px) {
  .mobile-menu-btn { display: flex; align-items: center; justify-content: center; }
  .mobile-overlay { display: block; }
  .app-sidebar { position: fixed; left: 0; top: 0; bottom: 0; z-index: 50; transform: translateX(-100%); transition: transform 0.25s cubic-bezier(0.25, 0.1, 0.25, 1); box-shadow: var(--shadow-lg); }
  .app-sidebar.is-open { transform: translateX(0); }
  .view-toggle { display: none; }
  .main-header { padding: 10px 16px; }
}

.icon-btn { background: none; border: none; color: var(--text-color-3); cursor: pointer; padding: 2px 4px; border-radius: var(--radius-xs); display: flex; align-items: center; justify-content: center; font-size: 0.8125rem; transition: all var(--transition-fast); }
.icon-btn:hover { background: var(--bg-color-hover); color: var(--text-color); }

.view-toggle { display: flex; background: var(--bg-color-2); border-radius: var(--radius-sm); border: 1px solid var(--border-color); padding: 2px; gap: 2px; }
.view-toggle button { background: transparent; border: none; padding: 3px 10px; border-radius: var(--radius-xs); color: var(--text-color-3); font-size: 0.75rem; font-weight: 500; cursor: pointer; transition: all var(--transition-fast); }
.view-toggle button:hover { color: var(--text-color-2); }
.view-toggle button.active { background: var(--bg-color-hover); color: var(--text-color); }

.secondary-btn { padding: 4px 12px; border-radius: var(--radius-sm); border: 1px solid var(--border-color); cursor: pointer; background: transparent; color: var(--text-color-2); font-weight: 500; font-size: 0.8125rem; transition: all var(--transition-fast); }
.secondary-btn:hover { background: var(--bg-color-hover); color: var(--text-color); }

.offline-indicator {
  position: fixed;
  bottom: 24px;
  left: 50%;
  transform: translateX(-50%);
  background: var(--warning-color);
  color: #fff;
  padding: 10px 20px;
  border-radius: var(--radius-lg);
  display: flex;
  align-items: center;
  gap: 10px;
  box-shadow: var(--shadow-float);
  z-index: 9999;
  font-weight: 500;
  font-size: 0.8125rem;
}
.fade-enter-active, .fade-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}
.fade-enter-from, .fade-leave-to {
  opacity: 0;
  transform: translate(-50%, 12px);
}

.local-search-widget {
  position: absolute;
  top: 16px;
  right: 16px;
  z-index: 2000;
  background: var(--bg-color-modal);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-float);
  padding: 6px 12px;
}
.local-search-inner {
  display: flex;
  align-items: center;
  gap: 6px;
}
.local-search-icon {
  color: var(--text-color-3);
}
.local-search-input {
  background: transparent;
  border: none;
  outline: none;
  color: var(--text-color);
  font-size: 0.8125rem;
  width: 180px;
}
.local-search-input::placeholder {
  color: var(--text-color-3);
}
.local-search-btn {
  background: transparent;
  border: none;
  color: var(--text-color-2);
  cursor: pointer;
  padding: 4px;
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.75rem;
  font-weight: bold;
  transition: all var(--transition-fast);
}
.local-search-btn:hover {
  background: var(--bg-color-hover);
  color: var(--text-color);
}
</style>
