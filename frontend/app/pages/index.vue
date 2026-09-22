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
const editingTask = ref<any>(null)
const editTaskTitle = ref('')
const editTaskDesc = ref('')

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

// Load boards when workspace changes
watch(activeWorkspaceId, async (wsId) => {
  if (wsId) {
    boards.value = await $fetch(`/api/workspaces/${wsId}/boards`)
    if (boards.value.length > 0) selectBoard(boards.value[0])
    else activeBoard.value = null
  }
})

function selectWorkspace(id: string) { activeWorkspaceId.value = id }

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
  targetColumnId.value = colId
  showTaskModal.value = true
}

async function createTask() {
  if (!newTaskTitle.value.trim() || !targetColumnId.value) return
  await $fetch(`/api/boards/columns/${targetColumnId.value}/tasks`, { method: 'POST', body: { title: newTaskTitle.value, description: "" } })
  
  // Le WebSocket (TASK_CREATED) l'ajoutera dans l'interface
  
  showTaskModal.value = false
  newTaskTitle.value = ''
  targetColumnId.value = null
}

function openTaskDetail(task: any) {
  editingTask.value = task
  editTaskTitle.value = task.title
  editTaskDesc.value = task.description || ''
  showTaskDetailModal.value = true
}

async function updateTask() {
  if (!editTaskTitle.value.trim() || !editingTask.value) return
  await $fetch(`/api/boards/tasks/${editingTask.value.id}`, { method: 'PUT', body: { title: editTaskTitle.value, description: editTaskDesc.value } })
  showTaskDetailModal.value = false
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
    // WebSocket gère la mise à jour
  } else if (renameType.value === 'column') {
    await $fetch(`/api/boards/columns/${id}`, { method: 'PUT', body: { name } })
    // WebSocket gère la mise à jour
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
    
    // Calculate new position
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
    
    // Optimistic update
    task.position = newPosition
    task.column_id = toColumnId
    
    await $fetch(`/api/boards/tasks/${task.id}/move`, { method: 'PUT', body: { column_id: toColumnId, position: newPosition } })
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
        // Remove from old column
        for (const col of activeBoard.value.columns) {
          const idx = col.tasks.findIndex((t: any) => t.id === event.task.id)
          if (idx !== -1) col.tasks.splice(idx, 1)
        }
        // Add to new column
        const col = activeBoard.value.columns.find((c: any) => c.id === event.task.column_id)
        if (col) {
          col.tasks.push(event.task)
          col.tasks.sort((a: any, b: any) => a.position - b.position)
        }
      } else if (event.type === 'TASK_UPDATED') {
        const col = activeBoard.value.columns.find((c: any) => c.id === event.task.column_id)
        if (col) {
          const idx = col.tasks.findIndex((t: any) => t.id === event.task.id)
          if (idx !== -1) col.tasks[idx] = event.task
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
      } else if (event.type === 'COLUMN_DELETED') {
        const idx = activeBoard.value.columns.findIndex((c: any) => c.id === event.column_id)
        if (idx !== -1) activeBoard.value.columns.splice(idx, 1)
      }
    } catch(e) {}
  }
})

// Gestion de la touche Echap pour fermer les modales
function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    showCreateModal.value = false
    showBoardModal.value = false
    showColumnModal.value = false
    showTaskModal.value = false
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

// Protège la route
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
          <div v-for="col in activeBoard.columns" :key="col.id" class="kanban-column">
            <div class="column-header">
              <h3 style="flex: 1; cursor: pointer" @dblclick="activeWorkspaceRole !== 'member' ? openRenameModal('column', col.id, col.name) : null" :title="activeWorkspaceRole !== 'member' ? 'Double-clic pour renommer' : ''">{{ col.name }}</h3>
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
                <h4>{{ task.title }}</h4>
              </div>
            </VueDraggableNext>
            
            <button class="add-task-btn" @click="openTaskModal(col.id)">+ Nouvelle tâche</button>
          </div>
          
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
    
    <!-- Modale de Création Tâche -->
    <div v-if="showTaskModal" class="modal-overlay" @click.self="showTaskModal = false">
      <div class="modal">
        <h2>Nouvelle tâche</h2>
        <form @submit.prevent="createTask">
          <input v-focus v-model="newTaskTitle" type="text" placeholder="Titre de la tâche" class="modern-input" required>
          <div class="modal-actions">
            <button type="button" class="secondary-btn" @click="showTaskModal = false">Annuler</button>
            <button type="submit" class="primary-btn">Créer</button>
          </div>
        </form>
      </div>
    </div>
    
    <!-- Modale de Détail/Edition de Tâche -->
    <div v-if="showTaskDetailModal" class="modal-overlay" @click.self="showTaskDetailModal = false">
      <div class="modal" style="width: 500px">
        <h2>Détails de la tâche</h2>
        <form @submit.prevent="updateTask">
          <input v-focus v-model="editTaskTitle" type="text" placeholder="Titre de la tâche" class="modern-input" required style="margin-bottom: 1rem">
          <textarea v-model="editTaskDesc" placeholder="Description (facultatif)" class="modern-input" rows="4"></textarea>
          <div class="modal-actions" style="justify-content: space-between; margin-top: 1.5rem">
            <button type="button" class="secondary-btn" style="color: #f87171" @click="deleteTask">Supprimer</button>
            <div style="display: flex; gap: 0.5rem">
              <button type="button" class="secondary-btn" @click="showTaskDetailModal = false">Fermer</button>
              <button type="submit" class="primary-btn">Enregistrer</button>
            </div>
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
</style>
