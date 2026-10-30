<script setup lang="ts">
import { LogOut, Trash2, Upload, Download } from 'lucide-vue-next'
import { useQueryClient } from '@tanstack/vue-query'
import { useWorkspace, wsKeys } from '~/composables/useWorkspace'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'
import { useTrash } from '~/composables/useTrash'
import { useMoodle, moodleKeys } from '~/composables/useMoodle'

const { wsId, workspace, role, isAdmin, canWriteRole: canWrite } = useWorkspace()
const personal = computed(() => !!workspace.data.value?.personal)
const importOpen = ref(false)
const exportOpen = ref(false)
const { t } = useI18n()
const qc = useQueryClient()
const name = ref('')
watch(() => workspace.data.value?.name, (v) => { if (v) name.value = v }, { immediate: true })
const saving = ref(false)

// Corbeille : durée de conservation (administrateurs)
const RETENTION = [0, 7, 14, 30, 60, 90, 180, 365]
const trash = useTrash(wsId)
const retention = ref<number | null>(null)
const retentionItems = computed(() => RETENTION.map((d) => ({ value: String(d), label: d ? t('trash.days', { count: d }) : t('trash.off') })))
watch(() => trash.data.value?.retention_days, (d) => { if (d != null) retention.value = d }, { immediate: true })
async function saveRetention(days: number) {
  retention.value = days
  await api(`/api/workspaces/${wsId.value}/trash/settings`, { method: 'PUT', body: { retention_days: days } })
  trash.refetch()
  toast.success(t('common.saved'))
}

// Moodle : plateforme de l'espace (administrateurs)
const moodle = useMoodle(wsId)
const moodleUrl = ref('')
watch(() => moodle.data.value, (d) => { if (d) moodleUrl.value = d.url ?? '' }, { immediate: true })
const savingMoodle = ref(false)
async function saveMoodle() {
  savingMoodle.value = true
  try {
    await api(`/api/workspaces/${wsId.value}/moodle/settings`, { method: 'PUT', body: { url: moodleUrl.value.trim() || null } })
    qc.invalidateQueries({ queryKey: moodleKeys.overview(wsId.value) })
    toast.success(t('common.saved'))
  } finally { savingMoodle.value = false }
}

// Drive temporaire : durée maximale de conservation et quota (administrateurs)
const DRIVE_DAYS = [1, 3, 7, 14, 30, 60, 90, 180, 365]
const driveDays = ref<string>('7')
const driveQuota = ref('2048')
const driveDaysItems = computed(() => DRIVE_DAYS.map((d) => ({ value: String(d), label: t('drive.days', { count: d }) })))
const driveDaysOpen = ref(false)
onMounted(async () => {
  if (!isAdmin.value) return
  const d = await api<{ max_days: number; quota_bytes: number }>(`/api/workspaces/${wsId.value}/drive`, { silent: true }).catch(() => null)
  if (d) { driveDays.value = String(d.max_days); driveQuota.value = String(Math.round(d.quota_bytes / 1048576)) }
})
async function saveDrive(days = driveDays.value) {
  driveDays.value = days
  await api(`/api/workspaces/${wsId.value}/drive/settings`, { method: 'PUT', body: { max_days: Number(days), quota_mb: Number(driveQuota.value) } })
  toast.success(t('common.saved'))
}

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
  <!-- Espace perso : ni nom ni rôle à régler, il est à soi. -->
  <SettingsSection :title="personal ? t('personal.title') : t('settings.ws.title')" :description="personal ? t('personal.hint') : t('settings.ws.description')">
    <SettingsRow v-if="!personal" :label="t('workspace.name')">
      <UiInput v-model="name" :disabled="!isAdmin" style="width: 260px" maxlength="120" />
      <UiButton v-if="isAdmin" variant="primary" size="sm" :disabled="!name.trim() || name.trim() === workspace.data.value?.name" :loading="saving" @click="save">{{ t('common.save') }}</UiButton>
    </SettingsRow>
    <SettingsRow v-if="!personal" :label="t('settings.ws.yourRole')"><span class="muted">{{ role ? t(`roles.${role}`) : '' }}</span></SettingsRow>
    <SettingsRow v-if="isAdmin && retention != null" :label="t('trash.retention')" :hint="t('trash.retentionHint')">
      <UiCombobox :model-value="String(retention)" :items="retentionItems" :width="240" @update:model-value="(v) => saveRetention(Number(v))">
        <button class="ws-select">{{ retentionItems.find((i) => i.value === String(retention))?.label }}</button>
      </UiCombobox>
    </SettingsRow>
    <SettingsRow v-if="isAdmin" :label="t('moodle.settingLabel')" :hint="t('moodle.settingHint')">
      <div class="moodle-row">
        <UiInput v-model="moodleUrl" :placeholder="moodle.data.value?.default_url ?? 'https://moodle.u-bordeaux.fr'" style="width: 260px" @keydown.enter="saveMoodle" />
        <UiButton v-if="!moodleUrl && moodle.data.value" size="sm" variant="ghost" @click="moodleUrl = moodle.data.value.default_url">{{ t('moodle.useDefault') }}</UiButton>
        <UiButton size="sm" :loading="savingMoodle" :disabled="(moodle.data.value?.url ?? '') === moodleUrl.trim()" @click="saveMoodle">{{ t('common.save') }}</UiButton>
      </div>
    </SettingsRow>
  </SettingsSection>
  <SettingsSection v-if="isAdmin" :title="t('drive.title')" :description="t('drive.settingsHint')">
    <SettingsRow :label="t('drive.maxDuration')" :hint="t('drive.maxDurationHint')">
      <UiCombobox :model-value="driveDays" v-model:open="driveDaysOpen" :items="driveDaysItems" :width="200" @update:model-value="(v) => saveDrive(String(v))">
        <button class="ws-select">{{ driveDaysItems.find((i) => i.value === driveDays)?.label }}</button>
      </UiCombobox>
    </SettingsRow>
    <SettingsRow :label="t('drive.quotaLabel')" :hint="t('drive.quotaHint')">
      <div class="moodle-row"><UiInput v-model="driveQuota" type="number" min="50" style="width: 110px" @keydown.enter="saveDrive()" /><span class="muted">Mo</span><UiButton size="sm" @click="saveDrive()">{{ t('common.save') }}</UiButton></div>
    </SettingsRow>
  </SettingsSection>
  <SettingsLatexPreamble />
  <SettingsSection :title="t('transfer.sectionTitle')" :description="t('transfer.sectionHint')">
    <SettingsRow v-if="canWrite" :label="t('transfer.importTitle')" :hint="t('transfer.importRowHint')">
      <UiButton size="sm" :icon="Upload" @click="importOpen = true">{{ t('transfer.import') }}</UiButton>
    </SettingsRow>
    <SettingsRow :label="t('transfer.exportWorkspace')" :hint="t('transfer.exportRowHint')">
      <UiButton size="sm" :icon="Download" @click="exportOpen = true">{{ t('transfer.export') }}</UiButton>
    </SettingsRow>
    <DocImportDialog v-model:open="importOpen" />
    <DocExportDialog v-model:open="exportOpen" />
  </SettingsSection>
  <SettingsSection v-if="!personal" :title="t('settings.ws.danger')" danger>
    <SettingsRow :label="t('settings.ws.leave')" :hint="t('settings.ws.leaveHint')">
      <UiButton size="sm" :icon="LogOut" @click="leave">{{ t('settings.ws.leave') }}</UiButton>
    </SettingsRow>
    <SettingsRow v-if="role === 'owner'" :label="t('settings.ws.delete')" :hint="t('settings.ws.deleteHint')">
      <UiButton size="sm" variant="danger" :icon="Trash2" @click="remove">{{ t('common.delete') }}</UiButton>
    </SettingsRow>
  </SettingsSection>
</template>

<style scoped>
.moodle-row { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; justify-content: flex-end; }
.ws-select { height: 28px; min-width: 200px; padding: 0 10px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); font-size: var(--fs-sm); background: var(--bg-input); text-align: left; }
</style>
