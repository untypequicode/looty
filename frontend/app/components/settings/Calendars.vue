<script setup lang="ts">
import { CalendarSync, Plus, RefreshCw, MoreHorizontal, Trash2, Pencil, Pause, Play, AlertTriangle, Check, ChevronLeft, CalendarCheck2 } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { api, errorMessage } from '~/composables/api'
import { useWorkspace } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'

interface Account {
  id: string; name: string; server_url: string; calendar_url: string; calendar_name?: string | null; calendar_color?: string | null
  username: string; board_ids?: string[] | null; include_completed: boolean; enabled: boolean
  last_sync_at?: string | null; last_error?: string | null; task_count: number
}
interface Discovered { url: string; name: string; color?: string | null; supports_todo: boolean }

const { t, relativeTime } = useI18n()
const qc = useQueryClient()
const { boards } = useWorkspace()
const KEY = ['me', 'caldav']
const { data: accounts, isLoading } = useQuery({ queryKey: KEY, queryFn: () => api<Account[]>('/api/user/caldav'), refetchInterval: 30_000 })
const refresh = () => qc.invalidateQueries({ queryKey: KEY })

// --- Assistant de connexion ---
const PRESETS = [
  { key: 'nextcloud', label: 'Nextcloud', url: 'https://cloud.example.com/remote.php/dav', hint: 'settings.caldav.presets.nextcloud' },
  { key: 'icloud', label: 'iCloud', url: 'https://caldav.icloud.com', hint: 'settings.caldav.presets.icloud' },
  { key: 'fastmail', label: 'Fastmail', url: 'https://caldav.fastmail.com', hint: 'settings.caldav.presets.fastmail' },
  { key: 'other', label: t('settings.caldav.other'), url: '', hint: 'settings.caldav.presets.other' },
]
const open = ref(false)
const step = ref<1 | 2>(1)
const preset = ref('nextcloud')
const form = reactive({ server_url: '', username: '', password: '' })
const calendars = ref<Discovered[]>([])
const chosen = ref<string | null>(null)
const name = ref('')
const boardFilter = ref<string[]>([])
const includeCompleted = ref(true)
const busy = ref(false)
const error = ref('')

function openWizard() {
  step.value = 1
  preset.value = 'nextcloud'
  Object.assign(form, { server_url: '', username: '', password: '' })
  calendars.value = []
  chosen.value = null
  boardFilter.value = []
  includeCompleted.value = true
  error.value = ''
  open.value = true
}
const presetInfo = computed(() => PRESETS.find((p) => p.key === preset.value)!)

async function findCalendars() {
  busy.value = true
  error.value = ''
  try {
    calendars.value = await api<Discovered[]>('/api/user/caldav/discover', { method: 'POST', body: { ...form }, silent: true })
    const first = calendars.value.find((c) => c.supports_todo)
    chosen.value = first?.url ?? null
    name.value = first?.name ?? presetInfo.value.label
    step.value = 2
  } catch (e) {
    error.value = errorMessage(e)
  } finally {
    busy.value = false
  }
}
watch(chosen, (u) => { const c = calendars.value.find((x) => x.url === u); if (c) name.value = c.name })

async function connect() {
  const cal = calendars.value.find((c) => c.url === chosen.value)
  if (!cal) return
  busy.value = true
  error.value = ''
  try {
    await api('/api/user/caldav', {
      method: 'POST',
      silent: true,
      body: { ...form, name: name.value.trim() || cal.name, calendar_url: cal.url, calendar_name: cal.name, calendar_color: cal.color ?? null, board_ids: boardFilter.value.length ? boardFilter.value : null, include_completed: includeCompleted.value },
    })
    open.value = false
    toast.success(t('settings.caldav.connected'))
    refresh()
    setTimeout(refresh, 4000)
  } catch (e) {
    error.value = errorMessage(e)
  } finally {
    busy.value = false
  }
}

// --- Actions ---
const syncing = ref<Record<string, boolean>>({})
async function syncNow(a: Account) {
  syncing.value = { ...syncing.value, [a.id]: true }
  try {
    const r = await api<{ report: { pushed: number; pulled: number; deleted: number } }>(`/api/user/caldav/${a.id}/sync`, { method: 'POST' })
    toast.success(t('settings.caldav.synced', { pushed: r.report.pushed, pulled: r.report.pulled }))
  } finally {
    syncing.value = { ...syncing.value, [a.id]: false }
    refresh()
  }
}
async function update(a: Account, body: Record<string, unknown>) {
  await api(`/api/user/caldav/${a.id}`, { method: 'PUT', body })
  refresh()
}
async function disconnect(a: Account) {
  const ok = await confirmDialog({ title: t('settings.caldav.disconnectTitle', { name: a.name }), message: t('settings.caldav.disconnectMessage'), danger: true, confirmLabel: t('settings.caldav.disconnect') })
  if (!ok) return
  await api(`/api/user/caldav/${a.id}`, { method: 'DELETE' })
  refresh()
}

// Édition du filtre de tableaux
const editing = ref<Account | null>(null)
const editBoards = ref<string[]>([])
const editIncludeCompleted = ref(true)
const editPassword = ref('')
function openEdit(a: Account) {
  editing.value = a
  editBoards.value = a.board_ids ?? []
  editIncludeCompleted.value = a.include_completed
  editPassword.value = ''
}
async function saveEdit() {
  if (!editing.value) return
  await update(editing.value, { board_ids: editBoards.value.length ? editBoards.value : null, include_completed: editIncludeCompleted.value, ...(editPassword.value ? { password: editPassword.value } : {}) })
  editing.value = null
  toast.success(t('common.saved'))
}
const editOpen = computed({ get: () => !!editing.value, set: (v) => { if (!v) editing.value = null } })

const menus = ref<Record<string, boolean>>({})
function items(a: Account): MenuItem[] {
  return [
    { label: t('settings.caldav.editSync'), icon: Pencil, onSelect: () => openEdit(a) },
    a.enabled ? { label: t('settings.caldav.pause'), icon: Pause, onSelect: () => update(a, { enabled: false }) } : { label: t('settings.caldav.resume'), icon: Play, onSelect: () => update(a, { enabled: true }) },
    { divider: true },
    { label: t('settings.caldav.disconnect'), icon: Trash2, danger: true, onSelect: () => disconnect(a) },
  ]
}
const boardItems = computed(() => (boards.data.value ?? []).map((b) => ({ value: b.id, label: b.name })))
const boardsLabel = (ids?: string[] | null) => (!ids?.length ? t('settings.caldav.allBoards') : ids.map((id) => boards.data.value?.find((b) => b.id === id)?.name).filter(Boolean).join(', ') || t('settings.caldav.otherWorkspace'))
const host = (u: string) => { try { return new URL(u).host } catch { return u } }
</script>

<template>
  <SettingsSection :description="t('settings.caldav.description')">
    <template #actions><UiButton size="sm" variant="primary" :icon="Plus" @click="openWizard">{{ t('settings.caldav.connect') }}</UiButton></template>
    <div v-if="isLoading" class="skeleton" style="height: 64px; margin: 12px" />
    <div v-for="a in accounts ?? []" :key="a.id" class="acc" :class="{ paused: !a.enabled }">
      <span class="acc-dot" :style="{ background: a.calendar_color || 'var(--accent)' }" />
      <div class="acc-body">
        <div class="acc-name">{{ a.name }} <span v-if="!a.enabled" class="badge">{{ t('settings.caldav.paused') }}</span></div>
        <div class="acc-meta truncate">{{ a.calendar_name || a.calendar_url }} · {{ a.username }}@{{ host(a.server_url) }}</div>
        <div class="acc-meta">{{ t('settings.caldav.taskCount', { count: a.task_count }) }} · {{ boardsLabel(a.board_ids) }}</div>
        <div v-if="a.last_error" class="acc-error"><AlertTriangle :size="12" /> {{ errorMessage({ data: { error: a.last_error.split(':')[0] } }) }} <span class="faint">({{ a.last_error }})</span></div>
        <div v-else-if="a.last_sync_at" class="acc-ok"><Check :size="12" /> {{ t('settings.caldav.lastSync', { time: relativeTime(a.last_sync_at) }) }}</div>
      </div>
      <UiButton size="sm" :icon="RefreshCw" :loading="syncing[a.id]" :disabled="!a.enabled" @click="syncNow(a)">{{ t('settings.caldav.syncNow') }}</UiButton>
      <UiMenu v-model:open="menus[a.id]" :items="items(a)" placement="bottom-end"><UiButton size="sm" variant="ghost" :icon="MoreHorizontal" /></UiMenu>
    </div>
    <div v-if="!isLoading && !accounts?.length" class="empty">
      <CalendarSync :size="22" />
      <div>
        <div class="empty-title">{{ t('settings.caldav.emptyTitle') }}</div>
        <div class="faint">{{ t('settings.caldav.emptyHint') }}</div>
      </div>
    </div>
  </SettingsSection>

  <SettingsSection :title="t('settings.caldav.howTitle')">
    <ul class="how">
      <li>{{ t('settings.caldav.how1') }}</li>
      <li>{{ t('settings.caldav.how2') }}</li>
      <li>{{ t('settings.caldav.how3') }}</li>
      <li>{{ t('settings.caldav.how4') }}</li>
    </ul>
  </SettingsSection>

  <!-- Assistant -->
  <UiDialog v-model:open="open" :title="t('settings.caldav.connect')" :description="step === 1 ? t('settings.caldav.step1') : t('settings.caldav.step2')" :width="520">
    <form v-if="step === 1" class="form" @submit.prevent="findCalendars">
      <UiSegmented v-model="preset" size="md" :options="PRESETS.map((p) => ({ value: p.key, label: p.label }))" @update:model-value="(v) => (form.server_url = PRESETS.find((p) => p.key === v)!.url.includes('example') ? '' : PRESETS.find((p) => p.key === v)!.url)" />
      <p class="preset-hint">{{ t(presetInfo.hint) }}</p>
      <div><label class="field-label">{{ t('settings.caldav.serverUrl') }}</label><UiInput v-model="form.server_url" :placeholder="presetInfo.url || 'https://dav.example.com/'" autofocus /></div>
      <div><label class="field-label">{{ t('settings.caldav.username') }}</label><UiInput v-model="form.username" autocomplete="username" /></div>
      <div><label class="field-label">{{ t('settings.caldav.password') }}</label><UiInput v-model="form.password" type="password" autocomplete="current-password" /><p class="field-hint">{{ t('settings.caldav.passwordHint') }}</p></div>
      <p v-if="error" class="error"><AlertTriangle :size="13" /> {{ error }}</p>
      <button type="submit" hidden />
    </form>
    <div v-else class="form">
      <div class="cals">
        <label v-for="c in calendars" :key="c.url" class="cal" :class="{ disabled: !c.supports_todo, on: chosen === c.url }">
          <input v-model="chosen" type="radio" :value="c.url" :disabled="!c.supports_todo" />
          <span class="acc-dot" :style="{ background: c.color || 'var(--accent)' }" />
          <span class="cal-name truncate">{{ c.name }}</span>
          <span v-if="c.supports_todo" class="cal-tag"><CalendarCheck2 :size="12" /> {{ t('settings.caldav.supportsTodo') }}</span>
          <span v-else class="cal-tag faint">{{ t('settings.caldav.noTodo') }}</span>
        </label>
        <p v-if="!calendars.some((c) => c.supports_todo)" class="error"><AlertTriangle :size="13" /> {{ t('settings.caldav.noTodoCalendar') }}</p>
      </div>
      <div><label class="field-label">{{ t('settings.caldav.displayName') }}</label><UiInput v-model="name" maxlength="120" /></div>
      <div>
        <label class="field-label">{{ t('settings.caldav.boards') }}</label>
        <UiCombobox v-model="boardFilter" :items="boardItems" multiple :width="300">
          <button class="select">{{ boardsLabel(boardFilter) }}</button>
        </UiCombobox>
        <p class="field-hint">{{ t('settings.caldav.boardsHint') }}</p>
      </div>
      <label class="opt"><UiSwitch v-model="includeCompleted" /> {{ t('settings.caldav.includeCompleted') }}</label>
      <p v-if="error" class="error"><AlertTriangle :size="13" /> {{ error }}</p>
    </div>
    <template #footer>
      <UiButton v-if="step === 2" variant="ghost" :icon="ChevronLeft" @click="step = 1">{{ t('common.back') }}</UiButton>
      <span class="spacer" />
      <UiButton v-if="step === 1" variant="primary" :loading="busy" :disabled="!form.server_url || !form.username || !form.password" @click="findCalendars">{{ t('settings.caldav.findCalendars') }}</UiButton>
      <UiButton v-else variant="primary" :loading="busy" :disabled="!chosen" @click="connect">{{ t('settings.caldav.connectBtn') }}</UiButton>
    </template>
  </UiDialog>

  <!-- Édition -->
  <UiDialog v-model:open="editOpen" :title="t('settings.caldav.editSync')" :width="460">
    <div class="form">
      <div>
        <label class="field-label">{{ t('settings.caldav.boards') }}</label>
        <UiCombobox v-model="editBoards" :items="boardItems" multiple :width="300"><button class="select">{{ boardsLabel(editBoards) }}</button></UiCombobox>
      </div>
      <label class="opt"><UiSwitch v-model="editIncludeCompleted" /> {{ t('settings.caldav.includeCompleted') }}</label>
      <div><label class="field-label">{{ t('settings.caldav.newPassword') }}</label><UiInput v-model="editPassword" type="password" autocomplete="new-password" :placeholder="t('settings.caldav.keepPassword')" /></div>
    </div>
    <template #footer>
      <UiButton variant="ghost" @click="editing = null">{{ t('common.cancel') }}</UiButton>
      <UiButton variant="primary" @click="saveEdit">{{ t('common.save') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.acc { display: flex; align-items: flex-start; gap: 12px; padding: 14px 16px; }
.acc + .acc { border-top: 1px solid var(--border); }
.acc.paused { opacity: 0.6; }
.acc-dot { width: 10px; height: 10px; border-radius: 50%; flex: none; margin-top: 5px; }
.acc-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
.acc-name { font-weight: 600; display: flex; align-items: center; gap: 8px; }
.acc-meta { font-size: var(--fs-sm); color: var(--text-3); }
.acc-error { display: flex; align-items: center; gap: 5px; font-size: var(--fs-sm); color: var(--red); margin-top: 2px; flex-wrap: wrap; }
.acc-ok { display: flex; align-items: center; gap: 5px; font-size: var(--fs-sm); color: var(--green); margin-top: 2px; }
.badge { font-size: 10.5px; font-weight: 600; padding: 1px 6px; border-radius: var(--r-full); background: var(--bg-active); color: var(--text-2); }
.empty { display: flex; align-items: center; gap: 14px; padding: 20px 16px; color: var(--text-3); }
.empty-title { color: var(--text); font-weight: 500; }
.how { margin: 0; padding: 14px 16px 14px 34px; display: flex; flex-direction: column; gap: 6px; color: var(--text-2); line-height: 1.5; }
.form { display: flex; flex-direction: column; gap: 14px; }
.preset-hint { margin: -4px 0 0; font-size: var(--fs-sm); color: var(--text-3); line-height: 1.5; }
.error { display: flex; align-items: center; gap: 6px; margin: 0; color: var(--red); font-size: var(--fs-sm); }
.cals { display: flex; flex-direction: column; gap: 6px; }
.cal { display: flex; align-items: center; gap: 10px; padding: 10px 12px; border-radius: var(--r-md); box-shadow: 0 0 0 1px var(--border-strong); cursor: pointer; }
.cal.on { box-shadow: 0 0 0 1.5px var(--accent); background: var(--accent-soft); }
.cal.disabled { opacity: 0.5; cursor: not-allowed; }
.cal input { accent-color: var(--accent); }
.cal .acc-dot { margin-top: 0; }
.cal-name { flex: 1; font-weight: 500; }
.cal-tag { display: inline-flex; align-items: center; gap: 4px; font-size: var(--fs-xs); color: var(--green); }
.select { height: 30px; padding: 0 10px; min-width: 240px; max-width: 100%; text-align: left; border-radius: var(--r-sm); border: 1px solid var(--border-strong); background: var(--bg-input); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.opt { display: flex; align-items: center; gap: 10px; cursor: pointer; }
</style>
