<script setup lang="ts">
const { user, isLoading } = useAuth()

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
      <h2 style="margin-top: 0;">Collab App</h2>
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
    <main style="flex-grow: 1; padding: 2rem;">
      <h1>Tableau de bord</h1>
      <p>Bienvenue sur votre espace de travail collaboratif.</p>
    </main>
  </div>
</template>
