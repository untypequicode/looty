<script setup lang="ts">
import { useWebSocket } from '@vueuse/core'

const { user, isLoading } = useAuth()
const messageInput = ref('')

// On se connecte au WebSocket uniquement côté client (hydration)
const { status, data, send } = useWebSocket('/api/ws', {
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

// Protège la route
watchEffect(() => {
  if (!isLoading.value && !user.value?.authenticated) {
    navigateTo('/login')
  }
})
</script>

<template>
  <div v-if="isLoading" style="padding: 2rem; font-family: sans-serif;">
    Chargement de la session...
  </div>
  <div v-else-if="user?.authenticated" style="font-family: sans-serif; display: flex; height: 100vh;">
    <!-- Sidebar -->
    <aside style="width: 250px; background: #fafafa; border-right: 1px solid #eaeaea; padding: 1.5rem; display: flex; flex-direction: column;">
      <h2 style="margin-top: 0;">Looty</h2>
      <div style="flex-grow: 1;">
        <p style="color: #666; font-size: 0.9em; font-weight: bold; text-transform: uppercase;">Espaces</p>
        <ul style="list-style: none; padding: 0; margin: 0;">
          <li style="padding: 0.5rem 0; cursor: pointer;">Mon espace</li>
        </ul>
      </div>
      
      <div style="border-top: 1px solid #eaeaea; padding-top: 1rem; margin-top: auto;">
        <p style="margin: 0; font-weight: bold;">{{ user.user.name }}</p>
        <p style="margin: 0; font-size: 0.8em; color: #666;">{{ user.user.email }}</p>
        <a href="/api/auth/logout" style="display: inline-block; margin-top: 10px; color: #d32f2f; text-decoration: none; font-size: 0.9em;">Se déconnecter</a>
      </div>
    </aside>

    <!-- Main Content -->
    <main style="flex-grow: 1; padding: 2rem; display: flex; flex-direction: column;">
      <h1>Tableau de bord</h1>
      <p>Bienvenue sur votre espace de travail collaboratif.</p>

      <!-- WebSocket Test -->
      <div style="margin-top: 2rem; border: 1px solid #eaeaea; padding: 1rem; border-radius: 8px; flex-grow: 1; display: flex; flex-direction: column;">
        <h3 style="margin-top: 0;">Test WebSockets (V2)</h3>
        <p>Statut : <strong>{{ status }}</strong></p>
        
        <div style="flex-grow: 1; border: 1px solid #ccc; border-radius: 4px; padding: 1rem; overflow-y: auto; background: #fafafa; margin-bottom: 1rem; min-height: 200px;">
          <div v-for="(msg, i) in messages" :key="i" style="margin-bottom: 0.5rem; padding: 0.5rem; background: white; border-radius: 4px; border: 1px solid #eaeaea;">
            {{ msg }}
          </div>
        </div>

        <form @submit.prevent="sendMessage" style="display: flex; gap: 10px;">
          <input v-model="messageInput" type="text" placeholder="Envoyer un message au serveur..." style="flex-grow: 1; padding: 0.5rem; border: 1px solid #ccc; border-radius: 4px;">
          <button type="submit" style="padding: 0.5rem 1rem; background: #007bff; color: white; border: none; border-radius: 4px; cursor: pointer;">Envoyer</button>
        </form>
      </div>
    </main>
  </div>
</template>
