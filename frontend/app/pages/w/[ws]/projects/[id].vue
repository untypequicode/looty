<script setup lang="ts">
definePageMeta({ layout: 'workspace' })

/** Projet (mini-site, LaTeX ou notebook) ouvert seul, hors des documents. */
const route = useRoute()
const id = computed(() => route.params.id as string)
</script>

<template>
  <ProjectWorkspace :key="id" :project-id="id" />
</template>
