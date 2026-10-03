<script setup lang="ts">
import { KeyRound, Plus, Trash2, Copy, Link2Off, ShieldCheck } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'

const { t, relativeTime, formatDate } = useI18n()
const qc = useQueryClient()
const { data: tokens } = useQuery({ queryKey: ['me', 'tokens'], queryFn: () => api<{ id: string; name: string; scope?: string; created_at: string; last_used_at?: string | null }[]>('/api/user/tokens') })
// Les mots de passe CalDAV sont gérés dans Calendriers.
const apiTokens = computed(() => (tokens.value ?? []).filter((tk) => tk.scope !== 'caldav'))
const { data: identities } = useQuery({ queryKey: ['me', 'identities'], queryFn: () => api<{ provider: string; provider_name?: string | null; created_at: string }[]>('/api/user/identities') })

const createOpen = ref(false)
const tokenName = ref('')
const created = ref<string | null>(null)
const creating = ref(false)
async function createToken() {
  creating.value = true
  try {
    const r = await api<{ token: string }>('/api/user/tokens', { method: 'POST', body: { name: tokenName.value.trim() } })
    created.value = r.token
    tokenName.value = ''
    qc.invalidateQueries({ queryKey: ['me', 'tokens'] })
  } finally { creating.value = false }
}
function closeCreate() { createOpen.value = false; created.value = null }
async function revoke(tk: { id: string; name: string }) {
  if (!(await confirmDialog({ title: t('settings.security.revokeTitle', { name: tk.name }), message: t('settings.security.revokeMessage'), danger: true, confirmLabel: t('common.revoke') }))) return
  await api(`/api/user/tokens/${tk.id}`, { method: 'DELETE' })
  qc.invalidateQueries({ queryKey: ['me', 'tokens'] })
}
async function unlink(provider: string) {
  if (!(await confirmDialog({ title: t('settings.security.unlinkTitle'), danger: true, confirmLabel: t('settings.security.unlink') }))) return
  await api(`/api/user/identities/${provider}`, { method: 'DELETE' })
  qc.invalidateQueries({ queryKey: ['me', 'identities'] })
}
function copy() { navigator.clipboard.writeText(created.value!); toast.success(t('common.copied')) }
</script>

<template>
  <SettingsSection :title="t('settings.security.accounts')" :description="t('settings.security.accountsHint')">
    <SettingsRow v-for="i in identities ?? []" :key="i.provider" :label="i.provider_name || t('settings.security.unknownProvider')" :hint="t('settings.security.linkedOn', { date: formatDate(i.created_at) })">
      <ShieldCheck :size="16" class="ok" />
      <UiButton v-if="(identities?.length ?? 0) > 1" size="sm" variant="ghost" :icon="Link2Off" @click="unlink(i.provider)">{{ t('settings.security.unlink') }}</UiButton>
    </SettingsRow>
  </SettingsSection>

  <SettingsSection :title="t('settings.security.tokens')" :description="t('settings.security.tokensHint')">
    <template #actions><UiButton size="sm" :icon="Plus" @click="createOpen = true">{{ t('settings.security.newToken') }}</UiButton></template>
    <SettingsRow v-for="tk in apiTokens" :key="tk.id" :label="tk.name" :hint="`${t('settings.security.createdOn', { date: formatDate(tk.created_at) })} · ${tk.last_used_at ? t('settings.security.lastUsed', { time: relativeTime(tk.last_used_at) }) : t('settings.security.neverUsed')}`">
      <UiButton size="sm" variant="ghost" :icon="Trash2" @click="revoke(tk)">{{ t('common.revoke') }}</UiButton>
    </SettingsRow>
    <div v-if="!apiTokens.length" class="empty"><KeyRound :size="16" /> {{ t('settings.security.noTokens') }}</div>
  </SettingsSection>

  <UiDialog :open="createOpen" :title="t('settings.security.newToken')" @update:open="(v) => !v && closeCreate()">
    <template v-if="created">
      <p class="muted">{{ t('settings.security.copyNow') }}</p>
      <div class="token"><code>{{ created }}</code><UiButton size="sm" :icon="Copy" @click="copy">{{ t('common.copy') }}</UiButton></div>
      <p class="faint usage">curl -H "Authorization: Bearer …" {{ '/api/workspaces' }}</p>
    </template>
    <form v-else @submit.prevent="createToken">
      <label class="field-label">{{ t('settings.security.tokenName') }}</label>
      <UiInput v-model="tokenName" :placeholder="t('settings.security.tokenPlaceholder')" autofocus />
    </form>
    <template #footer>
      <UiButton v-if="created" variant="primary" @click="closeCreate">{{ t('common.done') }}</UiButton>
      <template v-else>
        <UiButton variant="ghost" @click="closeCreate">{{ t('common.cancel') }}</UiButton>
        <UiButton variant="primary" :disabled="!tokenName.trim()" :loading="creating" @click="createToken">{{ t('common.create') }}</UiButton>
      </template>
    </template>
  </UiDialog>
</template>

<style scoped>
.ok { color: var(--green); }
.empty { display: flex; align-items: center; gap: 8px; padding: 18px 16px; color: var(--text-3); }
.token { display: flex; align-items: center; gap: 8px; padding: 8px 8px 8px 12px; border-radius: var(--r-md); background: var(--bg-code); margin: 10px 0; }
.token code { flex: 1; font-family: var(--font-mono); font-size: 12px; word-break: break-all; }
.usage { font-family: var(--font-mono); font-size: var(--fs-xs); }
</style>
