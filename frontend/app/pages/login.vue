<script setup lang="ts">
import { ref, onMounted } from 'vue'
const { user, isLoading } = useAuth()
const isRedirecting = ref(false)

const providers = ref<any[]>([])
const isLoadingProviders = ref(true)

// Redirige vers / si déjà authentifié
watchEffect(() => {
  if (user.value?.authenticated) {
    navigateTo('/')
  }
})

onMounted(async () => {
  try {
    providers.value = await $fetch('/api/auth/providers')
  } catch (e) {
    console.error("Failed to fetch providers", e)
  } finally {
    isLoadingProviders.value = false
  }
})

async function loginWithProvider(providerId: string) {
  isRedirecting.value = true
  try {
    const { url } = await $fetch<{ url: string }>(`/api/auth/login/${providerId}`)
    window.location.href = url
  } catch (e) {
    console.error("Erreur lors de la récupération de l'URL de login", e)
    isRedirecting.value = false
  }
}
</script>

<template>
  <div style="font-family: sans-serif; display: flex; height: 100vh; align-items: center; justify-content: center; background: var(--bg-color-main);">
    <div style="background: var(--bg-color-2); padding: 2rem; border-radius: 8px; box-shadow: 0 4px 6px rgba(0,0,0,0.1); text-align: center; border: 1px solid var(--border-color);">
      <h1 style="margin-top: 0;">Connexion</h1>
      <p style="color: var(--text-color-2); margin-bottom: 2rem;">Veuillez vous authentifier pour continuer</p>
      
      <p v-if="isLoading || isRedirecting || isLoadingProviders">Chargement...</p>
      <div v-else style="display: flex; flex-direction: column; gap: 12px;">
        <button 
          v-for="provider in providers" 
          :key="provider.id"
          @click="loginWithProvider(provider.id)" 
          style="display: inline-block; cursor: pointer; background: var(--primary-color); color: #fff; border: none; padding: 10px 20px; border-radius: 6px; font-weight: bold; font-size: 16px; transition: all 0.2s;"
          onmouseover="this.style.background='var(--primary-color-hover)'"
          onmouseout="this.style.background='var(--primary-color)'"
        >
          Se connecter avec {{ provider.name }}
        </button>
        <p v-if="providers.length === 0" style="color: var(--text-color-3);">Aucun fournisseur d'authentification configuré.</p>
      </div>
    </div>
  </div>
</template>
