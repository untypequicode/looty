<script setup lang="ts">
import { ShieldCheck, KeyRound } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api, errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'

/** Mot de passe et double authentification d'un compte local (cachée pour les comptes OpenID Connect). */
interface Sec { enabled: boolean; local: boolean; totp?: boolean; recovery_left?: number; totp_required?: boolean }
const { t } = useI18n()
const qc = useQueryClient()
const KEY = ['me', 'local-security']
const { data } = useQuery({ queryKey: KEY, queryFn: () => api<Sec>('/api/auth/local/security', { silent: true }) })

const current = ref('')
const next = ref('')
const pwBusy = ref(false)
async function changePassword() {
  pwBusy.value = true
  try {
    await api('/api/auth/local/password', { method: 'POST', body: { current: current.value, new: next.value } })
    current.value = ''; next.value = ''
    toast.success(t('local.sec.passwordChanged'))
  } finally { pwBusy.value = false }
}

const enrolling = ref(false)
function enrolled() { enrolling.value = false; qc.invalidateQueries({ queryKey: KEY }) }

// Retrait du second facteur et nouveaux codes de secours : mot de passe et code.
const action = ref<'' | 'disable' | 'codes'>('')
const password = ref('')
const code = ref('')
const busy = ref(false)
const newCodes = ref<string[] | null>(null)
async function confirmAction() {
  busy.value = true
  try {
    if (action.value === 'disable') {
      await api('/api/auth/local/totp/disable', { method: 'POST', body: { password: password.value, code: code.value } })
      toast.success(t('local.sec.totpOff'))
    } else {
      const r = await api<{ recovery_codes: string[] }>('/api/auth/local/recovery-codes', { method: 'POST', body: { password: password.value, code: code.value } })
      newCodes.value = r.recovery_codes
    }
    action.value = ''; password.value = ''; code.value = ''
    qc.invalidateQueries({ queryKey: KEY })
  } catch (e) { toast.error(errorMessage(e)) } finally { busy.value = false }
}
</script>

<template>
  <template v-if="data?.enabled && data.local">
    <SettingsSection :title="t('local.sec.password')">
      <SettingsRow :label="t('local.sec.current')"><UiInput v-model="current" type="password" autocomplete="current-password" style="width: 240px" /></SettingsRow>
      <SettingsRow :label="t('local.newPassword')" :hint="t('local.passwordHint')"><UiInput v-model="next" type="password" autocomplete="new-password" style="width: 240px" /></SettingsRow>
      <SettingsRow><UiButton size="sm" variant="primary" :icon="KeyRound" :loading="pwBusy" :disabled="!current || !next" @click="changePassword">{{ t('local.sec.change') }}</UiButton></SettingsRow>
    </SettingsSection>
    <SettingsSection :title="t('local.sec.totp')" :description="t('local.sec.totpHint')">
      <template v-if="enrolling"><div class="box"><AuthTotpEnrol @done="enrolled" /></div></template>
      <template v-else-if="!data.totp">
        <SettingsRow :label="t('local.sec.totpOffLabel')" :hint="data.totp_required ? t('local.sec.required') : undefined">
          <UiButton size="sm" variant="primary" :icon="ShieldCheck" @click="enrolling = true">{{ t('local.sec.enable') }}</UiButton>
        </SettingsRow>
      </template>
      <template v-else>
        <SettingsRow :label="t('local.sec.totpOnLabel')" :hint="t('local.sec.recoveryLeft', { count: data.recovery_left ?? 0 })">
          <UiButton size="sm" @click="action = 'codes'">{{ t('local.sec.newCodes') }}</UiButton>
          <UiButton v-if="!data.totp_required" size="sm" variant="ghost" @click="action = 'disable'">{{ t('local.sec.disable') }}</UiButton>
        </SettingsRow>
        <div v-if="data.totp_required" class="note faint">{{ t('local.sec.required') }}</div>
      </template>
      <div v-if="action" class="box">
        <p class="faint">{{ t('local.sec.confirmHint') }}</p>
        <UiInput v-model="password" type="password" autocomplete="current-password" :placeholder="t('local.password')" />
        <UiInput v-model="code" autocomplete="one-time-code" :placeholder="t('local.code')" />
        <div class="row">
          <UiButton size="sm" variant="primary" :loading="busy" :disabled="!password || !code" @click="confirmAction">{{ t('common.confirm') }}</UiButton>
          <UiButton size="sm" variant="ghost" @click="action = ''">{{ t('common.cancel') }}</UiButton>
        </div>
      </div>
      <div v-if="newCodes" class="box">
        <p class="faint">{{ t('local.recoveryHint') }}</p>
        <ul class="codes"><li v-for="c in newCodes" :key="c">{{ c }}</li></ul>
        <UiButton size="sm" @click="newCodes = null">{{ t('local.savedCodes') }}</UiButton>
      </div>
    </SettingsSection>
  </template>
</template>

<style scoped>
.box { display: flex; flex-direction: column; gap: 8px; padding: 14px; max-width: 420px; }
.note { padding: 0 14px 12px; font-size: var(--fs-sm); }
.row { display: flex; gap: 8px; }
.codes { list-style: none; margin: 0; padding: 12px; display: grid; grid-template-columns: 1fr 1fr; gap: 6px 12px; font-family: var(--font-mono, monospace); border-radius: var(--r-md); background: var(--bg-hover); }
p { margin: 0; }
</style>
