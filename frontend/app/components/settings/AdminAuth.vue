<script setup lang="ts">
import { Plus, Pencil, Trash2, ShieldCheck } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'

interface Provider { id: string; name: string; issuer_url: string; client_id: string; has_secret: boolean; is_active: boolean; user_count: number }
const { t } = useI18n()
const qc = useQueryClient()
const { data: providers } = useQuery({ queryKey: ['admin', 'providers'], queryFn: () => api<Provider[]>('/api/admin/providers') })
const callbackBase = computed(() => `${location.origin}/api/auth/callback/`)

const open = ref(false)
const editing = ref<Provider | null>(null)
const form = reactive({ name: '', issuer_url: '', client_id: '', client_secret: '', is_active: true })
const saving = ref(false)
function openEdit(p?: Provider) {
  editing.value = p ?? null
  Object.assign(form, p ? { name: p.name, issuer_url: p.issuer_url, client_id: p.client_id, client_secret: '', is_active: p.is_active } : { name: '', issuer_url: 'https://', client_id: '', client_secret: '', is_active: true })
  open.value = true
}
async function save() {
  saving.value = true
  try {
    const body = { ...form, client_secret: form.client_secret || null }
    if (editing.value) await api(`/api/admin/providers/${editing.value.id}`, { method: 'PUT', body })
    else await api('/api/admin/providers', { method: 'POST', body })
    qc.invalidateQueries({ queryKey: ['admin', 'providers'] })
    open.value = false
    toast.success(t('common.saved'))
  } finally { saving.value = false }
}
async function remove(p: Provider) {
  if (!(await confirmDialog({ title: t('settings.auth.deleteTitle', { name: p.name }), message: p.user_count ? t('settings.auth.deleteUsers', { count: p.user_count }) : undefined, danger: true, confirmLabel: t('common.delete') }))) return
  await api(`/api/admin/providers/${p.id}`, { method: 'DELETE' })
  qc.invalidateQueries({ queryKey: ['admin', 'providers'] })
}
async function toggle(p: Provider, active: boolean) {
  await api(`/api/admin/providers/${p.id}`, { method: 'PUT', body: { name: p.name, issuer_url: p.issuer_url, client_id: p.client_id, client_secret: null, is_active: active } })
  qc.invalidateQueries({ queryKey: ['admin', 'providers'] })
}
</script>

<template>
  <SettingsSection :title="t('settings.auth.title')" :description="t('settings.auth.description')">
    <template #actions><UiButton size="sm" variant="primary" :icon="Plus" @click="openEdit()">{{ t('settings.auth.add') }}</UiButton></template>
    <SettingsRow v-for="p in providers ?? []" :key="p.id">
      <template #text>
        <div class="p-name"><ShieldCheck :size="15" :class="p.is_active ? 'ok' : 'faint'" /> {{ p.name }}</div>
        <div class="p-meta">{{ p.issuer_url }} · {{ t('settings.auth.users', { count: p.user_count }) }}</div>
        <div class="p-cb mono">{{ callbackBase }}{{ p.id }}</div>
      </template>
      <UiSwitch :model-value="p.is_active" :label="t('settings.auth.active')" @update:model-value="(v) => toggle(p, v)" />
      <UiButton size="sm" variant="ghost" :icon="Pencil" @click="openEdit(p)" />
      <UiButton size="sm" variant="ghost" :icon="Trash2" @click="remove(p)" />
    </SettingsRow>
  </SettingsSection>

  <UiDialog v-model:open="open" :title="editing ? t('settings.auth.edit') : t('settings.auth.add')" :width="500">
    <form class="form" @submit.prevent="save">
      <div><label class="field-label">{{ t('settings.auth.name') }}</label><UiInput v-model="form.name" placeholder="Zitadel, Keycloak, Google…" /></div>
      <div><label class="field-label">{{ t('settings.auth.issuer') }}</label><UiInput v-model="form.issuer_url" placeholder="https://auth.example.com" /></div>
      <div><label class="field-label">Client ID</label><UiInput v-model="form.client_id" /></div>
      <div>
        <label class="field-label">Client secret</label>
        <UiInput v-model="form.client_secret" type="password" autocomplete="new-password" :placeholder="editing?.has_secret ? t('settings.auth.secretKeep') : ''" />
      </div>
      <label class="opt"><UiSwitch v-model="form.is_active" /> {{ t('settings.auth.active') }}</label>
      <p v-if="editing" class="field-hint">{{ t('settings.auth.callbackHint') }} <code>{{ callbackBase }}{{ editing.id }}</code></p>
    </form>
    <template #footer>
      <UiButton variant="ghost" @click="open = false">{{ t('common.cancel') }}</UiButton>
      <UiButton variant="primary" :loading="saving" :disabled="!form.name || !form.issuer_url || !form.client_id || (!editing && !form.client_secret)" @click="save">{{ t('common.save') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.p-name { display: flex; align-items: center; gap: 8px; font-weight: 500; }
.p-meta { color: var(--text-3); font-size: var(--fs-sm); margin-top: 2px; word-break: break-all; }
.p-cb { color: var(--text-3); font-size: var(--fs-xs); margin-top: 4px; word-break: break-all; }
.ok { color: var(--green); }
.form { display: flex; flex-direction: column; gap: 12px; }
.opt { display: flex; align-items: center; gap: 10px; cursor: pointer; }
code { font-family: var(--font-mono); font-size: 11.5px; word-break: break-all; }
</style>
