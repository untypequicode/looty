<script setup lang="ts">
const route = useRoute()
const workspaceId = route.params.id as string
const { user, isLoading } = useAuth()
const isJoining = ref(false)
const error = ref<string | null>(null)

async function joinWorkspace() {
  if (!user.value?.authenticated) {
    navigateTo('/login')
    return
  }
  
  isJoining.value = true
  error.value = null
  
  try {
    await $fetch(`/api/workspaces/${workspaceId}/join`, { method: 'POST' })
    navigateTo('/')
  } catch(e: any) {
    if (e.status === 404) {
      error.value = "Cet espace de travail n'existe pas."
    } else {
      error.value = "Une erreur est survenue en rejoignant l'espace."
    }
  } finally {
    isJoining.value = false
  }
}

watchEffect(() => {
  if (!isLoading.value && !user.value?.authenticated) {
    // Save intended destination for redirect after login if we wanted
    navigateTo('/login')
  }
})
</script>

<template>
  <div class="auth-layout" v-if="!isLoading && user?.authenticated">
    <div class="auth-card" style="text-align: center">
      <div class="logo">Looty</div>
      <h2>Rejoindre un espace de travail</h2>
      <p style="color: var(--text-secondary); margin-bottom: 2rem">
        Vous avez été invité à rejoindre un espace de travail.
      </p>
      
      <div v-if="error" style="color: #f87171; margin-bottom: 1rem;">
        {{ error }}
      </div>

      <button class="primary-btn" @click="joinWorkspace" :disabled="isJoining" style="width: 100%">
        {{ isJoining ? 'Connexion en cours...' : 'Rejoindre l\'espace' }}
      </button>
      
      <div style="margin-top: 1rem;">
        <NuxtLink to="/" style="color: var(--text-secondary); text-decoration: none; font-size: 0.9rem">
          Retour à l'accueil
        </NuxtLink>
      </div>
    </div>
  </div>
</template>
