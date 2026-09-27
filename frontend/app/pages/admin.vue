<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useAuth } from '../composables/useAuth'

const { user, isLoading } = useAuth()
const currentTab = ref('users') // 'users' or 'providers'

const users = ref<any[]>([])
const providers = ref<any[]>([])

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
      fetchUsers()
      fetchProviders()
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

async function fetchUsers() {
  try {
    users.value = await $fetch('/api/admin/users')
  } catch (e) {
    console.error(e)
  }
}

async function fetchProviders() {
  try {
    providers.value = await $fetch('/api/admin/providers')
  } catch (e) {
    console.error(e)
  }
}

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
      await $fetch(`/api/admin/providers/${editingProvider.value.id}`, {
        method: 'PUT',
        body: providerForm.value
      })
    } else {
      await $fetch('/api/admin/providers', {
        method: 'POST',
        body: providerForm.value
      })
    }
    showProviderModal.value = false
    fetchProviders()
  } catch (e) {
    alert("Erreur lors de la sauvegarde")
  }
}

async function deleteProvider(id: string) {
  if (confirm("Supprimer ce fournisseur OAuth ?")) {
    try {
      await $fetch(`/api/admin/providers/${id}`, { method: 'DELETE' })
      fetchProviders()
    } catch (e) {
      alert("Erreur")
    }
  }
}
</script>

<template>
  <div class="admin-layout" v-if="user?.user?.is_super_admin">
    <aside class="admin-sidebar">
      <h2>Looty Admin</h2>
      <nav>
        <button :class="{ active: currentTab === 'users' }" @click="currentTab = 'users'">Utilisateurs</button>
        <button :class="{ active: currentTab === 'providers' }" @click="currentTab = 'providers'">OAuth Agents</button>
      </nav>
      <div style="margin-top: auto;">
        <NuxtLink to="/" style="color: var(--text-color-3); text-decoration: none; font-size: 0.85rem;">&larr; Retour à l'app</NuxtLink>
      </div>
    </aside>

    <main class="admin-content">
      <div v-if="currentTab === 'users'">
        <div class="header-row">
          <h1>Utilisateurs ({{ users.length }})</h1>
        </div>
        <table class="admin-table">
          <thead>
            <tr>
              <th>ID</th>
              <th>Nom</th>
              <th>Email</th>
              <th>Admin</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="u in users" :key="u.id">
              <td><span style="font-family: monospace; font-size: 0.8em; color: var(--text-color-3)">{{ u.id.split('-')[0] }}...</span></td>
              <td>{{ u.name }}</td>
              <td>{{ u.email }}</td>
              <td>
                <span v-if="u.is_super_admin" style="color: var(--primary-color); font-weight: bold;">Oui</span>
                <span v-else>Non</span>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <div v-if="currentTab === 'providers'">
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
                <span v-if="p.is_active" style="color: #4ade80;">Actif</span>
                <span v-else style="color: var(--text-color-3);">Inactif</span>
              </td>
              <td>
                <button class="icon-btn" @click="openEditProvider(p)">✏️</button>
                <button class="icon-btn" @click="deleteProvider(p.id)">🗑️</button>
              </td>
            </tr>
            <tr v-if="providers.length === 0">
              <td colspan="5" style="text-align: center; color: var(--text-color-3);">Aucun fournisseur</td>
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
          <div style="margin-bottom: 12px;">
            <label style="display: flex; align-items: center; gap: 8px;">
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
.admin-layout { display: flex; height: 100vh; background: var(--bg-color-main); color: var(--text-color); }
.admin-sidebar { width: 250px; background: var(--bg-color-2); border-right: 1px solid var(--border-color); padding: 1.5rem; display: flex; flex-direction: column; }
.admin-sidebar h2 { margin: 0 0 2rem 0; font-size: 1.2rem; }
.admin-sidebar nav { display: flex; flex-direction: column; gap: 8px; }
.admin-sidebar nav button { text-align: left; background: transparent; border: none; padding: 8px 12px; border-radius: 6px; color: var(--text-color-2); cursor: pointer; font-size: 0.95rem; font-weight: 500; transition: all 0.15s; }
.admin-sidebar nav button:hover { background: var(--bg-color-hover); color: var(--text-color); }
.admin-sidebar nav button.active { background: var(--border-color); color: var(--text-color); }

.admin-content { flex: 1; padding: 2rem; overflow-y: auto; }
.header-row { display: flex; justify-content: space-between; align-items: center; margin-bottom: 2rem; }
.header-row h1 { margin: 0; font-size: 1.5rem; }

.admin-table { width: 100%; border-collapse: collapse; }
.admin-table th { text-align: left; padding: 12px; border-bottom: 1px solid var(--border-color); color: var(--text-color-3); font-weight: 500; font-size: 0.85rem; text-transform: uppercase; letter-spacing: 0.5px; }
.admin-table td { padding: 12px; border-bottom: 1px solid var(--border-color); font-size: 0.9rem; }
.admin-table tr:hover td { background: var(--bg-color-2); }

.primary-btn { padding: 6px 14px; border-radius: 6px; border: none; cursor: pointer; background: var(--primary-color); color: #fff; font-weight: 500; font-size: 0.85rem; transition: all 0.12s; }
.primary-btn:hover { background: var(--primary-color-hover); }
.secondary-btn { padding: 6px 14px; border-radius: 6px; border: 1px solid var(--border-color); cursor: pointer; background: transparent; color: var(--text-color-3); font-weight: 500; font-size: 0.85rem; transition: all 0.12s; }
.icon-btn { background: none; border: none; cursor: pointer; font-size: 1rem; padding: 4px; border-radius: 4px; }
.icon-btn:hover { background: var(--border-color); }

/* Modal */
.modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.7); display: flex; align-items: center; justify-content: center; z-index: 100; backdrop-filter: blur(4px); }
.modal { width: 100%; max-width: 450px; background: var(--bg-color-modal); padding: 1.5rem; border-radius: 8px; border: 1px solid var(--border-color); box-shadow: 0 20px 60px rgba(0,0,0,0.6); }
.modal h2 { margin: 0 0 1.5rem 0; font-size: 1.1rem; }
.modal label { display: block; font-size: 0.85rem; color: var(--text-color-2); margin-bottom: 4px; }
.modern-input { width: 100%; padding: 8px 12px; border-radius: 6px; background: var(--bg-color-2); border: 1px solid var(--border-color); color: var(--text-color); font-family: inherit; font-size: 0.9rem; box-sizing: border-box; }
.modern-input:focus { border-color: var(--primary-color); outline: none; }
.modal-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 1.5rem; }
</style>
