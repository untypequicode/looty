<script setup lang="ts">
import { Send, Info } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api, errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'

interface MailSettings {
  enabled: boolean; host: string; port: number; security: 'starttls' | 'tls' | 'none'; username: string; has_password: boolean
  from_address: string; from_name: string; delay_minutes: number; env_configured: boolean
}
const { t } = useI18n()
const qc = useQueryClient()
const KEY = ['admin', 'mail']
const { data } = useQuery({ queryKey: KEY, queryFn: () => api<MailSettings>('/api/admin/mail') })
const form = reactive({ enabled: false, host: '', port: 587, security: 'starttls' as MailSettings['security'], username: '', password: '', from_address: '', from_name: 'Looty', delay_minutes: 2 })
const passwordTouched = ref(false)
watch(data, (d) => { if (d) Object.assign(form, { ...d, password: '' }); passwordTouched.value = false }, { immediate: true })

const DEFAULT_PORTS = { starttls: 587, tls: 465, none: 25 }
function setSecurity(s: MailSettings['security']) {
  if (form.port === DEFAULT_PORTS[form.security]) form.port = DEFAULT_PORTS[s]
  form.security = s
}

const saving = ref(false)
const testing = ref(false)
async function save() {
  saving.value = true
  try {
    const body = { ...form, port: Number(form.port), delay_minutes: Number(form.delay_minutes), password: passwordTouched.value ? form.password : undefined }
    qc.setQueryData(KEY, await api<MailSettings>('/api/admin/mail', { method: 'PUT', body }))
    toast.success(t('common.saved'))
  } finally { saving.value = false }
}
async function test() {
  testing.value = true
  try {
    const r = await api<{ sent_to: string }>('/api/admin/mail/test', { method: 'POST', silent: true })
    toast.success(t('settings.mail.testSent', { email: r.sent_to }))
  } catch (e: any) {
    // Le détail SMTP (« smtp_send_failed: … ») aide à diagnostiquer.
    const detail = String(e?.data?.error ?? '').split(':').slice(1).join(':').trim()
    toast.error(detail ? `${errorMessage(e)} — ${detail}` : errorMessage(e))
  } finally { testing.value = false }
}
</script>

<template>
  <SettingsSection :title="t('settings.mail.title')" :description="t('settings.mail.description')">
    <template #actions><UiButton size="sm" :icon="Send" :loading="testing" @click="test">{{ t('settings.mail.test') }}</UiButton></template>
    <div v-if="data?.env_configured && !form.enabled" class="info"><Info :size="14" /> {{ t('settings.mail.envActive') }}</div>
    <SettingsRow :label="t('settings.mail.enabled')" :hint="t('settings.mail.enabledHint')"><UiSwitch v-model="form.enabled" /></SettingsRow>
    <SettingsRow :label="t('settings.mail.host')"><UiInput v-model="form.host" placeholder="smtp.example.com" style="width: 260px" autocomplete="off" /></SettingsRow>
    <SettingsRow :label="t('settings.mail.security')" :hint="t('settings.mail.securityHint')">
      <UiSegmented :model-value="form.security" size="md" :options="[{ value: 'starttls', label: 'STARTTLS' }, { value: 'tls', label: 'TLS' }, { value: 'none', label: t('settings.mail.none') }]" @update:model-value="setSecurity" />
    </SettingsRow>
    <SettingsRow :label="t('settings.mail.port')"><UiInput v-model.number="form.port" type="number" min="1" max="65535" style="width: 110px" /></SettingsRow>
    <SettingsRow :label="t('settings.mail.username')"><UiInput v-model="form.username" style="width: 260px" autocomplete="off" /></SettingsRow>
    <SettingsRow :label="t('settings.mail.password')" :hint="data?.has_password ? t('settings.mail.passwordKept') : undefined">
      <UiInput v-model="form.password" type="password" :placeholder="data?.has_password ? '••••••••' : ''" style="width: 260px" autocomplete="new-password" @input="passwordTouched = true" />
    </SettingsRow>
    <SettingsRow :label="t('settings.mail.from')" :hint="t('settings.mail.fromHint')"><UiInput v-model="form.from_address" type="email" placeholder="looty@example.com" style="width: 260px" /></SettingsRow>
    <SettingsRow :label="t('settings.mail.fromName')"><UiInput v-model="form.from_name" maxlength="120" style="width: 260px" /></SettingsRow>
    <SettingsRow :label="t('settings.mail.delay')" :hint="t('settings.mail.delayHint')"><UiInput v-model.number="form.delay_minutes" type="number" min="0" max="1440" style="width: 110px" /></SettingsRow>
  </SettingsSection>
  <div class="actions"><UiButton variant="primary" :loading="saving" @click="save">{{ t('common.save') }}</UiButton></div>
</template>

<style scoped>
.info { display: flex; align-items: center; gap: 8px; padding: 10px 16px; color: var(--text-2); font-size: var(--fs-sm); border-bottom: 1px solid var(--border); background: var(--accent-soft); }
.actions { display: flex; justify-content: flex-end; margin-top: 14px; }
</style>
