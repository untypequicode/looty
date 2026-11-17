<script setup lang="ts">
import { Plus, Trash2, Pencil, Bot, ShieldCheck, ChevronDown } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { aiApi, AI_PROVIDERS, type AiConnection, type AiProvider } from '~/composables/useAi'

const { t } = useI18n()
const qc = useQueryClient()
const { data: connections } = useQuery({ queryKey: ['ai', 'connections'], queryFn: aiApi.connections })

const open = ref(false)
const providerOpen = ref(false)
const providerItems = AI_PROVIDERS.map((p) => ({ value: p.value, label: p.label }))
const editing = ref<AiConnection | null>(null)
const saving = ref(false)
const form = reactive({ provider: 'openai' as AiProvider, label: '', model: '', base_url: '', api_key: '' })
const needsUrl = computed(() => form.provider === 'openai_compatible')
const providerLabel = (p: AiProvider) => AI_PROVIDERS.find((x) => x.value === p)?.label ?? p

function start(c?: AiConnection) {
  editing.value = c ?? null
  const def = AI_PROVIDERS[0]!
  Object.assign(form, c
    ? { provider: c.provider, label: c.label, model: c.model, base_url: c.base_url ?? '', api_key: '' }
    : { provider: def.value, label: '', model: def.model, base_url: '', api_key: '' })
  open.value = true
}
function pickProvider(p: AiProvider) {
  form.provider = p
  const def = AI_PROVIDERS.find((x) => x.value === p)!
  form.model = def.model
  form.base_url = def.baseUrl ?? ''
}
const valid = computed(() => !!form.label.trim() && !!form.model.trim() && (!needsUrl.value || !!form.base_url.trim()) && (editing.value || needsUrl.value || !!form.api_key.trim()))

async function save() {
  saving.value = true
  try {
    const body = { label: form.label, model: form.model, base_url: form.base_url, api_key: form.api_key }
    if (editing.value) await api(`/api/ai/connections/${editing.value.id}`, { method: 'PATCH', body })
    else await api('/api/ai/connections', { method: 'POST', body: { ...body, provider: form.provider } })
    form.api_key = ''
    open.value = false
    qc.invalidateQueries({ queryKey: ['ai', 'connections'] })
  } finally { saving.value = false }
}
async function remove(c: AiConnection) {
  if (!(await confirmDialog({ title: t('ai.settings.deleteTitle', { name: c.label }), message: t('ai.settings.deleteMessage'), danger: true, confirmLabel: t('common.delete') }))) return
  await api(`/api/ai/connections/${c.id}`, { method: 'DELETE' })
  qc.invalidateQueries({ queryKey: ['ai', 'connections'] })
}
</script>

<template>
  <SettingsSection :title="t('ai.settings.title')" :description="t('ai.settings.description')">
    <template #actions><UiButton size="sm" variant="primary" :icon="Plus" @click="start()">{{ t('ai.settings.add') }}</UiButton></template>
    <SettingsRow v-for="c in connections" :key="c.id" :label="c.label" :hint="`${providerLabel(c.provider)} · ${c.model}${c.base_url ? ` · ${c.base_url}` : ''}${c.key_hint ? ` · ${c.key_hint}` : ''}`">
      <UiButton size="sm" variant="ghost" :icon="Pencil" :aria-label="t('common.edit')" @click="start(c)" />
      <UiButton size="sm" variant="ghost" :icon="Trash2" :aria-label="t('common.delete')" @click="remove(c)" />
    </SettingsRow>
    <div v-if="!connections?.length" class="empty"><Bot :size="16" /> {{ t('ai.settings.none') }}</div>
    <div class="how"><ShieldCheck :size="14" /> {{ t('ai.settings.privacy') }}</div>
  </SettingsSection>

  <UiDialog :open="open" :title="editing ? t('ai.settings.edit') : t('ai.settings.add')" @update:open="(v) => (open = v)">
    <form class="form" @submit.prevent="valid && save()">
      <template v-if="!editing">
        <label class="field-label">{{ t('ai.settings.provider') }}</label>
        <UiCombobox v-model:open="providerOpen" :model-value="form.provider" :items="providerItems" :width="320" @update:model-value="(v) => pickProvider(v as AiProvider)">
          <button type="button" class="sel"><span class="truncate">{{ providerLabel(form.provider) }}</span><ChevronDown :size="14" class="faint" /></button>
        </UiCombobox>
      </template>
      <label class="field-label">{{ t('ai.settings.label') }}</label>
      <UiInput v-model="form.label" maxlength="80" :placeholder="t('ai.settings.labelPlaceholder')" />
      <label class="field-label">{{ t('ai.settings.model') }}</label>
      <UiInput v-model="form.model" maxlength="120" />
      <label class="field-label">{{ t('ai.settings.baseUrl') }}{{ needsUrl ? '' : ` (${t('ai.settings.optional')})` }}</label>
      <UiInput v-model="form.base_url" maxlength="300" placeholder="https://…" />
      <label class="field-label">{{ t('ai.settings.apiKey') }}{{ needsUrl ? ` (${t('ai.settings.optional')})` : '' }}</label>
      <UiInput v-model="form.api_key" type="password" autocomplete="off" :placeholder="editing ? t('ai.settings.keepKey') : ''" />
    </form>
    <template #footer>
      <UiButton variant="ghost" @click="open = false">{{ t('common.cancel') }}</UiButton>
      <UiButton variant="primary" :disabled="!valid || saving" @click="save">{{ t('common.save') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.empty, .how { display: flex; align-items: center; gap: 8px; padding: 14px 16px; color: var(--text-3); font-size: var(--fs-sm); }
.how { border-top: 1px solid var(--border); line-height: 1.45; }
.form { display: flex; flex-direction: column; gap: 6px; }
.field-label { font-size: var(--fs-sm); color: var(--text-2); font-weight: 500; margin-top: 6px; }
.sel { display: flex; align-items: center; justify-content: space-between; gap: 8px; width: 100%; height: 34px; padding: 0 10px; border: 1px solid var(--border); border-radius: var(--r-sm); background: var(--bg-input); color: var(--text); }
</style>
