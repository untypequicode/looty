<script setup lang="ts">
import { Plus, Pencil, Trash2, Send, ScrollText, KeyRound, Copy, Webhook as WebhookIcon, Power } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api, errorMessage } from '~/composables/api'
import { useWorkspace } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'

/** Webhooks de l'espace (administrateurs) : adresse, événements choisis, secret de signature, journal des livraisons. */
interface Hook { id: string; url: string; events: string[]; active: boolean; failures: number; created_at: string }
interface Delivery { id: string; event: string; status: 'pending' | 'delivered' | 'failed'; attempts: number; response_status: number | null; error: string | null; created_at: string; next_attempt_at: string; delivered_at: string | null }

const { wsId } = useWorkspace()
const { t, relativeTime } = useI18n()
const qc = useQueryClient()
const key = computed(() => ['ws', wsId.value, 'webhooks'])
const { data } = useQuery({ queryKey: key, queryFn: () => api<{ webhooks: Hook[]; events: string[] }>(`/api/workspaces/${wsId.value}/webhooks`) })
const evName = (e: string) => t(`integ.hooks.event.${e.replace('.', '_')}`)

const open = ref(false)
const editing = ref<Hook | null>(null)
const form = reactive<{ url: string; events: string[] }>({ url: '', events: [] })
const saving = ref(false)
const secret = ref<string | null>(null)

function edit(h: Hook | null) {
  editing.value = h
  form.url = h?.url ?? ''
  form.events = h ? [...h.events] : ['task.created', 'task.completed']
  open.value = true
}
function toggle(e: string) {
  form.events = form.events.includes(e) ? form.events.filter((x) => x !== e) : [...form.events, e]
}
async function save() {
  saving.value = true
  try {
    const body = { url: form.url.trim(), events: form.events }
    if (editing.value) {
      await api(`/api/workspaces/${wsId.value}/webhooks/${editing.value.id}`, { method: 'PUT', body, silent: true })
      toast.success(t('integ.hooks.saved'))
    } else {
      secret.value = (await api<{ secret: string }>(`/api/workspaces/${wsId.value}/webhooks`, { method: 'POST', body, silent: true })).secret
    }
    open.value = false
    qc.invalidateQueries({ queryKey: key.value })
  } catch (e) {
    toast.error(errorMessage(e))
  } finally {
    saving.value = false
  }
}
async function setActive(h: Hook, active: boolean) {
  await api(`/api/workspaces/${wsId.value}/webhooks/${h.id}`, { method: 'PUT', body: { active } })
  qc.invalidateQueries({ queryKey: key.value })
}
async function remove(h: Hook) {
  if (!(await confirmDialog({ title: t('integ.hooks.deleteTitle'), message: t('integ.hooks.deleteMessage'), confirmLabel: t('common.delete'), danger: true }))) return
  await api(`/api/workspaces/${wsId.value}/webhooks/${h.id}`, { method: 'DELETE' })
  qc.invalidateQueries({ queryKey: key.value })
  toast.success(t('integ.hooks.deleted'))
}
async function rotate(h: Hook) {
  if (!(await confirmDialog({ title: t('integ.hooks.rotateConfirm'), message: t('integ.hooks.rotateMessage'), confirmLabel: t('integ.hooks.rotate'), danger: true }))) return
  secret.value = (await api<{ secret: string }>(`/api/workspaces/${wsId.value}/webhooks/${h.id}/secret`, { method: 'POST' })).secret
}
async function test(h: Hook) {
  await api(`/api/workspaces/${wsId.value}/webhooks/${h.id}/test`, { method: 'POST' })
  toast.success(t('integ.hooks.testSent'))
}

const logOf = ref<Hook | null>(null)
const { data: log } = useQuery({
  queryKey: computed(() => ['ws', wsId.value, 'webhook-log', logOf.value?.id]),
  queryFn: () => api<Delivery[]>(`/api/workspaces/${wsId.value}/webhooks/${logOf.value!.id}/deliveries`),
  enabled: computed(() => !!logOf.value),
  staleTime: 0,
})
function copySecret() { navigator.clipboard.writeText(secret.value!); toast.success(t('common.copied')) }
</script>

<template>
  <SettingsSection :description="t('integ.hooks.description')">
    <template #actions><UiButton size="sm" variant="primary" :icon="Plus" @click="edit(null)">{{ t('integ.hooks.new') }}</UiButton></template>
    <div v-if="!data?.webhooks.length" class="empty"><WebhookIcon :size="16" /> {{ t('integ.hooks.empty') }}</div>
    <SettingsRow v-for="h in data?.webhooks ?? []" :key="h.id" :label="h.url" :hint="h.active ? h.events.map(evName).join(' · ') : t('integ.hooks.offHint')">
      <UiButton size="sm" variant="ghost" :icon="Send" :disabled="!h.active" :aria-label="t('integ.hooks.test')" :title="t('integ.hooks.test')" @click="test(h)" />
      <UiButton size="sm" variant="ghost" :icon="ScrollText" :aria-label="t('integ.hooks.log')" :title="t('integ.hooks.log')" @click="logOf = h" />
      <UiButton size="sm" variant="ghost" :icon="Power" :aria-label="h.active ? t('integ.hooks.turnOff') : t('integ.hooks.turnOn')" :title="h.active ? t('integ.hooks.turnOff') : t('integ.hooks.turnOn')" @click="setActive(h, !h.active)" />
      <UiButton size="sm" variant="ghost" :icon="KeyRound" :aria-label="t('integ.hooks.rotate')" :title="t('integ.hooks.rotate')" @click="rotate(h)" />
      <UiButton size="sm" variant="ghost" :icon="Pencil" :aria-label="t('common.edit')" @click="edit(h)" />
      <UiButton size="sm" variant="ghost" :icon="Trash2" :aria-label="t('common.delete')" @click="remove(h)" />
    </SettingsRow>
  </SettingsSection>

  <UiDialog v-model:open="open" :title="editing ? t('integ.hooks.edit') : t('integ.hooks.new')" :width="520">
    <div class="form">
      <label class="field-label">{{ t('integ.hooks.url') }}</label>
      <UiInput v-model="form.url" placeholder="https://" autofocus />
      <p class="field-hint">{{ t('integ.hooks.urlHint') }}</p>
      <label class="field-label">{{ t('integ.hooks.events') }}</label>
      <label v-for="e in data?.events ?? []" :key="e" class="ev"><input type="checkbox" :checked="form.events.includes(e)" @change="toggle(e)"> {{ evName(e) }}</label>
    </div>
    <template #footer>
      <UiButton variant="ghost" @click="open = false">{{ t('common.cancel') }}</UiButton>
      <UiButton variant="primary" :loading="saving" :disabled="!form.url.trim() || !form.events.length" @click="save">{{ t('common.save') }}</UiButton>
    </template>
  </UiDialog>

  <UiDialog :open="!!secret" :title="t('integ.hooks.secretTitle')" @update:open="(v) => !v && (secret = null)">
    <p class="muted">{{ t('integ.hooks.secretHint') }}</p>
    <div class="secret"><code>{{ secret }}</code><UiButton size="sm" :icon="Copy" @click="copySecret">{{ t('common.copy') }}</UiButton></div>
    <template #footer><UiButton variant="primary" @click="secret = null">{{ t('common.done') }}</UiButton></template>
  </UiDialog>

  <UiDialog :open="!!logOf" :title="t('integ.hooks.log')" :width="620" @update:open="(v) => !v && (logOf = null)">
    <div v-if="!log?.length" class="faint">{{ t('integ.hooks.noLog') }}</div>
    <div v-for="d in log ?? []" :key="d.id" class="dl">
      <span class="dl-status" :class="d.status">{{ t(`integ.hooks.${d.status}`) }}</span>
      <span class="dl-event">{{ evName(d.event) }}</span>
      <span class="faint dl-meta">{{ t('integ.hooks.attempts', { count: d.attempts }) }}<template v-if="d.error"> · {{ d.error }}</template></span>
      <span class="faint">{{ relativeTime(d.created_at) }}</span>
    </div>
    <template #footer><UiButton variant="primary" @click="logOf = null">{{ t('common.close') }}</UiButton></template>
  </UiDialog>
</template>

<style scoped>
.empty { display: flex; align-items: center; gap: 8px; padding: 18px 16px; color: var(--text-3); }
.form { display: flex; flex-direction: column; gap: 8px; }
.ev { display: flex; align-items: center; gap: 8px; cursor: pointer; }
.secret { display: flex; align-items: center; gap: 8px; padding: 8px 8px 8px 12px; border-radius: var(--r-md); background: var(--bg-code); margin: 10px 0; }
.secret code { flex: 1; font-family: var(--font-mono); font-size: 12px; word-break: break-all; }
.dl { display: flex; align-items: center; gap: 10px; padding: 6px 0; border-top: 1px solid var(--border); }
.dl:first-child { border-top: 0; }
.dl-status { font-size: var(--fs-sm); font-weight: 500; min-width: 80px; }
.dl-status.delivered { color: var(--green); }
.dl-status.failed { color: var(--red, #eb5757); }
.dl-event { min-width: 150px; }
.dl-meta { flex: 1; font-size: var(--fs-sm); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
