<script setup lang="ts">
import { FileQuestion } from 'lucide-vue-next'
import { api } from '~/composables/api'
import { links } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'

/** Lien court vers une tâche : résout son tableau puis redirige vers la vue tableau. */
definePageMeta({ layout: 'workspace' })
const route = useRoute()
const { t } = useI18n()
const missing = ref(false)

onMounted(async () => {
  try {
    const r = await api<{ board_id: string; workspace_id: string }>(`/api/boards/tasks/${route.params.task}`, { silent: true })
    navigateTo({ path: links.board(r.workspace_id, r.board_id), query: { task: route.params.task as string }, hash: route.hash }, { replace: true })
  } catch {
    missing.value = true
  }
})
</script>

<template>
  <div class="page">
    <AppPageHeader :title="t('task.task')" />
    <UiEmpty v-if="missing" :icon="FileQuestion" :title="t('task.notFound')" :description="t('task.notFoundHint')" />
    <div v-else style="display: grid; place-items: center; flex: 1"><UiSpinner /></div>
  </div>
</template>
