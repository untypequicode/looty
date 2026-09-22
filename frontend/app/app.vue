<script setup lang="ts">
const { data, pending, error } = useFetch('/api/health')
</script>

<template>
  <div style="font-family: sans-serif; padding: 2rem;">
    <h1>App Collaborative - Version 0</h1>
    <p>Frontend (Nuxt 4) : 🟢 En ligne</p>
    <div style="margin-top: 1rem; padding: 1rem; border: 1px solid #ccc; border-radius: 8px;">
      <h2>Statut du Backend (Rust)</h2>
      <p v-if="pending">Vérification en cours...</p>
      <p v-else-if="error" style="color: red;">Erreur de connexion : {{ error.message }}</p>
      <p v-else style="color: green;">Connecté ! Réponse du serveur : {{ data }}</p>
    </div>
  </div>
</template>
