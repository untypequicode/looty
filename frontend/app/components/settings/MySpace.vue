<script setup lang="ts">
import { PanelLeftDashed } from 'lucide-vue-next'
import { useQueryClient } from '@tanstack/vue-query'
import { useWorkspace, wsKeys } from '~/composables/useWorkspace'
import { useAccess, AREAS, accessKey, type AreaMode, type Area } from '~/composables/useAccess'
import { useSidebarLayout } from '~/composables/useSidebarLayout'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'

/** Ma participation dans cet espace : mode d'édition par domaine, validations collectives. */
const { wsId, workspace } = useWorkspace()
const { t } = useI18n()
const qc = useQueryClient()
const { access, allows, mode, setMode } = useAccess(wsId)
const { customizeOpen } = useSidebarLayout(wsId)
const areas = computed(() => AREAS.filter((a) => allows(a, 'edit')))
const MODES: AreaMode[] = ['write', 'read', 'restricted']
async function pick(a: Area, m: AreaMode) {
  await setMode(a, m)
  toast.success(t('common.saved'))
}
const groupTasks = computed({
  get: () => access.value?.group_tasks !== false,
  set: async (v: boolean) => {
    await api(`/api/workspaces/${wsId.value}/access/me`, { method: 'PATCH', body: { group_tasks: v } })
    qc.invalidateQueries({ queryKey: accessKey(wsId.value) })
    qc.invalidateQueries({ queryKey: wsKeys.members(wsId.value) })
    toast.success(t('common.saved'))
  },
})
</script>

<template>
  <SettingsSection :title="t('access.modesTitle')" :description="t('access.modesHint', { name: workspace.data.value?.name ?? '' })">
    <SettingsRow v-for="a in areas" :key="a" :label="t(`access.areas.${a}`)" :hint="t(`access.modeHint.${mode(a)}`)" stacked>
      <UiSegmented :model-value="mode(a)" size="md" :options="MODES.map((m) => ({ value: m, label: t(`access.modes.${m}`) }))" @update:model-value="(m) => pick(a, m as AreaMode)" />
    </SettingsRow>
    <p v-if="!areas.length" class="faint none">{{ t('access.noEditAreas') }}</p>
  </SettingsSection>
  <SettingsSection :title="t('access.groupTitle')">
    <SettingsRow :label="t('access.groupTasks')" :hint="t('access.groupTasksHint')">
      <UiSwitch v-model="groupTasks" />
    </SettingsRow>
  </SettingsSection>
  <SettingsSection :title="t('sidebarLayout.title')">
    <SettingsRow :label="t('sidebarLayout.customize')" :hint="t('sidebarLayout.hint')">
      <UiButton size="sm" :icon="PanelLeftDashed" @click="customizeOpen = true">{{ t('sidebarLayout.customize') }}</UiButton>
    </SettingsRow>
  </SettingsSection>
  <AppSidebarCustomize v-model:open="customizeOpen" />
</template>

<style scoped>
.none { padding: 14px 16px; margin: 0; }
</style>
