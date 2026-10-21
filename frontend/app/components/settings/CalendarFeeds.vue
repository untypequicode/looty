<script setup lang="ts">
import { Plus, Trash2, Copy, Rss, ExternalLink } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useWorkspaceId } from '~/composables/useWorkspace'
import { useLootyCalendars } from '~/composables/useCalendar'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'

interface Token { id: string; name: string; scope: string; created_at: string; last_used_at?: string | null; calendar_id?: string | null }

const { t, relativeTime, formatDate } = useI18n()
const qc = useQueryClient()
const wsId = useWorkspaceId()
const { data: tokens } = useQuery({ queryKey: ['me', 'tokens'], queryFn: () => api<Token[]>('/api/user/tokens') })
const { data: calendars } = useLootyCalendars(wsId)
const feeds = computed(() => (tokens.value ?? []).filter((tk) => tk.scope === 'ics'))
const calendarItems = computed(() => [
  { value: '', label: t('settings.feeds.allCalendars') },
  ...(calendars.value ?? []).map((c) => ({ value: c.id, label: c.name, color: c.color })),
])
const calendarLabel = (id?: string | null) => (id ? calendars.value?.find((c) => c.id === id)?.name ?? t('settings.feeds.otherSpace') : t('settings.feeds.allCalendars'))

function copy(text: string) {
  navigator.clipboard.writeText(text)
  toast.success(t('common.copied'))
}

const createOpen = ref(false)
const name = ref('')
const calendarId = ref('')
const created = ref<string | null>(null)
const creating = ref(false)
const feedUrl = computed(() => (created.value ? `${location.origin}/api/ics/${created.value}.ics` : ''))
const webcalUrl = computed(() => feedUrl.value.replace(/^https?:\/\//, 'webcal://'))
const buttons = computed(() => [
  { key: 'google', label: 'Google Calendar', href: `https://calendar.google.com/calendar/r?cid=${encodeURIComponent(webcalUrl.value)}` },
  { key: 'apple', label: 'Apple Calendar', href: webcalUrl.value },
  { key: 'outlook', label: 'Outlook', href: `https://outlook.live.com/calendar/0/addfromweb?url=${encodeURIComponent(feedUrl.value)}&name=${encodeURIComponent(name.value || 'Looty')}` },
])

function openCreate() {
  name.value = ''
  calendarId.value = ''
  created.value = null
  createOpen.value = true
}
async function create() {
  creating.value = true
  try {
    const r = await api<{ token: string }>('/api/user/tokens', { method: 'POST', body: { name: name.value.trim(), scope: 'ics', ...(calendarId.value ? { calendar_id: calendarId.value } : {}) } })
    created.value = r.token
    qc.invalidateQueries({ queryKey: ['me', 'tokens'] })
  } finally { creating.value = false }
}
function closeCreate() { createOpen.value = false; created.value = null }
async function revoke(tk: Token) {
  if (!(await confirmDialog({ title: t('settings.feeds.revokeTitle', { name: tk.name }), message: t('settings.feeds.revokeMessage'), danger: true, confirmLabel: t('common.revoke') }))) return
  await api(`/api/user/tokens/${tk.id}`, { method: 'DELETE' })
  qc.invalidateQueries({ queryKey: ['me', 'tokens'] })
}
</script>

<template>
  <SettingsSection :title="t('settings.feeds.title')" :description="t('settings.feeds.description')">
    <template #actions><UiButton size="sm" variant="primary" :icon="Plus" @click="openCreate">{{ t('settings.feeds.new') }}</UiButton></template>
    <SettingsRow
      v-for="tk in feeds"
      :key="tk.id"
      :label="tk.name"
      :hint="`${calendarLabel(tk.calendar_id)} · ${t('settings.security.createdOn', { date: formatDate(tk.created_at) })} · ${tk.last_used_at ? t('settings.security.lastUsed', { time: relativeTime(tk.last_used_at) }) : t('settings.security.neverUsed')}`"
    >
      <UiButton size="sm" variant="ghost" :icon="Trash2" @click="revoke(tk)">{{ t('common.revoke') }}</UiButton>
    </SettingsRow>
    <div v-if="!feeds.length" class="empty"><Rss :size="16" /> {{ t('settings.feeds.empty') }}</div>
  </SettingsSection>

  <UiDialog :open="createOpen" :title="t('settings.feeds.new')" :width="520" @update:open="(v) => !v && closeCreate()">
    <template v-if="created">
      <p class="muted">{{ t('settings.feeds.copyNow') }}</p>
      <div class="token"><code>{{ feedUrl }}</code><UiButton size="sm" :icon="Copy" @click="copy(feedUrl)">{{ t('common.copy') }}</UiButton></div>
      <div class="field-label sub">{{ t('settings.feeds.addTo') }}</div>
      <div class="targets">
        <a v-for="b in buttons" :key="b.key" class="target" :href="b.href" target="_blank" rel="noopener">{{ b.label }} <ExternalLink :size="12" /></a>
      </div>
      <div class="field-label sub">Infomaniak</div>
      <ol class="steps">
        <li>{{ t('settings.feeds.infomaniak1') }}</li>
        <li>{{ t('settings.feeds.infomaniak2') }}</li>
      </ol>
    </template>
    <form v-else class="form" @submit.prevent="create">
      <div>
        <label class="field-label">{{ t('settings.feeds.name') }}</label>
        <UiInput v-model="name" :placeholder="t('settings.feeds.namePlaceholder')" maxlength="80" autofocus />
      </div>
      <div>
        <label class="field-label">{{ t('settings.feeds.calendar') }}</label>
        <UiCombobox v-model="calendarId" :items="calendarItems" :width="300">
          <button type="button" class="select">{{ calendarLabel(calendarId || null) }}</button>
        </UiCombobox>
        <p class="field-hint">{{ t('settings.feeds.calendarHint') }}</p>
      </div>
    </form>
    <template #footer>
      <UiButton v-if="created" variant="primary" @click="closeCreate">{{ t('common.done') }}</UiButton>
      <template v-else>
        <UiButton variant="ghost" @click="closeCreate">{{ t('common.cancel') }}</UiButton>
        <UiButton variant="primary" :disabled="!name.trim()" :loading="creating" @click="create">{{ t('common.create') }}</UiButton>
      </template>
    </template>
  </UiDialog>
</template>

<style scoped>
.empty { display: flex; align-items: center; gap: 8px; padding: 14px 16px; color: var(--text-3); border-top: 1px solid var(--border); }
.form { display: flex; flex-direction: column; gap: 14px; }
.token { display: flex; align-items: center; gap: 8px; padding: 6px 6px 6px 10px; border-radius: var(--r-md); background: var(--bg-code); min-width: 0; }
.token code { flex: 1; font-family: var(--font-mono); font-size: 12px; word-break: break-all; }
.sub { margin-top: 14px; }
.targets { display: flex; flex-wrap: wrap; gap: 8px; }
.target { display: inline-flex; align-items: center; gap: 6px; height: 30px; padding: 0 12px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); background: var(--bg-input); color: var(--text); font-size: var(--fs-sm); text-decoration: none; }
.target:hover { background: var(--bg-hover); }
.steps { margin: 4px 0 0; padding-left: 18px; color: var(--text-2); font-size: var(--fs-sm); display: flex; flex-direction: column; gap: 4px; }
.select { height: 30px; padding: 0 10px; min-width: 240px; max-width: 100%; text-align: left; border-radius: var(--r-sm); border: 1px solid var(--border-strong); background: var(--bg-input); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
