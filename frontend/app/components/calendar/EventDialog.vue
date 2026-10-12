<script setup lang="ts">
import { Repeat, MapPin, AlignLeft, FileText, CalendarDays, Users, Tag, ListTodo } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import type { Board } from '~/types'
import { boardKey } from '~/composables/useRealtime'
import { api, errorMessage } from '~/composables/api'
import { useWorkspace, useMyTasks } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'
import { useCalendarInvalidate, useLootyCalendars, type EventDetail } from '~/composables/useCalendar'

const open = defineModel<boolean>('open', { default: false })
/** Événement à modifier, ou valeurs initiales d'une création (début, journée entière). */
const props = defineProps<{ event?: EventDetail | null; defaults?: { start: Date; allDay?: boolean; calendarId?: string } | null }>()
const { wsId, pages, members } = useWorkspace()
// Calendrier Looty et membres mentionnés
const { data: lootyCals } = useLootyCalendars(wsId)
const calendarId = ref<string | null>(null)
const tagged = ref<string[]>([])
// Tâches mentionnées : celles du tableau du calendrier (sinon de tout l'espace).
const taskIds = ref<string[]>([])
const taskOpen = ref(false)
const openTasks = useMyTasks(ref<'mine' | 'all'>('all'))
const taskItems = computed(() => {
  const linked = props.event?.tasks ?? []
  const seen = new Set<string>()
  const out: { value: string; label: string; hint?: string }[] = []
  for (const x of linked) { seen.add(x.id); out.push({ value: x.id, label: x.title, hint: x.board_name }) }
  for (const x of openTasks.data.value ?? []) {
    if (seen.has(x.id) || (boardId.value && x.board_id !== boardId.value)) continue
    out.push({ value: x.id, label: x.title, hint: boardId.value ? x.column_name : x.board_name })
    if (out.length >= 300) break
  }
  return out
})
// Étiquettes : celles du tableau dont c'est le calendrier.
const labelIds = ref<string[]>([])
const labelOpen = ref(false)
const boardId = computed(() => lootyCals.value?.find((c) => c.id === calendarId.value)?.board_id ?? null)
const board = useQuery({
  queryKey: computed(() => boardKey(boardId.value ?? '')),
  queryFn: () => api<Board>(`/api/boards/${boardId.value}`, { silent: true }),
  enabled: computed(() => !!boardId.value && open.value),
  staleTime: 30_000,
})
const boardLabels = computed(() => (boardId.value ? board.data.value?.labels ?? [] : []))
const labelItems = computed(() => boardLabels.value.map((l) => ({ value: l.id, label: l.name, color: l.color })))
// Autre tableau : ses étiquettes seulement.
watch(boardLabels, (ls) => { if (ls.length) labelIds.value = labelIds.value.filter((id) => ls.some((l) => l.id === id)) })
const calItems = computed(() => (lootyCals.value ?? []).map((c) => ({ value: c.id, label: c.name, hint: c.board_name ?? undefined })))
const memberItems = computed(() => (members.data.value ?? []).map((m) => ({ value: m.user_id, label: m.name, hint: m.email })))
const calOpen = ref(false)
const memberOpen = ref(false)
watch(() => [props.event, props.defaults, lootyCals.value] as const, () => {
  calendarId.value = props.event?.calendar_id ?? props.defaults?.calendarId ?? lootyCals.value?.find((c) => c.is_default)?.id ?? null
  tagged.value = [...(props.event?.members ?? [])]
  labelIds.value = [...(props.event?.label_ids ?? [])]
  taskIds.value = (props.event?.tasks ?? []).map((x) => x.id)
}, { immediate: true })
// Lien vers un document : adresse complète (utile aussi dans les agendas externes),
// affichée dans Looty comme une mention de page et reprise dans le graphe.
const descEl = ref<HTMLTextAreaElement | null>(null)
const pagePick = ref(false)
const pageItems = computed(() => (pages.data.value ?? []).map((p) => ({ value: p.id, label: `${p.icon ? p.icon + ' ' : ''}${p.title || t('docs.untitled')}` })))
function linkPage(id: string | null) {
  if (!id) return
  const url = `${location.origin}/w/${wsId.value}/doc/${id}`
  const el = descEl.value
  const at = el?.selectionStart ?? form.description.length
  const before = form.description.slice(0, at)
  const sep = before && !before.endsWith('\n') && !before.endsWith(' ') ? ' ' : ''
  form.description = before + sep + url + ' ' + form.description.slice(at)
}
const { t } = useI18n()
const invalidate = useCalendarInvalidate()
const qc = useQueryClient()

type Freq = 'none' | 'daily' | 'weekly' | 'monthly' | 'yearly' | 'custom'
const pad = (n: number) => String(n).padStart(2, '0')
const dateStr = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
const timeStr = (d: Date) => `${pad(d.getHours())}:${pad(d.getMinutes())}`
const form = reactive({ title: '', allDay: false, startDate: '', startTime: '09:00', endDate: '', endTime: '10:00', freq: 'none' as Freq, interval: 1, until: '', location: '', description: '' })
const busy = ref(false)
const error = ref('')
const editing = computed(() => !!props.event)
const complex = computed(() => !!props.event?.complex_recurrence)

watch(open, (o) => {
  if (!o) return
  error.value = ''
  const e = props.event
  if (e) {
    const s = new Date(e.start)
    // Journée entière : la fin est exclusive côté serveur, incluse dans le formulaire.
    const en = e.all_day ? new Date(+new Date(e.end) - 86400000) : new Date(e.end)
    const day = (d: Date) => (e.all_day ? `${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())}` : dateStr(d))
    Object.assign(form, {
      title: e.title, allDay: e.all_day, startDate: day(s), startTime: timeStr(s), endDate: day(en), endTime: timeStr(en),
      freq: complex.value ? 'custom' : (e.repeat?.freq ?? 'none'), interval: e.repeat?.interval ?? 1, until: e.repeat?.until ?? '',
      location: e.location ?? '', description: e.description ?? '',
    })
  } else {
    const s = props.defaults?.start ?? new Date()
    const en = new Date(+s + 3600000)
    Object.assign(form, { title: '', allDay: !!props.defaults?.allDay, startDate: dateStr(s), startTime: timeStr(s), endDate: dateStr(en), endTime: timeStr(en), freq: 'none', interval: 1, until: '', location: '', description: '' })
  }
})
// Début déplacé : la fin suit (même durée).
watch(() => [form.startDate, form.startTime], ([d, tm], [od, ot]) => {
  if (!open.value || !od || !d) return
  const before = new Date(`${od}T${ot}`), after = new Date(`${d}T${tm}`)
  const end = new Date(`${form.endDate}T${form.endTime}`)
  if (isNaN(+before) || isNaN(+after) || isNaN(+end)) return
  const moved = new Date(+end + (+after - +before))
  form.endDate = dateStr(moved)
  form.endTime = timeStr(moved)
})

const freqs = computed(() => [
  { value: 'none', label: t('event.noRepeat') }, { value: 'daily', label: t('event.daily') }, { value: 'weekly', label: t('event.weekly') },
  { value: 'monthly', label: t('event.monthly') }, { value: 'yearly', label: t('event.yearly') },
  ...(complex.value ? [{ value: 'custom', label: t('event.custom') }] : []),
])
const valid = computed(() => {
  if (!form.title.trim() || !form.startDate || !form.endDate) return false
  if (form.allDay) return form.endDate >= form.startDate
  return new Date(`${form.endDate}T${form.endTime}`) >= new Date(`${form.startDate}T${form.startTime}`)
})

async function save() {
  busy.value = true
  error.value = ''
  const body = {
    title: form.title,
    all_day: form.allDay,
    start: form.allDay ? form.startDate : new Date(`${form.startDate}T${form.startTime}`).toISOString(),
    end: form.allDay ? form.endDate : new Date(`${form.endDate}T${form.endTime}`).toISOString(),
    tz: Intl.DateTimeFormat().resolvedOptions().timeZone,
    location: form.location || undefined,
    description: form.description || undefined,
    repeat: form.freq === 'none' || form.freq === 'custom' ? undefined : { freq: form.freq, interval: Number(form.interval) || 1, until: form.until || undefined },
    keep_recurrence: form.freq === 'custom',
    calendar_id: calendarId.value ?? undefined,
    members: tagged.value,
    label_ids: boardId.value ? labelIds.value : [],
  }
  try {
    let eventId = props.event?.id
    if (props.event) await api(`/api/workspaces/${wsId.value}/events/${props.event.id}`, { method: 'PUT', body, silent: true })
    else eventId = (await api<{ id: string }>(`/api/workspaces/${wsId.value}/events`, { method: 'POST', body, silent: true })).id
    // Tâches mentionnées : liées (ou déliées) à l'événement.
    const before = new Set((props.event?.tasks ?? []).map((x) => x.id))
    const start = form.allDay ? `${form.startDate}T00:00:00Z` : new Date(`${form.startDate}T${form.startTime}`).toISOString()
    for (const id of taskIds.value.filter((i) => !before.has(i))) await api(`/api/boards/tasks/${id}`, { method: 'PUT', body: { event_id: eventId, event_start: start }, silent: true })
    for (const id of [...before].filter((i) => !taskIds.value.includes(i))) await api(`/api/boards/tasks/${id}`, { method: 'PUT', body: { event_id: null }, silent: true })
    if (before.size || taskIds.value.length) { qc.invalidateQueries({ queryKey: ['board'] }); qc.invalidateQueries({ queryKey: ['ws', wsId.value, 'my-tasks'] }) }
    invalidate(wsId.value)
    toast.success(props.event ? t('event.saved') : t('event.created'))
    open.value = false
  } catch (e) {
    error.value = errorMessage(e)
  } finally { busy.value = false }
}
</script>

<template>
  <UiDialog v-model:open="open" :title="editing ? t('event.editTitle') : t('event.newTitle')" :description="editing && event?.repeat || complex ? t('event.seriesHint') : t('event.sharedHint')" :width="520">
    <form class="evf" @submit.prevent="valid && save()">
      <UiInput v-model="form.title" class="evf-title" :placeholder="t('event.titlePlaceholder')" maxlength="500" autofocus />
      <label class="evf-allday"><UiSwitch v-model="form.allDay" /><span>{{ t('calendar.allDay') }}</span></label>
      <div class="evf-row">
        <span class="evf-lab">{{ t('event.starts') }}</span>
        <UiInput v-model="form.startDate" class="evf-date" type="date" />
        <UiInput v-if="!form.allDay" v-model="form.startTime" class="evf-time" type="time" step="300" />
      </div>
      <div class="evf-row">
        <span class="evf-lab">{{ t('event.ends') }}</span>
        <UiInput v-model="form.endDate" class="evf-date" type="date" :min="form.startDate" />
        <UiInput v-if="!form.allDay" v-model="form.endTime" class="evf-time" type="time" step="300" />
      </div>
      <div class="evf-row">
        <Repeat :size="15" class="evf-ic" />
        <UiCombobox v-model="form.freq" :items="freqs" :width="240"><button type="button" class="evf-select">{{ freqs.find((f) => f.value === form.freq)?.label }}</button></UiCombobox>
      </div>
      <div v-if="form.freq !== 'none' && form.freq !== 'custom'" class="evf-row evf-sub">
        <span class="evf-small">{{ t('event.every') }}</span>
        <UiInput v-model.number="form.interval" class="evf-num" type="number" min="1" max="365" />
        <span class="evf-small">{{ t('event.until') }}</span>
        <UiInput v-model="form.until" class="evf-date" type="date" :min="form.startDate" />
      </div>
      <p v-if="form.freq === 'custom'" class="evf-hint">{{ t('event.customHint') }}</p>
      <div v-if="(lootyCals?.length ?? 0) > 1" class="evf-row">
        <CalendarDays :size="15" class="evf-ic" />
        <UiCombobox v-model:open="calOpen" v-model="calendarId" :items="calItems" :width="260">
          <button type="button" class="evf-select"><i class="evf-dot" :style="{ background: lootyCals?.find((c) => c.id === calendarId)?.color }" />{{ lootyCals?.find((c) => c.id === calendarId)?.name }}</button>
        </UiCombobox>
      </div>
      <div v-if="boardId && boardLabels.length" class="evf-row">
        <Tag :size="15" class="evf-ic" />
        <UiCombobox v-model:open="labelOpen" v-model="labelIds" multiple :items="labelItems" :width="260">
          <button type="button" class="evf-select evf-labels">
            <template v-if="labelIds.length">
              <span v-for="l in boardLabels.filter((x) => labelIds.includes(x.id))" :key="l.id" class="evf-label" :style="{ '--lc': l.color }">{{ l.name }}</span>
            </template>
            <template v-else>{{ t('event.addLabels') }}</template>
          </button>
        </UiCombobox>
      </div>
      <div v-if="taskItems.length" class="evf-row">
        <ListTodo :size="15" class="evf-ic" />
        <UiCombobox v-model:open="taskOpen" v-model="taskIds" multiple :items="taskItems" :width="320">
          <button type="button" class="evf-select">{{ taskIds.length ? t('planning.mentionedTasks', { count: taskIds.length }) : t('planning.mentionTask') }}</button>
        </UiCombobox>
      </div>
      <div class="evf-row">
        <Users :size="15" class="evf-ic" />
        <UiCombobox v-model:open="memberOpen" v-model="tagged" multiple :items="memberItems" :width="280">
          <button type="button" class="evf-select">{{ tagged.length ? tagged.map((id) => members.data.value?.find((m) => m.user_id === id)?.name).filter(Boolean).join(', ') : t('planning.tagMembers') }}</button>
        </UiCombobox>
      </div>
      <div class="evf-row"><MapPin :size="15" class="evf-ic" /><UiInput v-model="form.location" class="evf-grow" :placeholder="t('event.location')" maxlength="500" /></div>
      <div class="evf-row top"><AlignLeft :size="15" class="evf-ic" /><textarea ref="descEl" v-model="form.description" class="evf-desc" rows="3" :placeholder="t('event.description')" maxlength="10000" /></div>
      <div class="evf-row evf-sub">
        <UiCombobox v-model:open="pagePick" :items="pageItems" :width="320" :model-value="null" @update:model-value="(v) => linkPage(v as string)">
          <button type="button" class="evf-link"><FileText :size="13" /> {{ t('pageMention.linkPage') }}</button>
        </UiCombobox>
      </div>
      <p v-if="error" class="evf-err">{{ error }}</p>
      <button type="submit" hidden />
    </form>
    <template #footer>
      <UiButton variant="ghost" @click="open = false">{{ t('common.cancel') }}</UiButton>
      <UiButton variant="primary" :loading="busy" :disabled="!valid" @click="save">{{ editing ? t('common.save') : t('common.create') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.evf-labels { display: inline-flex; flex-wrap: wrap; gap: 4px; }
.evf-label { padding: 0 7px; border-radius: var(--r-full); font-size: var(--fs-xs); font-weight: 500; color: var(--lc); background: color-mix(in srgb, var(--lc) 16%, transparent); }
.evf { display: flex; flex-direction: column; gap: 10px; }
.evf-title :deep(input) { font-size: 16px; font-weight: 500; }
.evf-allday { display: flex; align-items: center; gap: 10px; cursor: pointer; font-size: var(--fs-md); }
.evf-row { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.evf-row.top { align-items: flex-start; }
.evf-dot { display: inline-block; width: 8px; height: 8px; border-radius: 50%; margin-right: 6px; }
.evf-link { display: inline-flex; align-items: center; gap: 6px; height: 26px; padding: 0 8px; border-radius: var(--r-sm); font-size: var(--fs-sm); color: var(--text-3); }
.evf-link:hover { background: var(--bg-hover); color: var(--text); }
.evf-lab { width: 52px; font-size: var(--fs-sm); color: var(--text-3); }
.evf-ic { color: var(--text-3); width: 52px; flex: none; }
.evf-small { font-size: var(--fs-sm); color: var(--text-3); }
.evf-num { width: 70px; flex: none; }
.evf-date { width: 160px; flex: none; }
.evf-time { width: 120px; flex: none; }
.evf-grow { flex: 1; width: auto; }
.evf-select { height: 30px; min-width: 170px; padding: 0 10px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); background: var(--bg-input); font-size: var(--fs-md); text-align: left; }
.evf-desc { flex: 1; min-height: 70px; resize: vertical; padding: 7px 10px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); background: var(--bg-input); color: var(--text); font: inherit; font-size: var(--fs-md); }
.evf-sub { margin: -2px 0 0 60px; }
.evf-hint { margin: -4px 0 0 60px; font-size: var(--fs-sm); color: var(--text-3); }
.evf-err { margin: 0; color: var(--red); font-size: var(--fs-sm); }
@media (max-width: 639px) { .evf-ic, .evf-lab { width: auto; } .evf-hint, .evf-sub { margin-left: 0; } }
</style>
