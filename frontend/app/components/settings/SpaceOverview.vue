<script setup lang="ts">
import { useWorkspace } from '~/composables/useWorkspace'
import { useAccess } from '~/composables/useAccess'
import { useI18n } from '~/composables/useI18n'

/** Vue d'ensemble de l'espace pour ses administrateurs : usage et liens publics. */
const { wsId } = useWorkspace()
const { can } = useAccess(wsId)
const { t } = useI18n()
const tab = ref<'usage' | 'links'>('usage')
</script>

<template>
  <UiEmpty v-if="!can.admin" :title="t('errors.forbidden')" />
  <template v-else>
    <UiSegmented v-model="tab" size="md" class="so-tabs" :options="[{ value: 'usage', label: t('insights.usage') }, { value: 'links', label: t('links.title') }]" />
    <AdminInsights v-if="tab === 'usage'" :endpoint="`/api/workspaces/${wsId}/admin/analytics`" />
    <SettingsPublicLinks v-else :ws-id="wsId" />
  </template>
</template>

<style scoped>
.so-tabs { margin-bottom: 16px; }
</style>
