<script setup lang="ts">
import { LogOut, Trash2 } from 'lucide-vue-next'
import { useQueryClient } from '@tanstack/vue-query'
import { useWorkspace, wsKeys } from '~/composables/useWorkspace'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'

const { wsId, workspace, role, isAdmin } = useWorkspace()
const { t } = useI18n()
const qc = useQueryClient()
const name = ref('')
watch(() => workspace.data.value?.name, (v) => { if (v) name.value = v }, { immediate: true })
const saving = ref(false)

async function save() {
  saving.value = true
  try {
    await api(`/api/workspaces/${wsId.value}`, { method: 'PUT', body: { name: name.value.trim() } })
    qc.invalidateQueries({ queryKey: wsKeys.one(wsId.value) })
    qc.invalidateQueries({ queryKey: wsKeys.list })
    toast.success(t('common.saved'))
  } finally { saving.value = false }
}
async function leave() {
  if (!(await confirmDialog({ title: t('settings.ws.leaveTitle', { name: workspace.data.value?.name ?? '' }), message: t('settings.ws.leaveMessage'), danger: true, confirmLabel: t('settings.ws.leave') }))) return
  await api(`/api/workspaces/${wsId.value}/leave`, { method: 'POST' })
  await qc.invalidateQueries({ queryKey: wsKeys.list })
  localStorage.removeItem('looty.lastWorkspace')
  navigateTo('/')
}
async function remove() {
  const n = workspace.data.value?.name ?? ''
  if (!(await confirmDialog({ title: t('settings.ws.deleteTitle', { name: n }), message: t('settings.ws.deleteMessage'), danger: true, confirmLabel: t('common.delete'), typeToConfirm: n }))) return
  await api(`/api/workspaces/${wsId.value}`, { method: 'DELETE' })
  await qc.invalidateQueries({ queryKey: wsKeys.list })
  localStorage.removeItem('looty.lastWorkspace')
  navigateTo('/')
}
</script>

<template>
  <SettingsSection :title="t('settings.ws.title')" :description="t('settings.ws.description')">
    <SettingsRow :label="t('workspace.name')">
      <UiInput v-model="name" :disabled="!isAdmin" style="width: 260px" maxlength="120" />
      <UiButton v-if="isAdmin" variant="primary" size="sm" :disabled="!name.trim() || name.trim() === workspace.data.value?.name" :loading="saving" @click="save">{{ t('common.save') }}</UiButton>
    </SettingsRow>
    <SettingsRow :label="t('settings.ws.yourRole')"><span class="muted">{{ role ? t(`roles.${role}`) : '' }}</span></SettingsRow>
  </SettingsSection>
  <SettingsSection :title="t('settings.ws.danger')" danger>
    <SettingsRow :label="t('settings.ws.leave')" :hint="t('settings.ws.leaveHint')">
      <UiButton size="sm" :icon="LogOut" @click="leave">{{ t('settings.ws.leave') }}</UiButton>
    </SettingsRow>
    <SettingsRow v-if="role === 'owner'" :label="t('settings.ws.delete')" :hint="t('settings.ws.deleteHint')">
      <UiButton size="sm" variant="danger" :icon="Trash2" @click="remove">{{ t('common.delete') }}</UiButton>
    </SettingsRow>
  </SettingsSection>
</template>
