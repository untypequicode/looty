<script setup lang="ts">
import { Download, Trash2, Undo2 } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api, errorMessage } from '~/composables/api'
import { useAuth } from '~/composables/useAuth'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'

/** Export du compte (archive zip) et suppression du compte avec délai de grâce (ADR 0013). */
interface Deletion { scheduled_at: string | null; grace_days: number; blockers: { id: string; name: string }[]; last_super_admin: boolean }
const { t, formatDate } = useI18n()
const { user } = useAuth()
const qc = useQueryClient()
const { data } = useQuery({ queryKey: ['me', 'deletion'], queryFn: () => api<Deletion>('/api/user/deletion') })
const busy = ref(false)

function download() {
  toast(t('data.exportStarted'))
  window.location.href = '/api/user/export'
}

const blocked = computed(() => (data.value?.blockers.length ?? 0) > 0 || !!data.value?.last_super_admin)
async function requestDeletion() {
  const d = data.value
  const email = user.value?.email
  if (!d || !email) return
  const ok = await confirmDialog({
    title: t('data.deleteConfirmTitle'), message: t('data.deleteConfirmMessage', { days: d.grace_days }),
    danger: true, confirmLabel: t('data.deleteConfirmLabel'), typeToConfirm: email,
  })
  if (!ok) return
  busy.value = true
  try {
    await api('/api/user/deletion', { method: 'POST', body: { email } })
    await qc.invalidateQueries({ queryKey: ['me', 'deletion'] })
    toast.success(t('data.deleteRequested'))
  } catch (e) { toast.error(errorMessage(e)) } finally { busy.value = false }
}
async function cancelDeletion() {
  busy.value = true
  try {
    await api('/api/user/deletion', { method: 'DELETE' })
    await qc.invalidateQueries({ queryKey: ['me', 'deletion'] })
    toast.success(t('data.deleteCancelled'))
  } finally { busy.value = false }
}
</script>

<template>
  <SettingsSection :title="t('data.exportTitle')">
    <SettingsRow :label="t('data.exportTitle')" :hint="t('data.exportHint')">
      <UiButton size="sm" :icon="Download" @click="download">{{ t('data.exportButton') }}</UiButton>
    </SettingsRow>
  </SettingsSection>
  <SettingsSection v-if="data" :title="t('data.deleteTitle')" danger>
    <template v-if="data.scheduled_at">
      <SettingsRow :label="t('data.deleteScheduled', { date: formatDate(data.scheduled_at) })" :hint="t('data.deleteHint')">
        <UiButton size="sm" :icon="Undo2" :loading="busy" @click="cancelDeletion">{{ t('data.deleteCancel') }}</UiButton>
      </SettingsRow>
    </template>
    <template v-else>
      <SettingsRow :label="t('data.deleteTitle')" :hint="`${t('data.deleteHint')} ${t('data.deleteGrace', { days: data.grace_days })}`">
        <UiButton size="sm" variant="danger" :icon="Trash2" :disabled="blocked" :loading="busy" @click="requestDeletion">{{ t('data.deleteButton') }}</UiButton>
      </SettingsRow>
      <p v-if="data.blockers.length" class="warn">{{ t('data.deleteBlockers', { names: data.blockers.map((b) => `« ${b.name} »`).join(', ') }) }}</p>
      <p v-else-if="data.last_super_admin" class="warn">{{ t('data.deleteLastAdmin') }}</p>
    </template>
  </SettingsSection>
</template>

<style scoped>
.warn { margin: 0; padding: 0 16px 12px; font-size: var(--fs-sm); color: var(--orange); }
</style>
