<script setup lang="ts">
import { Plus, Trash2, Copy, KeyRound, Smartphone } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useAuth } from '~/composables/useAuth'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'

interface Token { id: string; name: string; scope: string; created_at: string; last_used_at?: string | null }

const { t, relativeTime, formatDate } = useI18n()
const { user } = useAuth()
const qc = useQueryClient()
const { data: tokens } = useQuery({ queryKey: ['me', 'tokens'], queryFn: () => api<Token[]>('/api/user/tokens') })
const passwords = computed(() => (tokens.value ?? []).filter((tk) => tk.scope === 'caldav'))

const serverUrl = computed(() => `${location.origin}/dav/`)
function copy(text: string) {
  navigator.clipboard.writeText(text)
  toast.success(t('common.copied'))
}

const createOpen = ref(false)
const deviceName = ref('')
const created = ref<string | null>(null)
const creating = ref(false)
async function create() {
  creating.value = true
  try {
    const r = await api<{ token: string }>('/api/user/tokens', { method: 'POST', body: { name: deviceName.value.trim(), scope: 'caldav' } })
    created.value = r.token
    deviceName.value = ''
    qc.invalidateQueries({ queryKey: ['me', 'tokens'] })
  } finally { creating.value = false }
}
function closeCreate() { createOpen.value = false; created.value = null }
async function revoke(tk: Token) {
  if (!(await confirmDialog({ title: t('settings.davServer.revokeTitle', { name: tk.name }), message: t('settings.davServer.revokeMessage'), danger: true, confirmLabel: t('common.revoke') }))) return
  await api(`/api/user/tokens/${tk.id}`, { method: 'DELETE' })
  qc.invalidateQueries({ queryKey: ['me', 'tokens'] })
}
</script>

<template>
  <SettingsSection :title="t('settings.davServer.title')" :description="t('settings.davServer.description')">
    <template #actions><UiButton size="sm" variant="primary" :icon="Plus" @click="createOpen = true">{{ t('settings.davServer.newPassword') }}</UiButton></template>
    <SettingsRow :label="t('settings.davServer.server')" :hint="t('settings.davServer.serverHint')">
      <code class="val">{{ serverUrl }}</code>
      <UiButton size="sm" variant="ghost" :icon="Copy" :aria-label="t('common.copy')" @click="copy(serverUrl)" />
    </SettingsRow>
    <SettingsRow :label="t('settings.davServer.username')">
      <code class="val">{{ user?.email }}</code>
      <UiButton size="sm" variant="ghost" :icon="Copy" :aria-label="t('common.copy')" @click="copy(user?.email ?? '')" />
    </SettingsRow>
    <SettingsRow
      v-for="tk in passwords"
      :key="tk.id"
      :label="tk.name"
      :hint="`${t('settings.security.createdOn', { date: formatDate(tk.created_at) })} · ${tk.last_used_at ? t('settings.security.lastUsed', { time: relativeTime(tk.last_used_at) }) : t('settings.security.neverUsed')}`"
    >
      <UiButton size="sm" variant="ghost" :icon="Trash2" @click="revoke(tk)">{{ t('common.revoke') }}</UiButton>
    </SettingsRow>
    <div v-if="!passwords.length" class="empty"><KeyRound :size="16" /> {{ t('settings.davServer.noPasswords') }}</div>
    <div class="how">
      <div class="how-title"><Smartphone :size="14" /> {{ t('settings.davServer.howTitle') }}</div>
      <ul>
        <li>{{ t('settings.davServer.how1') }}</li>
        <li>{{ t('settings.davServer.how2') }}</li>
        <li>{{ t('settings.davServer.how3') }}</li>
        <li>{{ t('settings.davServer.how4') }}</li>
      </ul>
    </div>
  </SettingsSection>

  <UiDialog :open="createOpen" :title="t('settings.davServer.newPassword')" @update:open="(v) => !v && closeCreate()">
    <template v-if="created">
      <p class="muted">{{ t('settings.davServer.copyNow') }}</p>
      <div class="creds">
        <span class="faint">{{ t('settings.davServer.server') }}</span><code>{{ serverUrl }}</code>
        <span class="faint">{{ t('settings.davServer.username') }}</span><code>{{ user?.email }}</code>
        <span class="faint">{{ t('settings.davServer.password') }}</span>
        <div class="token"><code>{{ created }}</code><UiButton size="sm" :icon="Copy" @click="copy(created!)">{{ t('common.copy') }}</UiButton></div>
      </div>
    </template>
    <form v-else @submit.prevent="create">
      <label class="field-label">{{ t('settings.davServer.deviceName') }}</label>
      <UiInput v-model="deviceName" :placeholder="t('settings.davServer.devicePlaceholder')" maxlength="80" autofocus />
    </form>
    <template #footer>
      <UiButton v-if="created" variant="primary" @click="closeCreate">{{ t('common.done') }}</UiButton>
      <template v-else>
        <UiButton variant="ghost" @click="closeCreate">{{ t('common.cancel') }}</UiButton>
        <UiButton variant="primary" :disabled="!deviceName.trim()" :loading="creating" @click="create">{{ t('common.create') }}</UiButton>
      </template>
    </template>
  </UiDialog>
</template>

<style scoped>
.val { font-family: var(--font-mono); font-size: 12px; padding: 4px 8px; border-radius: var(--r-sm); background: var(--bg-code); max-width: 320px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.empty { display: flex; align-items: center; gap: 8px; padding: 14px 16px; color: var(--text-3); border-top: 1px solid var(--border); }
.how { padding: 14px 16px; border-top: 1px solid var(--border); color: var(--text-2); font-size: var(--fs-sm); }
.how-title { display: flex; align-items: center; gap: 6px; font-weight: 500; color: var(--text); margin-bottom: 6px; }
.how ul { margin: 0; padding-left: 18px; display: flex; flex-direction: column; gap: 4px; }
.creds { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 8px 12px; align-items: center; margin-top: 10px; }
.creds code { font-family: var(--font-mono); font-size: 12px; word-break: break-all; }
.token { display: flex; align-items: center; gap: 8px; padding: 6px 6px 6px 10px; border-radius: var(--r-md); background: var(--bg-code); min-width: 0; }
.token code { flex: 1; }
@media (max-width: 639px) { .val { max-width: 170px; } }
</style>
