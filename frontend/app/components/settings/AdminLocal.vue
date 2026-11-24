<script setup lang="ts">
import { TriangleAlert, Copy, UserPlus, Trash2 } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'

/** Comptes locaux : invitations, adresses à confirmer, liens de réinitialisation, double authentification imposée. */
interface Account { id: string; email: string; name: string; is_super_admin: boolean; is_suspended: boolean; verified: boolean; totp: boolean; created_at: string }
interface Invite { id: string; email: string; name: string; make_admin: boolean; expires_at: string }
interface Overview { enabled: boolean; signup: 'open' | 'closed'; require_totp: boolean; smtp: boolean; accounts: Account[]; invites: Invite[] }
const { t, formatDate } = useI18n()
const qc = useQueryClient()
const KEY = ['admin', 'local']
const { data } = useQuery({ queryKey: KEY, queryFn: () => api<Overview>('/api/admin/local') })
const refresh = () => qc.invalidateQueries({ queryKey: KEY })

const email = ref('')
const name = ref('')
const asAdmin = ref(false)
const busy = ref(false)
const link = ref<{ url: string; who: string; emailed?: boolean } | null>(null)
async function invite() {
  busy.value = true
  try {
    const r = await api<{ link: string; emailed: boolean }>('/api/admin/local/invite', { method: 'POST', body: { email: email.value, name: name.value || undefined, admin: asAdmin.value } })
    link.value = { url: r.link, who: email.value, emailed: r.emailed }
    email.value = ''; name.value = ''; asAdmin.value = false
    refresh()
  } finally { busy.value = false }
}
async function resetLink(a: Account) {
  const r = await api<{ link: string }>(`/api/admin/local/users/${a.id}/reset-link`, { method: 'POST' })
  link.value = { url: r.link, who: a.email }
}
async function verify(a: Account) { await api(`/api/admin/local/users/${a.id}/verify`, { method: 'POST' }); refresh() }
async function resetTotp(a: Account) { await api(`/api/admin/local/users/${a.id}/totp-reset`, { method: 'POST' }); refresh(); toast.success(t('localAdmin.totpReset')) }
async function cancel(i: Invite) { await api(`/api/admin/local/invites/${i.id}`, { method: 'DELETE' }); refresh() }
async function setRequire(v: boolean) { await api('/api/admin/local/settings', { method: 'PUT', body: { require_totp: v } }); refresh() }
function copy() { navigator.clipboard.writeText(link.value!.url); toast.success(t('common.linkCopied')) }
</script>

<template>
  <div class="warn" role="note"><TriangleAlert :size="16" /> {{ t('localAdmin.warning') }}</div>
  <SettingsSection v-if="data && !data.enabled">
    <SettingsRow :label="t('localAdmin.off')" :hint="t('localAdmin.offHint')" />
  </SettingsSection>
  <template v-else-if="data">
    <SettingsSection :title="t('localAdmin.access')">
      <SettingsRow :label="t('localAdmin.signup')" :hint="t('localAdmin.signupHint')"><span class="faint">{{ data.signup === 'open' ? t('localAdmin.signupOpen') : t('localAdmin.signupClosed') }}</span></SettingsRow>
      <SettingsRow :label="t('localAdmin.requireTotp')" :hint="t('localAdmin.requireTotpHint')"><UiSwitch :model-value="data.require_totp" @update:model-value="setRequire" /></SettingsRow>
      <SettingsRow :label="t('localAdmin.smtp')" :hint="data.smtp ? t('localAdmin.smtpOn') : t('localAdmin.smtpOff')" />
    </SettingsSection>

    <SettingsSection :title="t('localAdmin.invite')" :description="t('localAdmin.inviteHint')">
      <SettingsRow :label="t('local.email')"><UiInput v-model="email" type="email" style="width: 260px" /></SettingsRow>
      <SettingsRow :label="t('local.name')"><UiInput v-model="name" style="width: 260px" /></SettingsRow>
      <SettingsRow :label="t('localAdmin.asAdmin')" :hint="t('localAdmin.asAdminHint')"><UiSwitch v-model="asAdmin" /></SettingsRow>
      <SettingsRow><UiButton size="sm" variant="primary" :icon="UserPlus" :loading="busy" :disabled="!email" @click="invite">{{ t('localAdmin.createInvite') }}</UiButton></SettingsRow>
      <div v-if="link" class="link-box">
        <p>{{ link.emailed ? t('localAdmin.emailed', { email: link.who }) : t('localAdmin.shareLink', { email: link.who }) }}</p>
        <div class="link-row"><code>{{ link.url }}</code><UiButton size="sm" :icon="Copy" @click="copy">{{ t('common.copy') }}</UiButton></div>
      </div>
      <SettingsRow v-for="i in data.invites" :key="i.id" :label="i.email" :hint="t('localAdmin.pendingInvite', { date: formatDate(i.expires_at) })">
        <UiButton size="sm" variant="ghost" :icon="Trash2" :title="t('common.delete')" @click="cancel(i)" />
      </SettingsRow>
    </SettingsSection>

    <SettingsSection :title="t('localAdmin.accounts')">
      <SettingsRow v-for="a in data.accounts" :key="a.id" :label="a.name" :hint="[a.email, a.is_super_admin ? t('localAdmin.admin') : '', a.verified ? '' : t('localAdmin.unverified'), a.totp ? t('localAdmin.totpOn') : t('localAdmin.totpOff')].filter(Boolean).join(' · ')">
        <UiButton v-if="!a.verified" size="sm" @click="verify(a)">{{ t('localAdmin.verify') }}</UiButton>
        <UiButton size="sm" @click="resetLink(a)">{{ t('localAdmin.resetLink') }}</UiButton>
        <UiButton v-if="a.totp" size="sm" variant="ghost" @click="resetTotp(a)">{{ t('localAdmin.resetTotp') }}</UiButton>
      </SettingsRow>
      <SettingsRow v-if="!data.accounts.length" :label="t('localAdmin.none')" />
    </SettingsSection>
  </template>
</template>

<style scoped>
.warn { display: flex; gap: 8px; align-items: center; margin-bottom: 20px; padding: 10px 12px; border-radius: var(--r-md); background: var(--amber-soft, var(--bg-hover)); color: var(--amber, inherit); font-size: var(--fs-md); }
.link-box { padding: 12px 14px; display: flex; flex-direction: column; gap: 8px; }
.link-box p { margin: 0; }
.link-row { display: flex; gap: 8px; align-items: center; }
.link-row code { flex: 1; overflow-wrap: anywhere; font-size: var(--fs-sm); padding: 6px 8px; border-radius: var(--r-sm, 6px); background: var(--bg-hover); }
</style>
