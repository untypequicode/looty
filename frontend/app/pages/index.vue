<script setup lang="ts">
import { useWebSocket } from '@vueuse/core'

const { user, isLoading } = useAuth()
const messageInput = ref('')

// WebSocket (V2)
const { status, data, send } = useWebSocket('ws://localhost:8081/api/ws', {
  autoReconnect: true,
  immediate: process.client
})

const messages = ref<string[]>([])
watch(data, (newData) => {
  if (newData) {
    messages.value.push(newData)
  }
})

function sendMessage() {
  if (messageInput.value.trim()) {
    send(messageInput.value)
    messageInput.value = ''
  }
}

// Workspaces (V3)
const { data: workspacesData, pending: isLoadingWorkspaces, refresh: refreshWorkspaces } = useFetch('/api/workspaces', {
  lazy: true,
  server: false 
})

const isCreatingWorkspace = ref(false)
const newWorkspaceName = ref('')
const showCreateModal = ref(false)
const activeWorkspaceId = ref<string | null>(null)

async function createWorkspace() {
  if (!newWorkspaceName.value.trim()) return;
  
  isCreatingWorkspace.value = true
  try {
    await $fetch('/api/workspaces', {
      method: 'POST',
      body: { name: newWorkspaceName.value }
    })
    newWorkspaceName.value = ''
    showCreateModal.value = false
    await refreshWorkspaces()
  } catch(e) {
    console.error(e)
  } finally {
    isCreatingWorkspace.value = false
  }
}

watch(workspacesData, (newVal: any) => {
  if (newVal?.workspaces?.length > 0 && !activeWorkspaceId.value) {
    activeWorkspaceId.value = newVal.workspaces[0].id
  }
})

function selectWorkspace(id: string) {
  activeWorkspaceId.value = id
}

// Protège la route
watchEffect(() => {
  if (!isLoading.value && !user.value?.authenticated) {
    navigateTo('/login')
  }
})
</script>

<template>
  <div v-if="isLoading" class="loading-screen">
    <div class="spinner"></div>
    <p>Chargement de Looty...</p>
  </div>
  
  <div v-else-if="user?.authenticated" class="app-layout">
    <!-- Sidebar -->
    <aside class="sidebar glass-panel">
      <div class="sidebar-header">
        <div class="logo">Looty</div>
      </div>
      
      <div class="sidebar-content">
        <div class="section-title">
          <span>Espaces de Travail</span>
          <button @click="showCreateModal = true" class="icon-btn" title="Créer un espace">
            <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none"><line x1="12" y1="5" x2="12" y2="19"></line><line x1="5" y1="12" x2="19" y2="12"></line></svg>
          </button>
        </div>
        
        <ul class="workspace-list">
          <li v-if="isLoadingWorkspaces" class="loading-item">Chargement...</li>
          <li v-else-if="!workspacesData?.workspaces?.length" class="empty-item">Aucun espace</li>
          
          <li 
            v-for="ws in workspacesData?.workspaces" 
            :key="ws.id"
            :class="['workspace-item', { active: activeWorkspaceId === ws.id }]"
            @click="selectWorkspace(ws.id)"
          >
            <div class="ws-avatar">{{ ws.name.charAt(0).toUpperCase() }}</div>
            <span class="ws-name">{{ ws.name }}</span>
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
        <a href="/api/auth/logout" class="logout-btn">
          <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"></path><polyline points="16 17 21 12 16 7"></polyline><line x1="21" y1="12" x2="9" y2="12"></line></svg>
          Déconnexion
        </a>
      </div>
    </aside>

    <!-- Main Content -->
    <main class="main-content">
      <header class="main-header glass-panel">
        <div v-if="activeWorkspaceId">
          <h1>{{ workspacesData?.workspaces.find((w: any) => w.id === activeWorkspaceId)?.name || 'Espace' }}</h1>
          <p class="subtitle">Tableau de bord de l'espace</p>
        </div>
        <div v-else>
          <h1>Bienvenue sur Looty</h1>
          <p class="subtitle">Sélectionnez ou créez un espace de travail pour commencer.</p>
        </div>
      </header>

      <div class="content-body">
        <!-- WebSocket Card -->
        <div class="card glass-panel" v-if="activeWorkspaceId">
          <div class="card-header">
            <h3>Messagerie en direct (V2)</h3>
            <span :class="['status-badge', status.toLowerCase()]">{{ status }}</span>
          </div>
          
          <div class="chat-box">
            <div class="messages-list">
              <div v-for="(msg, i) in messages" :key="i" class="message-item">
                {{ msg }}
              </div>
              <div v-if="messages.length === 0" class="empty-messages">
                Aucun message. Dites bonjour !
              </div>
            </div>
            
            <form @submit.prevent="sendMessage" class="message-form">
              <input v-model="messageInput" type="text" placeholder="Envoyer un message au serveur..." class="modern-input">
              <button type="submit" class="primary-btn">
                Envoyer
              </button>
            </form>
          </div>
        </div>
        
        <!-- Placeholder for V4 -->
        <div class="card empty-state glass-panel" v-if="activeWorkspaceId">
          <div class="empty-icon">📋</div>
          <h3>Tableaux Kanban (À venir)</h3>
          <p>La gestion des tableaux et des tâches sera implémentée dans la Version 4.</p>
        </div>
      </div>
    </main>

    <!-- Modale de Création -->
    <div v-if="showCreateModal" class="modal-overlay" @click.self="showCreateModal = false">
      <div class="modal glass-panel">
        <h2>Nouvel espace de travail</h2>
        <p>Donnez un nom à votre projet pour y inviter votre équipe.</p>
        <form @submit.prevent="createWorkspace">
          <input 
            v-model="newWorkspaceName" 
            type="text" 
            placeholder="Ex: Projet Refonte UI" 
            class="modern-input" 
            required 
            autofocus
          >
          <div class="modal-actions">
            <button type="button" class="secondary-btn" @click="showCreateModal = false">Annuler</button>
            <button type="submit" class="primary-btn" :disabled="isCreatingWorkspace">
              {{ isCreatingWorkspace ? 'Création...' : 'Créer l\'espace' }}
            </button>
          </div>
        </form>
      </div>
    </div>
  </div>
</template>

<style scoped>
@import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600&display=swap');

:global(body) {
  margin: 0;
  padding: 0;
  font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
  background-color: #0e0e11;
  color: #eeeeee;
  min-height: 100vh;
  -webkit-font-smoothing: antialiased;
}

.loading-screen {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100vh;
  color: #888;
}
.spinner {
  width: 24px; height: 24px;
  border: 2px solid #333;
  border-radius: 50%;
  border-top-color: #eee;
  animation: spin 1s linear infinite;
  margin-bottom: 1rem;
}
@keyframes spin { to { transform: rotate(360deg); } }

.app-layout {
  display: flex;
  height: 100vh;
  overflow: hidden;
}

/* Sidebar */
.sidebar {
  width: 260px;
  background-color: #17181c;
  display: flex;
  flex-direction: column;
  border-right: 1px solid #27282b;
  z-index: 10;
}

.sidebar-header {
  padding: 1.25rem 1.25rem;
}
.logo {
  font-size: 1rem;
  font-weight: 600;
  color: #fff;
  letter-spacing: -0.2px;
}

.sidebar-content {
  flex: 1;
  padding: 0 1rem;
  overflow-y: auto;
}
.section-title {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 0.75rem;
  color: #8a8f98;
  margin-bottom: 0.5rem;
  margin-top: 1rem;
  font-weight: 500;
}
.icon-btn {
  background: none; border: none; color: #8a8f98; cursor: pointer;
  padding: 4px; border-radius: 4px; display: flex; align-items: center; justify-content: center;
}
.icon-btn:hover { background: #27282b; color: #eee; }

.workspace-list { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 2px; }
.loading-item, .empty-item { color: #8a8f98; font-size: 0.85rem; padding: 0.5rem; }
.workspace-item {
  display: flex; align-items: center; gap: 10px;
  padding: 6px 8px; border-radius: 4px; cursor: pointer;
  color: #8a8f98; font-size: 0.875rem; font-weight: 500;
}
.workspace-item:hover { background: #27282b; color: #eee; }
.workspace-item.active { background: #27282b; color: #fff; }
.ws-avatar {
  width: 18px; height: 18px; border-radius: 4px;
  background: #333; display: flex; align-items: center; justify-content: center;
  font-size: 0.65rem; color: #fff;
}
.workspace-item.active .ws-avatar { background: #5e6ad2; }

.sidebar-footer {
  padding: 1rem;
  border-top: 1px solid #27282b;
  display: flex; flex-direction: column; gap: 0.75rem;
}
.user-profile { display: flex; align-items: center; gap: 10px; }
.user-avatar {
  width: 24px; height: 24px; border-radius: 50%;
  background: #333; display: flex; align-items: center; justify-content: center;
  font-size: 0.75rem; color: #fff;
}
.user-info p { margin: 0; }
.user-name { font-weight: 500; font-size: 0.875rem; color: #eee; }
.user-email { font-size: 0.75rem; color: #8a8f98; }
.logout-btn {
  display: flex; align-items: center; gap: 8px;
  text-decoration: none; color: #8a8f98; font-size: 0.85rem;
}
.logout-btn:hover { color: #f87171; }

/* Main Content */
.main-content {
  flex: 1; display: flex; flex-direction: column;
  overflow-y: auto; background-color: #0e0e11;
}

.main-header {
  padding: 2.5rem 3rem 1.5rem 3rem;
  border-bottom: 1px solid #27282b;
}
.main-header h1 { margin: 0 0 0.5rem 0; font-size: 1.5rem; font-weight: 600; letter-spacing: -0.02em; }
.subtitle { margin: 0; color: #8a8f98; font-size: 0.9rem; }

.content-body {
  padding: 2rem 3rem;
  display: grid; grid-template-columns: minmax(0, 1fr); gap: 2rem;
  max-width: 1000px;
}

.card {
  border: 1px solid #27282b; border-radius: 6px;
  background: #17181c; display: flex; flex-direction: column;
}
.card-header {
  padding: 1rem 1.5rem; border-bottom: 1px solid #27282b;
  display: flex; justify-content: space-between; align-items: center;
}
.card-header h3 { margin: 0; font-size: 1rem; font-weight: 500; }

.status-badge {
  font-size: 0.7rem; padding: 2px 8px; border-radius: 4px; font-weight: 500;
}
.status-badge.open { background: #164627; color: #6ee7b7; border: 1px solid #1f6438; }
.status-badge.connecting { background: #5c4413; color: #fde047; border: 1px solid #785a1a; }
.status-badge.closed { background: #5c1a1a; color: #fca5a5; border: 1px solid #7a2323; }

.chat-box { padding: 1.5rem; display: flex; flex-direction: column; gap: 1rem; flex: 1; min-height: 250px; }
.messages-list {
  flex: 1; border-radius: 4px; background: #0e0e11;
  padding: 1rem; overflow-y: auto; display: flex; flex-direction: column; gap: 8px;
  border: 1px solid #27282b; max-height: 300px;
}
.empty-messages { margin: auto; color: #555; font-size: 0.85rem; }
.message-item {
  background: #17181c; padding: 6px 12px; border-radius: 4px;
  font-size: 0.85rem; color: #ccc; border: 1px solid #27282b;
}

.message-form { display: flex; gap: 10px; }
.modern-input {
  flex: 1; padding: 8px 12px; border-radius: 4px;
  background: #0e0e11; border: 1px solid #27282b;
  color: #eee; font-family: inherit; font-size: 0.875rem;
}
.modern-input:focus { border-color: #5e6ad2; outline: none; }

.primary-btn {
  padding: 8px 16px; border-radius: 4px; border: none; cursor: pointer;
  background: #fff; color: #000; font-weight: 500; font-size: 0.875rem;
}
.primary-btn:hover { background: #e0e0e0; }
.primary-btn:disabled { opacity: 0.5; cursor: not-allowed; }

.secondary-btn {
  padding: 8px 16px; border-radius: 4px; border: 1px solid #27282b; cursor: pointer;
  background: #17181c; color: #eee; font-weight: 500; font-size: 0.875rem;
}
.secondary-btn:hover { background: #27282b; }

.empty-state { align-items: center; justify-content: center; padding: 3rem 2rem; text-align: center; }
.empty-icon { font-size: 2rem; margin-bottom: 0.5rem; color: #555; }
.empty-state h3 { margin: 0 0 0.5rem 0; font-size: 1rem; font-weight: 500; color: #eee; }
.empty-state p { margin: 0; color: #8a8f98; font-size: 0.85rem; }

/* Modal */
.modal-overlay {
  position: fixed; inset: 0; background: rgba(0,0,0,0.8);
  display: flex; align-items: center; justify-content: center; z-index: 100;
}
.modal {
  width: 100%; max-width: 400px; padding: 1.5rem; border-radius: 6px;
  background: #17181c; border: 1px solid #27282b;
  box-shadow: 0 10px 30px rgba(0,0,0,0.5);
}
.modal h2 { margin: 0 0 0.5rem 0; font-size: 1.1rem; font-weight: 500; }
.modal p { color: #8a8f98; margin: 0 0 1.2rem 0; font-size: 0.85rem; }
.modal-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 1.5rem; }
</style>
