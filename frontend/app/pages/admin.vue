<script setup lang="ts">
import { ref, onMounted, watchEffect } from 'vue'
import { useAuth } from '../composables/useAuth'

const { user, isLoading } = useAuth()
const currentTab = ref('settings')

const users = ref<any[]>([])
const providers = ref<any[]>([])
const workspaces = ref<any[]>([])
const stats = ref<any>(null)

const settingsForm = ref({
  app_name: 'Looty',
  favicon_url: '',
  allow_public_signups: true,
  umami_website_id: '',
  umami_script_url: ''
})

const showProviderModal = ref(false)
const editingProvider = ref<any>(null)
const providerForm = ref({
  name: '',
  issuer_url: '',
  client_id: '',
  client_secret: '',
  is_active: true
})

onMounted(() => {
  if (user.value?.authenticated) {
    if (!user.value.user.is_super_admin) {
      navigateTo('/')
    } else {
      fetchAllData()
    }
  }
})

watchEffect(() => {
  if (!isLoading.value && !user.value?.authenticated) {
    navigateTo('/login')
  } else if (!isLoading.value && user.value?.authenticated && !user.value.user.is_super_admin) {
    navigateTo('/')
  }
})

async function fetchAllData() {
  fetchSettings()
  fetchUsers()
  fetchProviders()
  fetchWorkspaces()
  fetchStats()
}

async function fetchSettings() {
  try {
    const data: any = await $fetch('/api/admin/settings')
    settingsForm.value = {
      app_name: data.app_name || '',
      favicon_url: data.favicon_url || '',
      allow_public_signups: data.allow_public_signups ?? true,
      umami_website_id: data.umami_website_id || '',
      umami_script_url: data.umami_script_url || ''
    }
  } catch(e) {}
}

async function saveSettings() {
  try {
    await $fetch('/api/admin/settings', {
      method: 'PUT',
      body: settingsForm.value
    })
    alert('Paramètres sauvegardés avec succès !')
  } catch(e) {
    alert("Erreur lors de la sauvegarde")
  }
}

const isUploadingFavicon = ref(false)
const faviconInput = ref<HTMLInputElement | null>(null)

async function uploadFavicon(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0]
  if (!file) return
  isUploadingFavicon.value = true
  const formData = new FormData()
  formData.append('file', file)
  try {
    const res: any = await $fetch('/api/upload', {
      method: 'POST',
      body: formData
    })
    settingsForm.value.favicon_url = res.url
  } catch(e) {
    alert("Erreur lors de l'upload")
  } finally {
    isUploadingFavicon.value = false
  }
}

async function fetchUsers() {
  try { users.value = await $fetch('/api/admin/users') } catch(e) {}
}

async function fetchProviders() {
  try { providers.value = await $fetch('/api/admin/providers') } catch(e) {}
}

async function fetchWorkspaces() {
  try { workspaces.value = await $fetch('/api/admin/workspaces') } catch(e) {}
}

async function fetchStats() {
  try { stats.value = await $fetch('/api/admin/stats') } catch(e) {}
}

async function toggleUserRole(u: any) {
  if (u.id === user.value?.user?.id) { alert("Impossible de modifier son propre rôle."); return }
  if (confirm(`Changer le rôle de ${u.name} ?`)) {
    try {
      await $fetch(`/api/admin/users/${u.id}/role`, { method: 'PUT', body: { is_super_admin: !u.is_super_admin } })
      fetchUsers()
    } catch(e) { alert("Erreur") }
  }
}

async function toggleSuspendUser(u: any) {
  if (u.id === user.value?.user?.id) { alert("Impossible de suspendre son propre compte."); return }
  const action = u.is_suspended ? 'Dé-suspendre' : 'Suspendre'
  if (confirm(`${action} ${u.name} ?`)) {
    try {
      await $fetch(`/api/admin/users/${u.id}/suspend`, { method: 'PUT', body: { is_suspended: !u.is_suspended } })
      fetchUsers()
    } catch(e) { alert("Erreur") }
  }
}

async function deleteUser(id: string, name: string) {
  if (id === user.value?.user?.id) { alert("Impossible de supprimer son propre compte."); return }
  if (confirm(`Supprimer DÉFINITIVEMENT l'utilisateur ${name} et toutes ses données ?`)) {
    try {
      await $fetch(`/api/admin/users/${id}`, { method: 'DELETE' })
      fetchUsers()
    } catch(e) { alert("Erreur") }
  }
}

async function deleteWorkspace(id: string, name: string) {
  if (confirm(`Supprimer DÉFINITIVEMENT l'espace "${name}" et toutes ses pages ?`)) {
    try {
      await $fetch(`/api/admin/workspaces/${id}`, { method: 'DELETE' })
      fetchWorkspaces()
      fetchStats()
    } catch(e) { alert("Erreur") }
  }
}

// OAuth Provider Modal functions
function openCreateProvider() {
  editingProvider.value = null
  providerForm.value = { name: '', issuer_url: '', client_id: '', client_secret: '', is_active: true }
  showProviderModal.value = true
}

function openEditProvider(p: any) {
  editingProvider.value = p
  providerForm.value = { ...p }
  showProviderModal.value = true
}

async function saveProvider() {
  try {
    if (editingProvider.value) {
      await $fetch(`/api/admin/providers/${editingProvider.value.id}`, { method: 'PUT', body: providerForm.value })
    } else {
      await $fetch('/api/admin/providers', { method: 'POST', body: providerForm.value })
    }
    showProviderModal.value = false
    fetchProviders()
  } catch(e) { alert("Erreur") }
}

async function deleteProvider(id: string) {
  if (confirm("Supprimer ce fournisseur OAuth ?")) {
    try { await $fetch(`/api/admin/providers/${id}`, { method: 'DELETE' }); fetchProviders() } catch(e) {}
  }
}

function formatBytes(bytes: number) {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}
</script>

<template>
  <div class="admin-layout" v-if="user?.user?.is_super_admin">
    <aside class="admin-sidebar">
      <h2>Looty Admin</h2>
      <nav>
        <button :class="{ active: currentTab === 'settings' }" @click="currentTab = 'settings'">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" style="margin-right: 6px; vertical-align: -3px;"><path stroke-linecap="round" stroke-linejoin="round" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" /><path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" /></svg>
          Paramètres
        </button>
        <button :class="{ active: currentTab === 'users' }" @click="currentTab = 'users'">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" style="margin-right: 6px; vertical-align: -3px;"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z" /></svg>
          Utilisateurs
        </button>
        <button :class="{ active: currentTab === 'workspaces' }" @click="currentTab = 'workspaces'">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" style="margin-right: 6px; vertical-align: -3px;"><path stroke-linecap="round" stroke-linejoin="round" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" /></svg>
          Espaces (Workspaces)
        </button>
        <button :class="{ active: currentTab === 'stats' }" @click="currentTab = 'stats'">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" style="margin-right: 6px; vertical-align: -3px;"><path stroke-linecap="round" stroke-linejoin="round" d="M7 12l3-3 3 3 4-4M8 21l4-4 4 4M3 4h18M4 4h16v12a1 1 0 01-1 1H5a1 1 0 01-1-1V4z" /></svg>
          Stats & Stockage
        </button>
        <button :class="{ active: currentTab === 'providers' }" @click="currentTab = 'providers'">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" style="margin-right: 6px; vertical-align: -3px;"><path stroke-linecap="round" stroke-linejoin="round" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" /></svg>
          OAuth Providers
        </button>
      </nav>
      <div style="margin-top: auto;">
        <NuxtLink to="/" style="color: var(--text-color-3); text-decoration: none; font-size: 0.85rem;">&larr; Retour à l'app</NuxtLink>
      </div>
    </aside>

    <main class="admin-content">
      
      <!-- PARAMÈTRES -->
      <div v-if="currentTab === 'settings'" class="tab-content">
        <h1>Paramètres Globaux</h1>
        <p class="subtitle">Configuration générale de l'instance Looty.</p>
        
        <form @submit.prevent="saveSettings" class="settings-form">
          <div class="form-group">
            <label>Nom de l'application</label>
            <input v-model="settingsForm.app_name" type="text" class="modern-input" placeholder="Looty">
          </div>
          
          <div class="form-group">
            <label>Favicon</label>
            <div style="display: flex; gap: 10px; align-items: center;">
              <img v-if="settingsForm.favicon_url" :src="settingsForm.favicon_url" style="width: 32px; height: 32px; object-fit: contain; border: 1px solid var(--border-color); padding: 2px; background: white;" />
              <input type="file" accept="image/*" @change="uploadFavicon" style="display: none;" ref="faviconInput">
              <button type="button" class="secondary-btn small" @click="faviconInput?.click()">{{ isUploadingFavicon ? 'Chargement...' : 'Uploader une icône' }}</button>
            </div>
            <p class="help-text" style="margin-top: 8px;">Ou via URL :</p>
            <input v-model="settingsForm.favicon_url" type="text" class="modern-input" placeholder="https://..." style="margin-top: 4px;">
          </div>

          <div class="form-group checkbox-group">
            <label>
              <input v-model="settingsForm.allow_public_signups" type="checkbox">
              Autoriser les nouvelles inscriptions (Public)
            </label>
            <p class="help-text">Si décoché, seuls les utilisateurs invités pourront se connecter.</p>
          </div>

          <hr>
          <h3>Statistiques Umami</h3>
          
          <div class="form-group">
            <label>Umami Website ID</label>
            <input v-model="settingsForm.umami_website_id" type="text" class="modern-input" placeholder="Ex: 8b4...-...">
          </div>

          <div class="form-group">
            <label>Umami Script URL</label>
            <input v-model="settingsForm.umami_script_url" type="text" class="modern-input" placeholder="https://analytics.tondomaine.com/script.js">
          </div>

          <button type="submit" class="primary-btn" style="margin-top: 1rem;">Enregistrer les paramètres</button>
        </form>
      </div>

      <!-- UTILISATEURS -->
      <div v-if="currentTab === 'users'" class="tab-content">
        <div class="header-row">
          <h1>Utilisateurs ({{ users.length }})</h1>
        </div>
        <table class="admin-table">
          <thead>
            <tr>
              <th>ID</th>
              <th>Nom</th>
              <th>Email</th>
              <th>Rôle Admin</th>
              <th>Statut</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="u in users" :key="u.id" :class="{ 'suspended-row': u.is_suspended }">
              <td><span style="font-family: monospace; font-size: 0.8em; color: var(--text-color-3)">{{ u.id.split('-')[0] }}...</span></td>
              <td>{{ u.name }}</td>
              <td>{{ u.email }}</td>
              <td>
                <button class="badge-btn" @click="toggleUserRole(u)" :class="{ 'badge-success': u.is_super_admin }">
                  {{ u.is_super_admin ? 'Oui' : 'Non' }}
                </button>
              </td>
              <td>
                <span v-if="u.is_suspended" class="status-badge danger">Suspendu</span>
                <span v-else class="status-badge success">Actif</span>
              </td>
              <td>
                <div class="action-buttons">
                  <button class="secondary-btn small" @click="toggleSuspendUser(u)">
                    {{ u.is_suspended ? 'Réactiver' : 'Suspendre' }}
                  </button>
                  <button class="danger-btn small" @click="deleteUser(u.id, u.name)">Supprimer</button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- ESPACES (WORKSPACES) -->
      <div v-if="currentTab === 'workspaces'" class="tab-content">
        <div class="header-row">
          <h1>Espaces de travail ({{ workspaces.length }})</h1>
        </div>
        <table class="admin-table">
          <thead>
            <tr>
              <th>Nom</th>
              <th>ID</th>
              <th>Membres</th>
              <th>Création</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="w in workspaces" :key="w.id">
              <td style="font-weight: bold;">{{ w.name }}</td>
              <td><span style="font-family: monospace; font-size: 0.8em; color: var(--text-color-3)">{{ w.id.split('-')[0] }}</span></td>
              <td>{{ w.members_count }}</td>
              <td>{{ new Date(w.created_at).toLocaleDateString() }}</td>
              <td>
                <button class="danger-btn small" @click="deleteWorkspace(w.id, w.name)">Supprimer</button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- STATS & STOCKAGE -->
      <div v-if="currentTab === 'stats'" class="tab-content">
        <h1>Statistiques & Stockage</h1>
        
        <div class="stats-grid" v-if="stats">
          <div class="stat-card">
            <div class="stat-value">{{ stats.total_users }}</div>
            <div class="stat-label">Utilisateurs inscrits</div>
          </div>
          <div class="stat-card">
            <div class="stat-value">{{ stats.total_workspaces }}</div>
            <div class="stat-label">Espaces de travail</div>
          </div>
          <div class="stat-card">
            <div class="stat-value">{{ stats.total_pages }}</div>
            <div class="stat-label">Pages créées</div>
          </div>
          <div class="stat-card">
            <div class="stat-value highlight">{{ formatBytes(stats.total_attachments_size) }}</div>
            <div class="stat-label">Espace Disque Utilisé (Uploads)</div>
          </div>
        </div>
      </div>

      <!-- OAUTH PROVIDERS -->
      <div v-if="currentTab === 'providers'" class="tab-content">
        <div class="header-row">
          <h1>Fournisseurs OAuth</h1>
          <button class="primary-btn" @click="openCreateProvider">+ Nouveau</button>
        </div>
        <table class="admin-table">
          <thead>
            <tr>
              <th>Nom</th>
              <th>Issuer URL</th>
              <th>Client ID</th>
              <th>Statut</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="p in providers" :key="p.id">
              <td style="font-weight: bold;">{{ p.name }}</td>
              <td>{{ p.issuer_url }}</td>
              <td><span style="font-family: monospace; font-size: 0.8em;">{{ p.client_id }}</span></td>
              <td>
                <span v-if="p.is_active" class="status-badge success">Actif</span>
                <span v-else class="status-badge">Inactif</span>
              </td>
              <td>
                <button class="icon-btn" @click="openEditProvider(p)">
                  <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z" /></svg>
                </button>
                <button class="icon-btn" @click="deleteProvider(p.id)">
                  <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" /></svg>
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

    </main>

    <!-- Provider Modal -->
    <div v-if="showProviderModal" class="modal-overlay" @click.self="showProviderModal = false">
      <div class="modal">
        <h2>{{ editingProvider ? 'Modifier Fournisseur' : 'Nouveau Fournisseur' }}</h2>
        <form @submit.prevent="saveProvider">
          <div style="margin-bottom: 12px;">
            <label>Nom (affiché sur le bouton)</label>
            <input v-model="providerForm.name" type="text" class="modern-input" required>
          </div>
          <div style="margin-bottom: 12px;">
            <label>Issuer URL (OIDC)</label>
            <input v-model="providerForm.issuer_url" type="url" class="modern-input" required>
          </div>
          <div style="margin-bottom: 12px;">
            <label>Client ID</label>
            <input v-model="providerForm.client_id" type="text" class="modern-input" required>
          </div>
          <div style="margin-bottom: 12px;">
            <label>Client Secret</label>
            <input v-model="providerForm.client_secret" type="password" class="modern-input" required>
          </div>
          <div style="margin-bottom: 12px;" class="checkbox-group">
            <label>
              <input v-model="providerForm.is_active" type="checkbox">
              Activer ce fournisseur
            </label>
          </div>
          
          <div class="modal-actions">
            <button type="button" class="secondary-btn" @click="showProviderModal = false">Annuler</button>
            <button type="submit" class="primary-btn">Sauvegarder</button>
          </div>
        </form>
      </div>
    </div>
  </div>
</template>

<style scoped>
.admin-layout { display: flex; height: 100vh; background: var(--bg-color-main); color: var(--text-color); font-family: 'Inter', sans-serif; }
.admin-sidebar { width: 260px; background: var(--bg-color-2); border-right: 1px solid var(--border-color); padding: 1.5rem; display: flex; flex-direction: column; }
.admin-sidebar h2 { margin: 0 0 2rem 0; font-size: 1.3rem; font-weight: 600; letter-spacing: -0.5px; }
.admin-sidebar nav { display: flex; flex-direction: column; gap: 6px; }
.admin-sidebar nav button { text-align: left; background: transparent; border: none; padding: 10px 14px; border-radius: 8px; color: var(--text-color-2); cursor: pointer; font-size: 0.95rem; font-weight: 500; transition: all 0.2s; }
.admin-sidebar nav button:hover { background: var(--bg-color-hover); color: var(--text-color); }
.admin-sidebar nav button.active { background: var(--border-color); color: var(--text-color); font-weight: 600; box-shadow: 0 1px 2px rgba(0,0,0,0.05); }

.admin-content { flex: 1; padding: 3rem 4rem; overflow-y: auto; }
.tab-content { max-width: 1000px; }
.tab-content h1 { margin: 0 0 0.5rem 0; font-size: 1.8rem; font-weight: 600; letter-spacing: -0.5px; }
.subtitle { color: var(--text-color-3); margin-top: 0; margin-bottom: 2rem; }

.header-row { display: flex; justify-content: space-between; align-items: center; margin-bottom: 2rem; }

/* Tables */
.admin-table { width: 100%; border-collapse: separate; border-spacing: 0; background: var(--bg-color-2); border: 1px solid var(--border-color); border-radius: 8px; overflow: hidden; }
.admin-table th { text-align: left; padding: 14px 16px; border-bottom: 1px solid var(--border-color); color: var(--text-color-3); font-weight: 600; font-size: 0.85rem; text-transform: uppercase; letter-spacing: 0.5px; background: var(--bg-color-modal); }
.admin-table td { padding: 14px 16px; border-bottom: 1px solid var(--border-color); font-size: 0.9rem; }
.admin-table tr:last-child td { border-bottom: none; }
.admin-table tr:hover td { background: var(--bg-color-hover); }
.suspended-row td { opacity: 0.6; }

/* Badges */
.status-badge { display: inline-block; padding: 4px 8px; border-radius: 4px; font-size: 0.75rem; font-weight: 600; background: var(--bg-color-hover); color: var(--text-color-2); }
.status-badge.success { background: rgba(74, 222, 128, 0.15); color: #4ade80; }
.status-badge.danger { background: rgba(239, 68, 68, 0.15); color: #ef4444; }

/* Buttons */
.primary-btn { padding: 8px 16px; border-radius: 6px; border: none; cursor: pointer; background: var(--primary-color); color: #fff; font-weight: 600; font-size: 0.9rem; transition: all 0.15s; }
.primary-btn:hover { filter: brightness(1.1); transform: translateY(-1px); }
.secondary-btn { padding: 8px 16px; border-radius: 6px; border: 1px solid var(--border-color); cursor: pointer; background: var(--bg-color-2); color: var(--text-color); font-weight: 500; font-size: 0.9rem; transition: all 0.15s; }
.secondary-btn:hover { background: var(--bg-color-hover); }
.danger-btn { padding: 8px 16px; border-radius: 6px; border: 1px solid rgba(239, 68, 68, 0.5); cursor: pointer; background: rgba(239, 68, 68, 0.1); color: #ef4444; font-weight: 500; font-size: 0.9rem; transition: all 0.15s; }
.danger-btn:hover { background: rgba(239, 68, 68, 0.2); }
.small { padding: 4px 10px; font-size: 0.8rem; }
.badge-btn { background: var(--bg-color-hover); border: none; padding: 4px 8px; border-radius: 4px; color: var(--text-color-2); font-size: 0.8rem; font-weight: 600; cursor: pointer; }
.badge-btn.badge-success { background: rgba(74, 222, 128, 0.15); color: #4ade80; }
.action-buttons { display: flex; gap: 8px; }
.icon-btn { background: none; border: none; cursor: pointer; font-size: 1.1rem; padding: 4px; border-radius: 4px; opacity: 0.7; transition: opacity 0.2s; }
.icon-btn:hover { opacity: 1; background: var(--bg-color-hover); }

/* Forms */
.settings-form { max-width: 600px; background: var(--bg-color-2); padding: 2rem; border-radius: 8px; border: 1px solid var(--border-color); }
.form-group { margin-bottom: 1.5rem; }
.form-group label { display: block; font-size: 0.9rem; font-weight: 500; color: var(--text-color); margin-bottom: 6px; }
.modern-input { width: 100%; padding: 10px 14px; border-radius: 6px; background: var(--bg-color-main); border: 1px solid var(--border-color); color: var(--text-color); font-family: inherit; font-size: 0.95rem; box-sizing: border-box; transition: border-color 0.2s; }
.modern-input:focus { border-color: var(--primary-color); outline: none; }
.checkbox-group label { display: flex; align-items: center; gap: 8px; cursor: pointer; font-weight: 500; }
.help-text { font-size: 0.8rem; color: var(--text-color-3); margin-top: 4px; }
hr { border: none; border-top: 1px solid var(--border-color); margin: 2rem 0; }

/* Stats Grid */
.stats-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1.5rem; }
.stat-card { background: var(--bg-color-2); border: 1px solid var(--border-color); padding: 1.5rem; border-radius: 8px; text-align: center; }
.stat-value { font-size: 2.5rem; font-weight: 700; color: var(--text-color); margin-bottom: 0.5rem; letter-spacing: -1px; }
.stat-value.highlight { color: var(--primary-color); }
.stat-label { font-size: 0.9rem; color: var(--text-color-2); font-weight: 500; text-transform: uppercase; letter-spacing: 0.5px; }

/* Modal */
.modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.6); display: flex; align-items: center; justify-content: center; z-index: 100; backdrop-filter: blur(2px); }
.modal { width: 100%; max-width: 450px; background: var(--bg-color-modal); padding: 2rem; border-radius: 12px; border: 1px solid var(--border-color); box-shadow: 0 20px 60px rgba(0,0,0,0.4); }
.modal h2 { margin: 0 0 1.5rem 0; font-size: 1.3rem; }
.modal-actions { display: flex; justify-content: flex-end; gap: 12px; margin-top: 2rem; }
</style>
