<script setup lang="ts">
const { user, isLoading } = useAuth()
const isRedirecting = ref(false)

// Redirige vers / si déjà authentifié
watchEffect(() => {
  if (user.value?.authenticated) {
    navigateTo('/')
  }
})

async function loginWithZitadel() {
  isRedirecting.value = true
  try {
    const { url } = await $fetch<{ url: string }>('/api/auth/login')
    window.location.href = url
  } catch (e) {
    console.error("Erreur lors de la récupération de l'URL de login", e)
    isRedirecting.value = false
  }
}
</script>

<template>
  <div style="font-family: sans-serif; display: flex; height: 100vh; align-items: center; justify-content: center; background: #f9f9f9;">
    <div style="background: white; padding: 2rem; border-radius: 8px; box-shadow: 0 4px 6px rgba(0,0,0,0.1); text-align: center;">
      <h1 style="margin-top: 0;">Connexion</h1>
      <p style="color: #666; margin-bottom: 2rem;">Veuillez vous authentifier pour continuer</p>
      
      <p v-if="isLoading || isRedirecting">Vérification de la session...</p>
      <button v-else @click="loginWithZitadel" style="display: inline-block; cursor: pointer; background: #000; color: #fff; border: none; padding: 10px 20px; border-radius: 6px; font-weight: bold; font-size: 16px;">
        Se connecter avec Zitadel
      </button>
    </div>
  </div>
</template>
