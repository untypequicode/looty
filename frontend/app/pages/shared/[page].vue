<script setup lang="ts">
import { useSharedPages } from '~/composables/useShared'

/**
 * Ancien lien de page partagée : la page s'ouvre désormais dans l'interface de l'espace
 * (en invité, seules les pages partagées y apparaissent).
 */
definePageMeta({ layout: false })
const route = useRoute()
const { data: shared, isFetched } = useSharedPages()
watch([shared, isFetched], () => {
  const id = route.params.page as string
  const entry = shared.value?.find((p) => p.id === id)
  if (entry) navigateTo(`/w/${entry.workspace_id}/doc/${id}`, { replace: true })
  else if (isFetched.value) navigateTo('/shared', { replace: true })
}, { immediate: true })
</script>

<template>
  <div style="min-height: 100vh; display: grid; place-items: center"><UiSpinner /></div>
</template>
