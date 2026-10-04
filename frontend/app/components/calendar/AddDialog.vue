<script setup lang="ts">
import { Link, Server, Search, Check } from 'lucide-vue-next'
import { api, errorMessage } from '~/composables/api'
import { useWorkspace } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'
import { useCalendarInvalidate, type CalendarSource } from '~/composables/useCalendar'

const open = defineModel<boolean>('open', { default: false })
const { wsId, isAdmin } = useWorkspace()
const { t } = useI18n()
const invalidate = useCalendarInvalidate()

const COLORS = ['#5e6ad2', '#4cb782', '#e2a03f', '#eb5757', '#26b5ce', '#bb87fc', '#f2994a', '#95a2b3']
const form = reactive({ scope: 'personal' as 'personal' | 'workspace', kind: 'ics' as 'ics' | 'caldav', name: '', color: COLORS[0]!, url: '', server: '', username: '', password: '' })
const found = ref<{ url: string; name: string; color?: string | null }[]>([])
const busy = ref(false)
const error = ref('')

watch(open, (o) => {
  if (!o) return
  Object.assign(form, { scope: 'personal', kind: 'ics', name: '', color: COLORS[Math.floor(Math.random() * COLORS.length)], url: '', server: '', username: '', password: '' })
  found.value = []
  error.value = ''
})

async function discover() {
  busy.value = true
  error.value = ''
  try {
    found.value = await api('/api/user/caldav/discover', { method: 'POST', body: { server_url: form.server, username: form.username, password: form.password }, silent: true })
    const first = found.value[0]
    if (first) pick(first)
    else error.value = t('calendar.noneFound')
  } catch (e) {
    error.value = errorMessage(e)
  } finally { busy.value = false }
}
function pick(c: { url: string; name: string; color?: string | null }) {
  form.url = c.url
  if (!form.name || found.value.some((f) => f.name === form.name)) form.name = c.name
  if (c.color && /^#[0-9a-f]{6}/i.test(c.color)) form.color = c.color.slice(0, 7)
}

const canSave = computed(() => form.name.trim() && form.url.trim() && (form.kind === 'ics' || form.username))
async function save() {
  busy.value = true
  error.value = ''
  try {
    const cal = await api<CalendarSource>(`/api/workspaces/${wsId.value}/calendars`, {
      method: 'POST',
      silent: true,
      body: { scope: form.scope, kind: form.kind, name: form.name, color: form.color, url: form.url, username: form.kind === 'caldav' ? form.username : undefined, password: form.kind === 'caldav' ? form.password : undefined },
    })
    invalidate(wsId.value)
    if (cal.last_error) toast.error(`${cal.name} : ${t('calendar.syncFailed')}`)
    else toast.success(t('calendar.added', { n: cal.event_count }))
    open.value = false
  } catch (e) {
    error.value = errorMessage(e)
  } finally { busy.value = false }
}
</script>

<template>
  <UiDialog v-model:open="open" :title="t('calendar.addTitle')" :description="t('calendar.addHint')" :width="500">
    <div class="ca">
      <template v-if="isAdmin">
        <div class="ca-label">{{ t('calendar.visibility') }}</div>
        <UiSegmented v-model="form.scope" size="md" :options="[{ value: 'personal', label: t('calendar.personal') }, { value: 'workspace', label: t('calendar.shared') }]" />
        <p class="ca-hint">{{ form.scope === 'personal' ? t('calendar.personalHint') : t('calendar.sharedHint') }}</p>
      </template>

      <div class="ca-label">{{ t('calendar.type') }}</div>
      <div class="ca-kinds">
        <button class="ca-kind" :class="{ on: form.kind === 'ics' }" @click="form.kind = 'ics'"><Link :size="16" /><span><b>{{ t('calendar.ics') }}</b><small>{{ t('calendar.icsHint') }}</small></span></button>
        <button class="ca-kind" :class="{ on: form.kind === 'caldav' }" @click="form.kind = 'caldav'"><Server :size="16" /><span><b>CalDAV</b><small>{{ t('calendar.caldavHint') }}</small></span></button>
      </div>

      <template v-if="form.kind === 'ics'">
        <div class="ca-label">{{ t('calendar.icsUrl') }}</div>
        <UiInput v-model="form.url" placeholder="https://… .ics  /  webcal://…" autocomplete="off" />
      </template>
      <template v-else>
        <div class="ca-label">{{ t('calendar.server') }}</div>
        <UiInput v-model="form.server" placeholder="https://cloud.example.com/remote.php/dav" autocomplete="off" />
        <div class="ca-row">
          <UiInput v-model="form.username" :placeholder="t('calendar.username')" autocomplete="off" />
          <UiInput v-model="form.password" type="password" :placeholder="t('calendar.password')" autocomplete="new-password" />
        </div>
        <UiButton size="sm" :icon="Search" :loading="busy" :disabled="!form.server || !form.username" @click="discover">{{ t('calendar.find') }}</UiButton>
        <div v-if="found.length" class="ca-found">
          <button v-for="c in found" :key="c.url" class="ca-cal" :class="{ on: form.url === c.url }" @click="pick(c)">
            <span class="dot" :style="{ background: c.color || 'var(--accent)' }" />{{ c.name }}<Check v-if="form.url === c.url" :size="14" />
          </button>
        </div>
      </template>

      <div class="ca-label">{{ t('calendar.name') }}</div>
      <div class="ca-row">
        <UiInput v-model="form.name" :placeholder="t('calendar.namePlaceholder')" maxlength="120" />
        <div class="ca-colors">
          <button v-for="c in COLORS" :key="c" class="sw" :class="{ on: form.color === c }" :style="{ background: c }" :aria-label="c" @click="form.color = c" />
        </div>
      </div>
      <p v-if="error" class="ca-err">{{ error }}</p>
    </div>
    <template #footer>
      <UiButton variant="ghost" @click="open = false">{{ t('common.cancel') }}</UiButton>
      <UiButton variant="primary" :loading="busy" :disabled="!canSave" @click="save">{{ t('calendar.add') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.ca { display: flex; flex-direction: column; gap: 8px; }
.ca-label { font-size: var(--fs-sm); color: var(--text-3); font-weight: 500; margin-top: 6px; }
.ca-hint { margin: 0; color: var(--text-3); font-size: var(--fs-sm); }
.ca-kinds { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
.ca-kind { display: flex; gap: 10px; align-items: flex-start; text-align: left; padding: 10px 12px; border-radius: var(--r-md); box-shadow: inset 0 0 0 1px var(--border-strong); color: var(--text-2); }
.ca-kind.on { box-shadow: inset 0 0 0 1.5px var(--accent); background: var(--accent-soft); color: var(--text); }
.ca-kind span { display: flex; flex-direction: column; gap: 2px; }
.ca-kind small { color: var(--text-3); font-size: var(--fs-sm); line-height: 1.35; }
.ca-row { display: flex; gap: 8px; align-items: center; }
.ca-row > :first-child { flex: 1; }
.ca-colors { display: flex; gap: 4px; flex-wrap: wrap; max-width: 150px; }
.sw { width: 16px; height: 16px; border-radius: 50%; }
.sw.on { box-shadow: 0 0 0 2px var(--bg-elevated), 0 0 0 3.5px var(--text); }
.ca-found { display: flex; flex-direction: column; gap: 2px; max-height: 140px; overflow-y: auto; }
.ca-cal { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: var(--r-sm); text-align: left; }
.ca-cal:hover, .ca-cal.on { background: var(--bg-hover); }
.ca-cal svg { margin-left: auto; color: var(--accent); }
.dot { width: 9px; height: 9px; border-radius: 50%; flex-shrink: 0; }
.ca-err { margin: 4px 0 0; color: var(--red); font-size: var(--fs-sm); }
@media (max-width: 639px) { .ca-kinds { grid-template-columns: 1fr; } .ca-row { flex-direction: column; align-items: stretch; } }
</style>
