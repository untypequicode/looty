<script setup lang="ts">
import { useStudySet } from '~/composables/useStudy'
import { links } from '~/composables/useWorkspace'

/** Ancienne adresse d'un entraînement (/study/<id>) : redirigée vers sa place dans son espace. */
definePageMeta({ layout: false })
const route = useRoute()
const id = computed(() => route.params.id as string)
const { data, isError } = useStudySet(id)
watch(data, (set) => {
  if (!set) return
  const mode = route.query.mode === 'edit' ? 'edit' : undefined
  navigateTo(links.studySet(set.workspace_id, set.id, mode), { replace: true })
}, { immediate: true })
</script>

<template>
  <div class="center">
    <UiEmpty v-if="isError" :title="$t('study.notFound')" />
    <UiSpinner v-else />
  </div>
</template>

<style scoped>
.center { display: grid; place-items: center; min-height: 60vh; }
</style>
