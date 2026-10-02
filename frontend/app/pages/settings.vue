<script setup lang="ts">
import { ref, onMounted, watchEffect } from 'vue'
import { useAuth } from '../composables/useAuth'

const { user, isLoading } = useAuth()
const currentTab = ref('profile')

const profileForm = ref({
  name: '',
  avatar_url: '',
  theme: 'system'
})

const identities = ref<any[]>([])
const tokens = ref<any[]>([])

const newTokenName = ref('')
const generatedToken = ref('')

onMounted(() => {
  if (user.value?.authenticated) {
    profileForm.value = {
      name: user.value.user.name || '',
      avatar_url: user.value.user.avatar_url || '',
      theme: user.value.user.theme || 'system'
    }
    fetchIdentities()
    fetchTokens()
  }
})

watchEffect(() => {
  if (!isLoading.value && !user.value?.authenticated) {
    navigateTo('/login')
  }
})

const isUploadingAvatar = ref(false)
const fileInput = ref<HTMLInputElement | null>(null)

async function saveProfile() {
  try {
    await $fetch('/api/user/profile', {
      method: 'PUT',
      body: profileForm.value
    })
    alert('Profil mis à jour ! Veuillez rafraîchir pour voir les changements.')
  } catch (e) {
    alert("Erreur lors de la sauvegarde")
  }
}

async function uploadAvatar(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0]
  if (!file) return
  isUploadingAvatar.value = true
  const formData = new FormData()
  formData.append('file', file)
  try {
    const res: any = await $fetch('/api/upload', {
      method: 'POST',
      body: formData
    })
    profileForm.value.avatar_url = res.url
  } catch(e) {
    alert("Erreur lors de l'upload")
  } finally {
    isUploadingAvatar.value = false
  }
}

async function fetchIdentities() {
  try {
    identities.value = await $fetch('/api/user/identities')
  } catch (e) {}
}

async function fetchTokens() {
  try {
    tokens.value = await $fetch('/api/user/tokens')
  } catch (e) {}
}

async function unlinkIdentity(provider: string) {
  if (identities.value.length <= 1) {
    alert("Impossible de supprimer le dernier compte lié. Vous perdriez l'accès à votre compte.")
    return
  }
  if (confirm(`Voulez-vous vraiment délier ce compte ${provider} ?`)) {
    try {
      await $fetch(`/api/user/identities/${provider}`, { method: 'DELETE' })
      fetchIdentities()
    } catch (e: any) {
      alert(e.response?.data || "Erreur")
    }
  }
}

async function createToken() {
  if (!newTokenName.value) return
  try {
    const res: any = await $fetch('/api/user/tokens', {
      method: 'POST',
      body: { name: newTokenName.value }
    })
    generatedToken.value = res.token
    newTokenName.value = ''
    fetchTokens()
  } catch (e) {
    alert("Erreur lors de la création")
  }
}

async function deleteToken(id: string) {
  if (confirm("Révoquer ce jeton d'accès ?")) {
    try {
      await $fetch(`/api/user/tokens/${id}`, { method: 'DELETE' })
      fetchTokens()
    } catch (e) {}
  }
}

function copyToken() {
  navigator.clipboard.writeText(generatedToken.value)
  alert('Jeton copié dans le presse-papier !')
}
</script>

<template>
  <div class="settings-layout" v-if="user?.authenticated">
    <aside class="settings-sidebar">
      <h2>Paramètres</h2>
      <nav>
        <button :class="{ active: currentTab === 'profile' }" @click="currentTab = 'profile'">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" style="margin-right: 6px; vertical-align: -3px;"><path stroke-linecap="round" stroke-linejoin="round" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" /></svg>
          Mon Profil
        </button>
        <button :class="{ active: currentTab === 'security' }" @click="currentTab = 'security'">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" style="margin-right: 6px; vertical-align: -3px;"><path stroke-linecap="round" stroke-linejoin="round" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" /></svg>
          Connexions liées
        </button>
        <button :class="{ active: currentTab === 'api' }" @click="currentTab = 'api'">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" style="margin-right: 6px; vertical-align: -3px;"><path stroke-linecap="round" stroke-linejoin="round" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" /></svg>
          Jetons API
        </button>
      </nav>
      <div style="margin-top: auto;">
        <NuxtLink to="/" style="color: var(--text-color-3); text-decoration: none; font-size: 0.85rem;">&larr; Retour à l'app</NuxtLink>
      </div>
    </aside>

    <main class="settings-content">
      
      <!-- PROFIL -->
      <div v-if="currentTab === 'profile'" class="tab-content">
        <h1>Profil & Préférences</h1>
        <p class="subtitle">Gérez vos informations personnelles et l'apparence.</p>
        
        <form @submit.prevent="saveProfile" class="settings-form">
          <div class="form-group">
            <label>Nom</label>
            <input v-model="profileForm.name" type="text" class="modern-input" required>
          </div>
          
          <div class="form-group">
            <label>Adresse e-mail</label>
            <input :value="user.user.email" type="email" class="modern-input" disabled style="opacity: 0.6; cursor: not-allowed;">
            <p class="help-text">Votre e-mail est lié à vos comptes de connexion.</p>
          </div>

          <div class="form-group">
            <label>Photo de profil (Avatar)</label>
            <div style="display: flex; gap: 10px; align-items: center;">
              <img v-if="profileForm.avatar_url" :src="profileForm.avatar_url" style="width: 40px; height: 40px; border-radius: 50%; object-fit: cover; border: 1px solid var(--border-color);" />
              <input type="file" accept="image/*" @change="uploadAvatar" style="display: none;" ref="fileInput">
              <button type="button" class="secondary-btn small" @click="fileInput?.click()">{{ isUploadingAvatar ? 'Chargement...' : 'Uploader une image' }}</button>
            </div>
            <p class="help-text" style="margin-top: 8px;">Ou via URL :</p>
            <input v-model="profileForm.avatar_url" type="url" class="modern-input" placeholder="https://..." style="margin-top: 4px;">
          </div>

          <div class="form-group">
            <label>Thème de l'interface</label>
            <select v-model="profileForm.theme" class="modern-input">
              <option value="system">Système (Auto)</option>
              <option value="light">Clair</option>
              <option value="dark">Sombre</option>
            </select>
          </div>

          <button type="submit" class="primary-btn" style="margin-top: 1rem;">Sauvegarder le profil</button>
        </form>
      </div>

      <!-- SÉCURITÉ / OAUTH -->
      <div v-if="currentTab === 'security'" class="tab-content">
        <h1>Comptes liés</h1>
        <p class="subtitle">Gérez les services externes autorisés à vous connecter.</p>
        
        <div class="card-list">
          <div v-for="identity in identities" :key="identity.provider" class="identity-card">
            <div class="identity-info">
              <span class="provider-badge">{{ identity.provider }}</span>
              <span class="date">Lié le {{ new Date(identity.created_at).toLocaleDateString() }}</span>
            </div>
            <button class="danger-btn small" @click="unlinkIdentity(identity.provider)" :disabled="identities.length <= 1">
              Délier
            </button>
          </div>
          <p v-if="identities.length === 1" class="help-text warning-text" style="display: flex; align-items: center; gap: 6px;">
            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" /></svg>
            Vous devez garder au moins une méthode de connexion active.
          </p>
        </div>
      </div>

      <!-- API TOKENS -->
      <div v-if="currentTab === 'api'" class="tab-content">
        <h1>Jetons d'Accès Personnels</h1>
        <p class="subtitle">Générez des jetons pour utiliser l'API Looty depuis d'autres applications.</p>
        
        <div v-if="generatedToken" class="alert-box success">
          <strong>Nouveau jeton généré !</strong><br>
          <p>Copiez-le maintenant, vous ne pourrez plus jamais le revoir :</p>
          <div class="token-display">
            <code>{{ generatedToken }}</code>
            <button @click="copyToken" class="secondary-btn small">Copier</button>
          </div>
        </div>

        <form @submit.prevent="createToken" class="add-token-form">
          <input v-model="newTokenName" type="text" class="modern-input" placeholder="Nom du jeton (ex: Script Python)" required>
          <button type="submit" class="primary-btn">Créer un jeton</button>
        </form>

        <table class="admin-table" style="margin-top: 2rem;">
          <thead>
            <tr>
              <th>Nom</th>
              <th>Créé le</th>
              <th>Dernière utilisation</th>
              <th>Action</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="t in tokens" :key="t.id">
              <td style="font-weight: bold;">{{ t.name }}</td>
              <td>{{ new Date(t.created_at).toLocaleDateString() }}</td>
              <td>
                <span v-if="t.last_used_at">{{ new Date(t.last_used_at).toLocaleDateString() }}</span>
                <span v-else style="color: var(--text-color-3)">Jamais</span>
              </td>
              <td>
                <button class="danger-btn small" @click="deleteToken(t.id)">Révoquer</button>
              </td>
            </tr>
            <tr v-if="tokens.length === 0">
              <td colspan="4" style="text-align: center; color: var(--text-color-3);">Aucun jeton actif</td>
            </tr>
          </tbody>
        </table>
      </div>

    </main>
  </div>
</template>

<style scoped>
.settings-layout { display: flex; height: 100vh; background: var(--bg-color-main); color: var(--text-color); font-family: 'Inter', sans-serif; }
.settings-sidebar { width: 260px; background: var(--bg-color-2); border-right: 1px solid var(--border-color); padding: 1.5rem; display: flex; flex-direction: column; }
.settings-sidebar h2 { margin: 0 0 2rem 0; font-size: 1.3rem; font-weight: 600; letter-spacing: -0.5px; }
.settings-sidebar nav { display: flex; flex-direction: column; gap: 6px; }
.settings-sidebar nav button { text-align: left; background: transparent; border: none; padding: 10px 14px; border-radius: 8px; color: var(--text-color-2); cursor: pointer; font-size: 0.95rem; font-weight: 500; transition: all 0.2s; }
.settings-sidebar nav button:hover { background: var(--bg-color-hover); color: var(--text-color); }
.settings-sidebar nav button.active { background: var(--border-color); color: var(--text-color); font-weight: 600; box-shadow: 0 1px 2px rgba(0,0,0,0.05); }

.settings-content { flex: 1; padding: 3rem 4rem; overflow-y: auto; }
.tab-content { max-width: 800px; }
.tab-content h1 { margin: 0 0 0.5rem 0; font-size: 1.8rem; font-weight: 600; letter-spacing: -0.5px; }
.subtitle { color: var(--text-color-3); margin-top: 0; margin-bottom: 2rem; }

/* Forms */
.settings-form { background: var(--bg-color-2); padding: 2rem; border-radius: 8px; border: 1px solid var(--border-color); }
.form-group { margin-bottom: 1.5rem; }
.form-group label { display: block; font-size: 0.9rem; font-weight: 500; color: var(--text-color); margin-bottom: 6px; }
.modern-input { width: 100%; padding: 10px 14px; border-radius: 6px; background: var(--bg-color-main); border: 1px solid var(--border-color); color: var(--text-color); font-family: inherit; font-size: 0.95rem; box-sizing: border-box; transition: border-color 0.2s; }
.modern-input:focus { border-color: var(--primary-color); outline: none; }
.help-text { font-size: 0.8rem; color: var(--text-color-3); margin-top: 6px; }
.warning-text { color: #f59e0b; font-weight: 500; }

/* Buttons */
.primary-btn { padding: 8px 16px; border-radius: 6px; border: none; cursor: pointer; background: var(--primary-color); color: #fff; font-weight: 600; font-size: 0.9rem; transition: all 0.15s; }
.primary-btn:hover { filter: brightness(1.1); transform: translateY(-1px); }
.secondary-btn { padding: 8px 16px; border-radius: 6px; border: 1px solid var(--border-color); cursor: pointer; background: var(--bg-color-2); color: var(--text-color); font-weight: 500; font-size: 0.9rem; transition: all 0.15s; }
.secondary-btn:hover { background: var(--bg-color-hover); }
.danger-btn { padding: 8px 16px; border-radius: 6px; border: 1px solid rgba(239, 68, 68, 0.5); cursor: pointer; background: rgba(239, 68, 68, 0.1); color: #ef4444; font-weight: 500; font-size: 0.9rem; transition: all 0.15s; }
.danger-btn:hover:not(:disabled) { background: rgba(239, 68, 68, 0.2); }
.danger-btn:disabled { opacity: 0.5; cursor: not-allowed; }
.small { padding: 4px 10px; font-size: 0.8rem; }

/* Identities & Cards */
.card-list { display: flex; flex-direction: column; gap: 12px; }
.identity-card { display: flex; justify-content: space-between; align-items: center; background: var(--bg-color-2); border: 1px solid var(--border-color); padding: 1rem 1.5rem; border-radius: 8px; }
.provider-badge { font-weight: 600; font-size: 1rem; text-transform: capitalize; }
.date { font-size: 0.85rem; color: var(--text-color-3); margin-left: 12px; }

/* API Tokens */
.add-token-form { display: flex; gap: 12px; margin-top: 1rem; }
.alert-box { padding: 1rem; border-radius: 8px; margin-bottom: 2rem; border: 1px solid transparent; }
.alert-box.success { background: rgba(74, 222, 128, 0.1); border-color: rgba(74, 222, 128, 0.3); color: #22c55e; }
.token-display { display: flex; gap: 12px; align-items: center; margin-top: 12px; background: rgba(0,0,0,0.2); padding: 8px 12px; border-radius: 6px; }
.token-display code { font-family: monospace; font-size: 1.1rem; flex: 1; word-break: break-all; }

/* Tables */
.admin-table { width: 100%; border-collapse: separate; border-spacing: 0; background: var(--bg-color-2); border: 1px solid var(--border-color); border-radius: 8px; overflow: hidden; }
.admin-table th { text-align: left; padding: 14px 16px; border-bottom: 1px solid var(--border-color); color: var(--text-color-3); font-weight: 600; font-size: 0.85rem; text-transform: uppercase; letter-spacing: 0.5px; background: var(--bg-color-modal); }
.admin-table td { padding: 14px 16px; border-bottom: 1px solid var(--border-color); font-size: 0.9rem; }
.admin-table tr:last-child td { border-bottom: none; }
.admin-table tr:hover td { background: var(--bg-color-hover); }
</style>
