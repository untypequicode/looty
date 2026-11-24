<script setup lang="ts">
import { KeyRound, Plus, Trash2, Copy, Link2Off, ShieldCheck } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useWorkspaces } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'

const { t, relativeTime, formatDate } = useI18n()
const qc = useQueryClient()
interface ApiToken { id: string; name: string; scope?: string; created_at: string; last_used_at?: string | null; access?: 'read' | 'write'; expires_at?: string | null; workspace_ids?: string[] | null }
const { data: tokens } = useQuery({ queryKey: ['me', 'tokens'], queryFn: () => api<ApiToken[]>('/api/user/tokens') })
const { data: workspaces } = useWorkspaces()
const spaceName = (id: string) => workspaces.value?.find((w) => w.id === id)?.name ?? '…'
/** « lecture seule · expire le … · seulement A, B » : ce qui distingue ce jeton d'un accès complet. */
function limits(tk: ApiToken) {
  const out: string[] = []
  if (tk.access === 'read') out.push(t('integ.token.readOnly'))
  if (tk.expires_at) out.push(new Date(tk.expires_at) < new Date() ? t('integ.token.expired') : t('integ.token.expiresOn', { date: formatDate(tk.expires_at) }))
  if (tk.workspace_ids?.length) out.push(t('integ.token.limitedTo', { names: tk.workspace_ids.map(spaceName).join(', ') }))
  return out
}
// Les mots de passe CalDAV sont gérés dans Calendriers.
const apiTokens = computed(() => (tokens.value ?? []).filter((tk) => tk.scope !== 'caldav' && tk.scope !== 'ics'))
const { data: identities } = useQuery({ queryKey: ['me', 'identities'], queryFn: () => api<{ provider: string; provider_name?: string | null; created_at: string }[]>('/api/user/identities') })

const createOpen = ref(false)
const tokenName = ref('')
const created = ref<string | null>(null)
const creating = ref(false)
const access = ref<'read' | 'write'>('write')
const expiryDays = ref<'0' | '30' | '90' | '365'>('0')
const limitedTo = ref<string[]>([])
function toggleSpace(id: string) {
  limitedTo.value = limitedTo.value.includes(id) ? limitedTo.value.filter((x) => x !== id) : [...limitedTo.value, id]
}
async function createToken() {
  creating.value = true
  try {
    const body = {
      name: tokenName.value.trim(), access: access.value,
      expires_in_days: expiryDays.value === '0' ? undefined : Number(expiryDays.value),
      workspace_ids: limitedTo.value.length ? limitedTo.value : undefined,
    }
    const r = await api<{ token: string }>('/api/user/tokens', { method: 'POST', body })
    created.value = r.token
    tokenName.value = ''
    access.value = 'write'; expiryDays.value = '0'; limitedTo.value = []
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
    <SettingsRow v-for="tk in apiTokens" :key="tk.id" :label="tk.name" :hint="[t('settings.security.createdOn', { date: formatDate(tk.created_at) }), tk.last_used_at ? t('settings.security.lastUsed', { time: relativeTime(tk.last_used_at) }) : t('settings.security.neverUsed'), ...limits(tk)].join(' · ')">
      <UiButton size="sm" variant="ghost" :icon="Trash2" @click="revoke(tk)">{{ t('common.revoke') }}</UiButton>
    </SettingsRow>
    <div v-if="!apiTokens.length" class="empty"><KeyRound :size="16" /> {{ t('settings.security.noTokens') }}</div>
    <div class="docs"><a href="/api/openapi.json" target="_blank" rel="noopener">{{ t('integ.token.docs') }}</a></div>
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
      <label class="field-label">{{ t('integ.token.access') }}</label>
      <UiSegmented v-model="access" size="md" :options="[{ value: 'write', label: t('integ.token.write') }, { value: 'read', label: t('integ.token.read') }]" />
      <p class="faint hint">{{ t('integ.token.accessHint') }}</p>
      <label class="field-label">{{ t('integ.token.expiry') }}</label>
      <UiSegmented v-model="expiryDays" size="md" :options="[{ value: '0', label: t('integ.token.never') }, { value: '30', label: t('integ.token.d30') }, { value: '90', label: t('integ.token.d90') }, { value: '365', label: t('integ.token.d365') }]" />
      <label class="field-label">{{ t('integ.token.workspaces') }}</label>
      <label v-for="w in (workspaces ?? []).filter((x) => x.role !== 'guest')" :key="w.id" class="ws"><input type="checkbox" :checked="limitedTo.includes(w.id)" @change="toggleSpace(w.id)"> {{ w.name }}</label>
      <p class="faint hint">{{ t('integ.token.limitHint') }}</p>
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
.docs { padding: 10px 16px; font-size: var(--fs-sm); }
.hint { font-size: var(--fs-sm); margin: 4px 0 8px; }
.ws { display: flex; align-items: center; gap: 8px; padding: 2px 0; cursor: pointer; }
</style>
