<script setup lang="ts">
import { useWorkspace } from '~/composables/useWorkspace'

/** Accueil de l'espace : tableau de bord des membres, pages partagées pour un invité. */
definePageMeta({ layout: 'workspace' })
const { workspace, isGuest } = useWorkspace()
</script>

<template>
  <div v-if="!workspace.data.value" class="home-wait"><UiSpinner /></div>
  <HomeGuest v-else-if="isGuest" />
  <HomeDashboard v-else />
</template>

<style scoped>
.home-wait { height: 100%; display: grid; place-items: center; }
</style>
