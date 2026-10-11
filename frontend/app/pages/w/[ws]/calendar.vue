<script setup lang="ts">
import { CalendarDays, ChevronLeft, ChevronRight, Plus, RefreshCw, Trash2, MoreHorizontal, AlertTriangle, MapPin, ExternalLink, CheckCircle2, Circle, PanelLeft, Users, Lock, Pencil, CalendarPlus, Repeat, Share2, MessageSquare, Kanban, Filter, Link2, X } from 'lucide-vue-next'
import { useLocalStorage } from '@vueuse/core'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { api } from '~/composables/api'
import { useAccess } from '~/composables/useAccess'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { useBreakpoint } from '~/composables/useBreakpoint'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'
import { useCommentCounts } from '~/composables/useComments'
import { matchesPattern } from '~/composables/useClasses'
import {
  useCalendars, useCalendarEvents, useCalendarPrefs, useCalendarInvalidate,
  useLootyCalendars, type LootyCalendar,
  startOfDay, startOfWeek, addDays, sameDay, onDay, eventDays, type CalEvent, type CalTask, type CalendarSource, type EventDetail,
} from '~/composables/useCalendar'

definePageMeta({ layout: 'workspace' })

const { wsId, workspace, isAdmin, invalidate: invalidateWs } = useWorkspace()
const { can } = useAccess(wsId)
/** Calendrier de l'espace : rôle + mode d'édition personnel. */
const canWrite = computed(() => can.value.editCalendar)
/** Couleur du calendrier de l'espace (réglable par les administrateurs). */
const wsColor = computed(() => workspace.data.value?.calendar_color || 'var(--accent)')
async function setWsColor(c: string | null) {
  await api(`/api/workspaces/${wsId.value}/calendar-color`, { method: 'PUT', body: { color: c } })
  invalidateWs('one')
}
async function setCalColor(c: CalendarSource, color: string | null) {
  if (!color) return
  await api(`/api/calendars/${c.id}`, { method: 'PUT', body: { color } })
  invalidate(wsId.value)
}
const { t, locale } = useI18n()
const { isMobile } = useBreakpoint()
const view = useLocalStorage<'month' | 'week' | 'agenda'>('looty.calendar.view', 'week')
const cursor = ref(startOfDay(new Date()))
const sideOpen = ref(!isMobile.value)
const addOpen = ref(false)
watch(isMobile, (m) => { if (m && view.value === 'week') view.value = 'agenda' }, { immediate: true })

const { data: calendars } = useCalendars(wsId)
const { hiddenIds, toggle, showTasks } = useCalendarPrefs(wsId)
const invalidate = useCalendarInvalidate()

// Période affichée
const range = computed(() => {
  const c = cursor.value
  if (view.value === 'month') {
    const first = new Date(c.getFullYear(), c.getMonth(), 1)
    const start = startOfWeek(first)
    return { from: start, to: addDays(start, 42) }
  }
  if (view.value === 'week') {
    const start = startOfWeek(c)
    return { from: start, to: addDays(start, 7) }
  }
  return { from: startOfDay(c), to: addDays(startOfDay(c), 60) }
})
const from = computed(() => range.value.from)
const to = computed(() => range.value.to)
const { data, isFetching } = useCalendarEvents(wsId, from, to)

const calById = computed(() => new Map((calendars.value ?? []).map((c) => [c.id, c])))
const events = computed(() => (data.value?.events ?? []).filter((e) => !hiddenIds.value.has(e.calendar_id)))
const tasks = computed(() => (showTasks.value ? data.value?.tasks ?? [] : []))
// Calendriers Looty de l'espace (par défaut, thématiques, tableaux).
const { data: lootyCals } = useLootyCalendars(wsId)
const lootyById = computed(() => new Map((lootyCals.value ?? []).map((c) => [c.id, c])))
// Filtre : sélectionne d'un coup tous les événements dont le titre correspond (cours…),
// pour les relier à un document.
const filterText = ref('')
const filtering = computed(() => filterText.value.split(/[,;]/).some((v) => v.trim()))
const isSel = (e: CalEvent) => !filtering.value || matchesPattern(filterText.value, e.summary || '')
const selection = computed(() => (filtering.value ? events.value.filter(isSel) : []))
/** Calendrier commun des événements sélectionnés (abonnement), proposé pour le lien. */
const selectionCal = computed(() => {
  const ids = new Set(selection.value.filter((e) => !e.looty).map((e) => e.calendar_id))
  return ids.size === 1 ? [...ids][0]! : null
})
const classDialog = ref(false)
const color = (e: CalEvent) => lootyById.value.get(e.calendar_id)?.color ?? calById.value.get(e.calendar_id)?.color ?? 'var(--accent)'
const calName = (e: CalEvent) => lootyById.value.get(e.calendar_id)?.name ?? calById.value.get(e.calendar_id)?.name
async function patchLooty(c: LootyCalendar, body: Record<string, unknown>) {
  await api(`/api/workspaces/${wsId.value}/planning/calendars/${c.id}`, { method: 'PATCH', body })
  invalidate(wsId.value)
}
async function newLootyCalendar() {
  const name = window.prompt(t('planning.newCalendarPrompt'), '')
  if (!name?.trim()) return
  await api(`/api/workspaces/${wsId.value}/planning/calendars`, { method: 'POST', body: { name: name.trim() } })
  invalidate(wsId.value)
}
function lootyMenu(c: LootyCalendar): MenuItem[] {
  return [
    { label: t('common.rename'), icon: Pencil, onSelect: async () => { const n = window.prompt(t('planning.renamePrompt'), c.name); if (n?.trim()) await patchLooty(c, { name: n.trim() }) } },
    ...(c.board_id ? [{ label: t('planning.openBoard'), icon: Kanban, onSelect: () => navigateTo(links.board(wsId.value, c.board_id!)) }] : []),
    ...(!c.is_default ? [{ divider: true }, { label: t('common.delete'), icon: Trash2, danger: true, onSelect: async () => {
      if (!(await confirmDialog({ title: t('planning.deleteTitle', { name: c.name }), message: t('planning.deleteHint'), danger: true, confirmLabel: t('common.delete') }))) return
      await api(`/api/workspaces/${wsId.value}/planning/calendars/${c.id}`, { method: 'DELETE' })
      invalidate(wsId.value)
    } }] : []),
  ]
}
const lootyMenuOpen = ref<Record<string, boolean>>({})

// --- Calendrier de l'espace : création, modification, suppression ---
const evOpen = ref(false)
const evEdit = ref<EventDetail | null>(null)
const evDefaults = ref<{ start: Date; allDay?: boolean } | null>(null)
function newEvent(start?: Date, allDay = false) {
  if (!canWrite.value) return
  const d = start ?? (() => { const n = new Date(); n.setMinutes(n.getMinutes() < 30 ? 30 : 60, 0, 0); return n })()
  evEdit.value = null
  evDefaults.value = { start: d, allDay }
  evOpen.value = true
}
async function editEvent(e: CalEvent) {
  if (!e.event_id) return
  detailOpen.value = false
  evEdit.value = await api<EventDetail>(`/api/workspaces/${wsId.value}/events/${e.event_id}`)
  evOpen.value = true
}
async function deleteEvent(e: CalEvent, scope: 'one' | 'all') {
  if (!e.event_id) return
  const one = scope === 'one' && e.recurring
  if (!(await confirmDialog({ title: one ? t('event.deleteOneTitle') : t('event.deleteTitle', { title: e.summary || t('calendar.untitled') }), message: one ? eventWhen(e) : e.recurring ? t('event.deleteSeries') : '', danger: true, confirmLabel: t('common.delete') }))) return
  detailOpen.value = false
  await api(`/api/workspaces/${wsId.value}/events/${e.event_id}${one ? `?occurrence=${encodeURIComponent(e.starts_at)}` : ''}`, { method: 'DELETE' })
  invalidate(wsId.value)
}
/** Clic dans une colonne de la semaine : nouvel événement à l'heure visée (par demi-heure). */
function clickSlot(day: Date, ev: MouseEvent) {
  if ((ev.target as HTMLElement).closest('.wk-ev')) return
  const rect = (ev.currentTarget as HTMLElement).getBoundingClientRect()
  const minutes = Math.max(0, Math.min(23.5 * 60, Math.floor(((ev.clientY - rect.top) / HOUR) * 2) * 30))
  const d = new Date(day)
  d.setHours(0, minutes, 0, 0)
  newEvent(d)
}
function clickDay(day: Date, ev: MouseEvent, allDay: boolean) {
  if (ev.target !== ev.currentTarget) return
  const d = new Date(day)
  d.setHours(9, 0, 0, 0)
  newEvent(d, allDay)
}

function move(dir: number) {
  const c = cursor.value
  cursor.value = view.value === 'month' ? new Date(c.getFullYear(), c.getMonth() + dir, 1) : addDays(c, view.value === 'week' ? 7 * dir : 30 * dir)
}
const fmt = (o: Intl.DateTimeFormatOptions, d: Date) => new Intl.DateTimeFormat(locale.value, o).format(d)
const time = (d: Date) => fmt({ hour: '2-digit', minute: '2-digit' }, d)
const title = computed(() => {
  const c = cursor.value
  if (view.value === 'month') return fmt({ month: 'long', year: 'numeric' }, c)
  if (view.value === 'week') {
    const s = startOfWeek(c), e = addDays(s, 6)
    return s.getMonth() === e.getMonth() ? `${s.getDate()} – ${fmt({ day: 'numeric', month: 'long', year: 'numeric' }, e)}` : `${fmt({ day: 'numeric', month: 'short' }, s)} – ${fmt({ day: 'numeric', month: 'short', year: 'numeric' }, e)}`
  }
  return fmt({ day: 'numeric', month: 'long', year: 'numeric' }, c)
})
const weekdays = computed(() => Array.from({ length: 7 }, (_, i) => fmt({ weekday: 'short' }, addDays(startOfWeek(new Date(2024, 0, 1)), i))))
const today = startOfDay(new Date())

// --- Mois ---
const monthDays = computed(() => Array.from({ length: 42 }, (_, i) => addDays(range.value.from, i)))
function dayItems(day: Date) {
  const ev = events.value.filter((e) => onDay(e, day)).sort((a, b) => Number(b.all_day) - Number(a.all_day) || +new Date(a.starts_at) - +new Date(b.starts_at))
  const tk = tasks.value.filter((tk) => sameDay(new Date(tk.due_date), day))
  return { ev, tk }
}

// --- Semaine ---
const HOUR = 44
const weekDays = computed(() => Array.from({ length: 7 }, (_, i) => addDays(startOfWeek(cursor.value), i)))
const grid = ref<HTMLElement | null>(null)
watch([view, grid], () => nextTick(() => { if (grid.value && view.value === 'week') grid.value.scrollTop = HOUR * 7.5 }), { immediate: true })
/** Événements horaires d'un jour, disposés en colonnes quand ils se chevauchent. */
function timed(day: Date) {
  const d0 = startOfDay(day), d1 = addDays(d0, 1)
  const list = events.value.filter((e) => !e.all_day && onDay(e, day)).map((e) => {
    const s = new Date(Math.max(+new Date(e.starts_at), +d0)), en = new Date(Math.min(Math.max(+new Date(e.ends_at), +s + 20 * 60000), +d1))
    return { e, s, en, col: 0, cols: 1 }
  }).sort((a, b) => +a.s - +b.s || +b.en - +a.en)
  let cluster: typeof list = []
  let clusterEnd = 0
  const flush = () => { const n = Math.max(1, ...cluster.map((x) => x.col + 1)); cluster.forEach((x) => (x.cols = n)); cluster = [] }
  for (const it of list) {
    if (+it.s >= clusterEnd) { flush(); clusterEnd = 0 }
    const used = new Set(cluster.filter((x) => +x.en > +it.s).map((x) => x.col))
    while (used.has(it.col)) it.col++
    cluster.push(it)
    clusterEnd = Math.max(clusterEnd, +it.en)
  }
  flush()
  return list.map((x) => ({ ...x, top: ((+x.s - +d0) / 3600000) * HOUR, height: Math.max(((+x.en - +x.s) / 3600000) * HOUR, 18) }))
}
const nowTop = computed(() => ((Date.now() - +today) / 3600000) * HOUR)

// --- Agenda ---
const agenda = computed(() => {
  const out: { day: Date; ev: CalEvent[]; tk: CalTask[] }[] = []
  for (let i = 0; i < 60; i++) {
    const day = addDays(range.value.from, i)
    const { ev, tk } = dayItems(day)
    if (ev.length || tk.length) out.push({ day, ev, tk })
  }
  return out
})

// --- Détail d'un événement ---
const evComments = ref(false)
const { data: eventCommentCounts } = useCommentCounts(wsId, 'event')
const detail = ref<CalEvent | null>(null)
const detailRect = ref<{ x: number; y: number; width?: number; height?: number } | null>(null)
const detailOpen = ref(false)
function openEvent(e: CalEvent, ev: MouseEvent) {
  const r = (ev.currentTarget as HTMLElement).getBoundingClientRect()
  detailRect.value = { x: r.left, y: r.top, width: r.width, height: r.height }
  detail.value = e
  detailOpen.value = true
}
const route = useRoute()
const wantEvent = ref<string | null>(null)
watch(() => route.query.event as string | undefined, async (id) => {
  if (!id || !wsId.value) return
  try {
    const d = await api<EventDetail>(`/api/workspaces/${wsId.value}/events/${id}`, { silent: true })
    cursor.value = startOfDay(new Date(d.start))
    if (view.value === 'agenda') cursor.value = startOfDay(new Date(d.start))
    wantEvent.value = id
  } catch { /* événement supprimé */ }
}, { immediate: true })
watch([data, wantEvent], () => {
  const id = wantEvent.value
  const e = id ? (data.value?.events ?? []).find((x) => x.event_id === id) : null
  if (!e) return
  wantEvent.value = null
  detailRect.value = { x: window.innerWidth / 2 - 170, y: 120 }
  detail.value = e
  detailOpen.value = true
})
function eventWhen(e: CalEvent) {
  const { start, end } = eventDays(e)
  if (e.all_day) {
    const last = addDays(end, -1)
    return sameDay(start, last) ? fmt({ weekday: 'long', day: 'numeric', month: 'long' }, start) : `${fmt({ day: 'numeric', month: 'short' }, start)} – ${fmt({ day: 'numeric', month: 'short' }, last)}`
  }
  return `${fmt({ weekday: 'long', day: 'numeric', month: 'long' }, start)} · ${time(start)} – ${time(end)}`
}
const taskLink = (tk: CalTask) => links.task(wsId.value, tk.board_id, tk.id)

// --- Gestion des calendriers ---
const menuFor = ref<string | null>(null)
function calMenu(c: CalendarSource): MenuItem[] {
  return [
    { label: t('calendar.refresh'), icon: RefreshCw, onSelect: () => sync(c) },
    { divider: true },
    { label: t('common.delete'), icon: Trash2, danger: true, onSelect: () => remove(c) },
  ]
}
async function sync(c: CalendarSource) {
  try {
    const r = await api<{ events: number }>(`/api/calendars/${c.id}/sync`, { method: 'POST' })
    toast.success(t('calendar.synced', { n: r.events }))
  } finally { invalidate(wsId.value) }
}
async function remove(c: CalendarSource) {
  if (!(await confirmDialog({ title: t('calendar.deleteTitle', { name: c.name }), message: c.scope === 'workspace' ? t('calendar.deleteShared') : '', danger: true, confirmLabel: t('common.delete') }))) return
  await api(`/api/calendars/${c.id}`, { method: 'DELETE' })
  invalidate(wsId.value)
}
const shared = computed(() => (calendars.value ?? []).filter((c) => c.scope === 'workspace'))
const personal = computed(() => (calendars.value ?? []).filter((c) => c.scope === 'personal'))
useHead({ title: () => t('calendar.title') })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: t('calendar.title'), icon: CalendarDays }]">
      <UiButton size="sm" variant="ghost" :icon="PanelLeft" :aria-label="t('calendar.calendars')" @click="sideOpen = !sideOpen" />
      <UiButton size="sm" @click="cursor = startOfDay(new Date())">{{ t('calendar.today') }}</UiButton>
      <UiButton size="sm" variant="ghost" :icon="ChevronLeft" :aria-label="t('calendar.previous')" @click="move(-1)" />
      <UiButton size="sm" variant="ghost" :icon="ChevronRight" :aria-label="t('calendar.next')" @click="move(1)" />
      <span class="cal-title">{{ title }}</span>
      <RefreshCw v-if="isFetching" :size="13" class="spin faint" />
      <span class="grow" />
      <label class="cal-filter" :class="{ on: filtering }" :title="t('classes.filterHint')">
        <Filter :size="13" />
        <input v-model="filterText" :placeholder="t('classes.filter')" :aria-label="t('classes.filter')" @keydown.escape="filterText = ''" />
      </label>
      <UiSegmented v-model="view" :options="[...(isMobile ? [] : [{ value: 'week' as const, label: t('calendar.week') }]), { value: 'month' as const, label: t('calendar.month') }, { value: 'agenda' as const, label: t('calendar.agenda') }]" />
      <UiTooltip :text="t('calendar.addTitle')"><UiButton size="sm" variant="ghost" :icon="CalendarPlus" :aria-label="t('calendar.addTitle')" @click="addOpen = true" /></UiTooltip>
      <AppEditToggle area="calendar" />
      <UiButton v-if="canWrite" size="sm" variant="primary" :icon="Plus" @click="newEvent()">{{ isMobile ? '' : t('event.new') }}</UiButton>
    </AppPageHeader>

    <div v-if="filtering" class="cal-selbar">
      <Filter :size="13" />
      <span>{{ t('classes.selected', { count: selection.length }) }}</span>
      <span class="grow" />
      <UiButton v-if="canWrite" size="sm" variant="primary" :icon="Link2" :disabled="!selection.length" @click="classDialog = true">{{ t('classes.linkDoc') }}</UiButton>
      <UiButton size="sm" variant="ghost" :icon="X" :aria-label="t('common.close')" @click="filterText = ''" />
    </div>
    <CalendarClassLinkDialog v-model:open="classDialog" :pattern="filterText" :calendar-id="selectionCal" :count="selection.length" />

    <div class="cal" :class="{ side: sideOpen, filtering }">
      <aside v-if="sideOpen" class="cal-side">
        <template v-for="grp in [{ key: 'shared', icon: Users, list: shared }, { key: 'personal', icon: Lock, list: personal }]" :key="grp.key">
          <div class="cs-head"><component :is="grp.icon" :size="12" /> {{ t(`calendar.group.${grp.key}`) }}</div>
          <template v-if="grp.key === 'shared'">
            <div v-for="c in lootyCals ?? []" :key="c.id" class="cs-item">
              <button class="cs-toggle" @click="toggle(c.id)">
                <span class="cs-box" :style="{ background: hiddenIds.has(c.id) ? 'transparent' : c.color, borderColor: c.color }" />
                <span class="truncate">{{ c.is_default ? t('calendar.workspaceCal') : c.name }}</span>
                <Kanban v-if="c.board_id" :size="11" class="faint" />
              </button>
              <CalendarColorPicker v-if="canWrite" :color="c.color" @pick="(col) => col && patchLooty(c, { color: col })" />
              <UiMenu v-if="canWrite" :open="lootyMenuOpen[c.id]" :items="lootyMenu(c)" placement="bottom-end" @update:open="(o: boolean) => (lootyMenuOpen[c.id] = o)">
                <UiButton size="xs" variant="ghost" :icon="MoreHorizontal" class="cs-more" />
              </UiMenu>
            </div>
            <button v-if="canWrite" class="cs-new" @click="newLootyCalendar"><Plus :size="12" /> {{ t('planning.newCalendar') }}</button>
          </template>
          <div v-for="c in grp.list" :key="c.id" class="cs-item">
            <button class="cs-toggle" :title="c.url_host" @click="toggle(c.id)">
              <span class="cs-box" :style="{ background: hiddenIds.has(c.id) ? 'transparent' : c.color, borderColor: c.color }" />
              <span class="truncate">{{ c.name }}</span>
            </button>
            <AlertTriangle v-if="c.last_error" :size="13" class="cs-err" :title="c.last_error" />
            <CalendarColorPicker v-if="c.can_manage" :color="c.color" @pick="(col) => setCalColor(c, col)" />
            <UiMenu v-if="c.can_manage" :open="menuFor === c.id" :items="calMenu(c)" placement="bottom-end" @update:open="(o: boolean) => (menuFor = o ? c.id : null)">
              <UiButton size="xs" variant="ghost" :icon="MoreHorizontal" class="cs-more" />
            </UiMenu>
          </div>
          <p v-if="!grp.list.length && grp.key !== 'shared'" class="cs-empty">{{ t(`calendar.empty.${grp.key}`) }}</p>
        </template>
        <div class="cs-head">{{ t('calendar.other') }}</div>
        <button class="cs-toggle" @click="showTasks = !showTasks">
          <span class="cs-box" :style="{ background: showTasks ? 'var(--text-3)' : 'transparent', borderColor: 'var(--text-3)' }" />
          {{ t('calendar.myTasks') }}
        </button>
      </aside>

      <!-- Semaine -->
      <div v-if="view === 'week'" class="wk">
        <div class="wk-head">
          <div class="wk-gutter" />
          <div v-for="d in weekDays" :key="+d" class="wk-day" :class="{ today: sameDay(d, today) }">
            <span>{{ fmt({ weekday: 'short' }, d) }}</span><b>{{ d.getDate() }}</b>
          </div>
        </div>
        <div class="wk-allday">
          <div class="wk-gutter faint">{{ t('calendar.allDay') }}</div>
          <div v-for="d in weekDays" :key="+d" class="wk-cell" :class="{ add: canWrite }" @click="clickDay(d, $event, true)">
            <button v-for="e in events.filter((e) => e.all_day && onDay(e, d))" :key="e.id" class="chip filled" :class="{ dim: !isSel(e), sel: filtering && isSel(e) }" :style="{ background: color(e) }" @click="openEvent(e, $event)">{{ e.summary }}</button>
            <NuxtLink v-for="tk in tasks.filter((tk) => sameDay(new Date(tk.due_date), d))" :key="tk.id" :to="taskLink(tk)" class="chip task" :class="{ done: tk.is_completed }">
              <CheckCircle2 v-if="tk.is_completed" :size="12" /><Circle v-else :size="12" />{{ tk.title }}
            </NuxtLink>
          </div>
        </div>
        <div ref="grid" class="wk-grid">
          <div class="wk-hours">
            <div v-for="h in 24" :key="h" class="wk-hour" :style="{ height: `${HOUR}px` }"><span v-if="h > 1">{{ String(h - 1).padStart(2, '0') }}:00</span></div>
          </div>
          <div v-for="d in weekDays" :key="+d" class="wk-col" :class="{ add: canWrite }" :style="{ height: `${HOUR * 24}px` }" @click="clickSlot(d, $event)">
            <div v-for="h in 24" :key="h" class="wk-line" :style="{ top: `${(h - 1) * HOUR}px` }" />
            <div v-if="sameDay(d, today)" class="wk-now" :style="{ top: `${nowTop}px` }" />
            <button
              v-for="x in timed(d)"
              :key="x.e.id"
              class="wk-ev"
              :class="{ dim: !isSel(x.e), sel: filtering && isSel(x.e) }"
              :style="{ top: `${x.top}px`, height: `${x.height}px`, left: `calc(${(x.col / x.cols) * 100}% + 1px)`, width: `calc(${100 / x.cols}% - 3px)`, '--c': color(x.e) }"
              @click="openEvent(x.e, $event)"
            >
              <b class="wk-title"><span class="truncate">{{ x.e.summary }}</span><template v-if="x.height <= 60"><i v-for="l in x.e.labels ?? []" :key="l.id" class="ev-ldot" :style="{ background: l.color }" :title="l.name" /></template></b>
              <span v-if="x.e.labels?.length && x.height > 60" class="ev-labels"><i v-for="l in x.e.labels" :key="l.id" class="ev-label" :style="{ '--lc': l.color }">{{ l.name }}</i></span>
              <span v-if="x.height > 30" class="truncate">{{ time(x.s) }}<template v-if="x.e.location"> · {{ x.e.location }}</template></span>
            </button>
          </div>
        </div>
      </div>

      <!-- Mois -->
      <div v-else-if="view === 'month'" class="mo">
        <div class="mo-head"><span v-for="w in weekdays" :key="w">{{ w }}</span></div>
        <div class="mo-grid">
          <div v-for="d in monthDays" :key="+d" class="mo-cell" :class="{ out: d.getMonth() !== cursor.getMonth(), today: sameDay(d, today), add: canWrite }" @click="clickDay(d, $event, false)">
            <button class="mo-num" @click="cursor = d; view = isMobile ? 'agenda' : 'week'">{{ d.getDate() }}</button>
            <template v-for="(it, i) in [...dayItems(d).ev.map((e) => ({ e })), ...dayItems(d).tk.map((tk) => ({ tk }))]" :key="i">
              <template v-if="i < 3">
                <button v-if="'e' in it" class="chip" :class="{ filled: it.e.all_day, dim: !isSel(it.e), sel: filtering && isSel(it.e) }" :style="it.e.all_day ? { background: color(it.e) } : { '--c': color(it.e) }" @click="openEvent(it.e, $event)">
                  <span v-if="!it.e.all_day" class="dot" />
                  <span v-if="!it.e.all_day" class="faint">{{ time(new Date(it.e.starts_at)) }}</span>
                  <span class="truncate">{{ it.e.summary }}</span>
                  <i v-for="l in it.e.labels ?? []" :key="l.id" class="ev-ldot" :style="{ background: l.color }" :title="l.name" />
                </button>
                <NuxtLink v-else :to="taskLink(it.tk)" class="chip task" :class="{ done: it.tk.is_completed }"><Circle :size="11" /><span class="truncate">{{ it.tk.title }}</span></NuxtLink>
              </template>
            </template>
            <button v-if="dayItems(d).ev.length + dayItems(d).tk.length > 3" class="mo-more" @click="cursor = d; view = 'agenda'">+{{ dayItems(d).ev.length + dayItems(d).tk.length - 3 }}</button>
          </div>
        </div>
      </div>

      <!-- Agenda -->
      <div v-else class="ag">
        <UiEmpty v-if="!agenda.length" :icon="CalendarDays" :title="t('calendar.nothing')" :description="(calendars?.length ?? 0) ? '' : t('calendar.nothingHint')">
          <UiButton v-if="canWrite" variant="primary" :icon="Plus" @click="newEvent()">{{ t('event.new') }}</UiButton>
        </UiEmpty>
        <section v-for="g in agenda" :key="+g.day" class="ag-day">
          <h3 :class="{ today: sameDay(g.day, today) }">{{ fmt({ weekday: 'long', day: 'numeric', month: 'long' }, g.day) }}</h3>
          <button v-for="e in g.ev" :key="e.id" class="ag-item" :class="{ dim: !isSel(e), sel: filtering && isSel(e) }" @click="openEvent(e, $event)">
            <span class="ag-bar" :style="{ background: color(e) }" />
            <span class="ag-time">{{ e.all_day ? t('calendar.allDay') : `${time(new Date(e.starts_at))} – ${time(new Date(e.ends_at))}` }}</span>
            <span class="ag-title truncate">{{ e.summary }}</span>
            <span v-if="e.labels?.length" class="ev-labels"><i v-for="l in e.labels" :key="l.id" class="ev-label" :style="{ '--lc': l.color }">{{ l.name }}</i></span>
            <span v-if="e.location" class="ag-loc truncate"><MapPin :size="12" /> {{ e.location }}</span>
          </button>
          <NuxtLink v-for="tk in g.tk" :key="tk.id" :to="taskLink(tk)" class="ag-item" :class="{ done: tk.is_completed }">
            <span class="ag-bar task" />
            <span class="ag-time">{{ t('calendar.due') }}</span>
            <span class="ag-title truncate">{{ tk.title }}</span>
            <span class="ag-loc truncate">{{ tk.board_name }}</span>
          </NuxtLink>
        </section>
      </div>
    </div>

    <UiPopover v-model:open="detailOpen" :virtual-rect="detailRect" :width="evComments ? 380 : 340" placement="right-start">
      <div v-if="detail" class="ev">
        <div class="ev-head"><span class="ev-dot" :style="{ background: color(detail) }" /><h4>{{ detail.summary || t('calendar.untitled') }}</h4></div>
        <p class="ev-when">{{ eventWhen(detail) }}</p>
        <p v-if="detail.labels?.length" class="ev-labels"><i v-for="l in detail.labels" :key="l.id" class="ev-label" :style="{ '--lc': l.color }">{{ l.name }}</i></p>
        <p v-if="detail.location" class="ev-line"><MapPin :size="13" /> {{ detail.location }}</p>
        <p class="ev-line faint"><CalendarDays :size="13" /> {{ calName(detail) }}</p>
        <p v-if="detail.recurring" class="ev-line faint"><Repeat :size="13" /> {{ t('event.recurring') }}</p>
        <CalendarEventDescription v-if="detail.description" :text="detail.description" class="ev-desc" />
        <a v-if="detail.url" :href="detail.url" target="_blank" rel="noopener noreferrer" class="ev-link"><ExternalLink :size="13" /> {{ t('calendar.openLink') }}</a>
        <div v-if="detail.looty && canWrite" class="ev-actions">
          <UiButton size="sm" :icon="Pencil" @click="editEvent(detail)">{{ detail.recurring ? t('event.editSeries') : t('common.edit') }}</UiButton>
          <template v-if="detail.recurring">
            <UiButton size="sm" variant="ghost" :icon="Trash2" @click="deleteEvent(detail, 'one')">{{ t('event.deleteOne') }}</UiButton>
            <UiButton size="sm" variant="ghost" @click="deleteEvent(detail, 'all')">{{ t('event.deleteAll') }}</UiButton>
          </template>
          <UiButton v-else size="sm" variant="ghost" :icon="Trash2" @click="deleteEvent(detail, 'all')">{{ t('common.delete') }}</UiButton>
        </div>
        <template v-if="detail.looty && detail.event_id">
          <CalendarEventPlanning :event="detail" :can-write="canWrite" />
          <GraphRelatedPanel kind="event" :id="detail.event_id" compact class="ev-related" />
          <button class="ev-cm-toggle" @click="evComments = !evComments">
            <MessageSquare :size="13" /> {{ t('itemComments.title') }}<span v-if="eventCommentCounts?.[detail.event_id]" class="ev-cm-n">{{ eventCommentCounts[detail.event_id] }}</span>
          </button>
          <CommentsPanel v-if="evComments" kind="event" :target-id="detail.event_id" :can-comment="can.seeCalendar" class="ev-comments" />
        </template>
      </div>
    </UiPopover>
    <CalendarAddDialog v-model:open="addOpen" />
    <CalendarEventDialog v-model:open="evOpen" :event="evEdit" :defaults="evDefaults" />
  </div>
</template>

<style scoped>
.cal-filter { display: inline-flex; align-items: center; gap: 6px; height: 28px; width: 170px; padding: 0 8px; border-radius: var(--r-md); box-shadow: inset 0 0 0 1px var(--border); color: var(--text-3); transition: width var(--dur-fast); }
.cal-filter:focus-within, .cal-filter.on { width: 230px; color: var(--text); box-shadow: inset 0 0 0 1.5px var(--border-focus); }
.cal-filter input { flex: 1; min-width: 0; border: none; outline: none; background: transparent; font: inherit; font-size: var(--fs-sm); color: var(--text); }
.cal-selbar { display: flex; align-items: center; gap: 8px; padding: 6px 16px; background: var(--accent-soft); color: var(--accent-text); font-size: var(--fs-sm); font-weight: 500; }
.dim { opacity: .28; }
.sel { box-shadow: 0 0 0 2px var(--accent) !important; }
@media (max-width: 639px) { .cal-filter { width: 120px; } .cal-filter:focus-within, .cal-filter.on { width: 160px; } }
.ev-labels { display: flex; flex-wrap: wrap; gap: 3px; margin: 2px 0; }
.ev-label { padding: 0 6px; border-radius: var(--r-full); font-size: 10.5px; font-style: normal; font-weight: 600; line-height: 16px; color: var(--lc); background: color-mix(in srgb, var(--lc) 18%, var(--bg-elevated, transparent)); }
.wk-title { display: flex; align-items: center; gap: 4px; min-width: 0; }
.ev-ldot { flex: none; width: 6px; height: 6px; border-radius: 50%; }
.cal-title { font-weight: 600; margin-left: 4px; white-space: nowrap; text-transform: capitalize; }
.grow { flex: 1; }
.spin { animation: cs-spin 1s linear infinite; }
@keyframes cs-spin { to { transform: rotate(360deg); } }
.cal { display: grid; grid-template-columns: minmax(0, 1fr); flex: 1; min-height: 0; }
.cal.side { grid-template-columns: 220px minmax(0, 1fr); }
.cal-side { border-right: 1px solid var(--border); padding: 12px 10px; overflow-y: auto; display: flex; flex-direction: column; gap: 2px; }
.cs-head { display: flex; align-items: center; gap: 6px; font-size: var(--fs-xs); color: var(--text-3); font-weight: 600; text-transform: uppercase; letter-spacing: .03em; margin: 12px 6px 4px; }
.cs-head:first-child { margin-top: 0; }
.cs-item { display: flex; align-items: center; gap: 4px; }
.cs-item :deep(.cs-more) { opacity: 0; }
.cs-item:hover :deep(.cs-more), .cs-item :deep(.cs-more:focus-visible) { opacity: 1; }
.cs-toggle { flex: 1; min-width: 0; display: flex; align-items: center; gap: 8px; height: 28px; padding: 0 6px; border-radius: var(--r-sm); text-align: left; color: var(--text-2); }
.cs-toggle:hover { background: var(--bg-hover); color: var(--text); }
.cs-box { width: 12px; height: 12px; border-radius: 3px; border: 2px solid; flex-shrink: 0; }
.cs-err { color: var(--orange, #e2a03f); flex-shrink: 0; }
.cs-empty { margin: 0 6px; font-size: var(--fs-sm); color: var(--text-3); }
.cs-new { display: flex; align-items: center; gap: 6px; height: 26px; padding: 0 6px; margin-top: 2px; font-size: var(--fs-sm); color: var(--text-3); border-radius: var(--r-sm); }
.cs-new:hover { background: var(--bg-hover); color: var(--text); }
.cal-side > .cs-toggle { flex: none; }
.wk-col.add, .wk-cell.add, .mo-cell.add { cursor: cell; }
.ev-actions { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 10px; padding-top: 10px; border-top: 1px solid var(--border); }

.chip { display: flex; align-items: center; gap: 4px; width: 100%; min-width: 0; padding: 1px 6px; border-radius: 4px; font-size: 12px; line-height: 18px; text-align: left; color: var(--text); }
.chip:hover { background: var(--bg-hover); }
.chip.filled { color: #fff; }
.chip.filled:hover { filter: brightness(1.08); }
.chip .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--c); flex-shrink: 0; }
.chip.task { color: var(--text-2); }
.chip.task.done { text-decoration: line-through; color: var(--text-3); }

/* Semaine */
.wk { display: flex; flex-direction: column; min-height: 0; }
.wk-head, .wk-allday { display: grid; grid-template-columns: 52px repeat(7, minmax(0, 1fr)); border-bottom: 1px solid var(--border); }
.wk-day { display: flex; align-items: baseline; justify-content: center; gap: 6px; padding: 8px 0; color: var(--text-3); font-size: var(--fs-sm); text-transform: capitalize; }
.wk-day b { font-size: 18px; color: var(--text); font-weight: 600; }
.wk-day.today b { color: #fff; background: var(--accent); border-radius: 999px; padding: 0 7px; }
.wk-gutter { font-size: 10px; display: flex; align-items: center; justify-content: flex-end; padding-right: 6px; }
.wk-cell { border-left: 1px solid var(--border); padding: 3px; display: flex; flex-direction: column; gap: 2px; min-height: 26px; }
.wk-grid { flex: 1; overflow-y: auto; display: grid; grid-template-columns: 52px repeat(7, minmax(0, 1fr)); position: relative; }
.wk-hour { position: relative; font-size: 10px; color: var(--text-3); text-align: right; padding-right: 6px; }
.wk-hour span { position: relative; top: -7px; }
.wk-col { position: relative; border-left: 1px solid var(--border); }
.wk-line { position: absolute; left: 0; right: 0; border-top: 1px solid var(--border); opacity: .6; }
.wk-now { position: absolute; left: 0; right: 0; border-top: 2px solid var(--red, #eb5757); z-index: 2; }
.wk-ev { position: absolute; display: flex; flex-direction: column; gap: 1px; padding: 2px 5px; border-radius: 5px; text-align: left; font-size: 11.5px; line-height: 1.25; overflow: hidden; background: color-mix(in srgb, var(--c) 22%, var(--bg)); border-left: 3px solid var(--c); color: var(--text); }
.wk-ev:hover { background: color-mix(in srgb, var(--c) 32%, var(--bg)); z-index: 3; }
.wk-ev b { font-weight: 600; }
.wk-ev span { color: var(--text-2); }

/* Mois */
.mo { display: flex; flex-direction: column; min-height: 0; }
.mo-head { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); border-bottom: 1px solid var(--border); }
.mo-head span { padding: 6px 8px; font-size: var(--fs-sm); color: var(--text-3); text-transform: capitalize; }
.mo-grid { flex: 1; display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); grid-template-rows: repeat(6, minmax(0, 1fr)); overflow-y: auto; }
.mo-cell { border-right: 1px solid var(--border); border-bottom: 1px solid var(--border); padding: 4px; display: flex; flex-direction: column; gap: 1px; min-height: 92px; min-width: 0; }
.mo-cell.out { background: var(--bg-sidebar); }
.mo-cell.out .mo-num { color: var(--text-3); }
.mo-num { align-self: flex-start; font-size: var(--fs-sm); font-weight: 500; padding: 0 6px; border-radius: 999px; line-height: 20px; }
.mo-num:hover { background: var(--bg-hover); }
.mo-cell.today .mo-num { background: var(--accent); color: #fff; }
.mo-more { font-size: 11px; color: var(--text-3); text-align: left; padding: 0 6px; }

/* Agenda */
.ag { overflow-y: auto; padding: 16px 24px 48px; max-width: 820px; width: 100%; }
.ag-day h3 { font-size: var(--fs-sm); color: var(--text-3); font-weight: 600; margin: 18px 0 6px; text-transform: capitalize; }
.ag-day h3.today { color: var(--accent); }
.ag-item { display: grid; grid-template-columns: 4px 120px minmax(0, 1fr) auto; align-items: center; gap: 12px; width: 100%; padding: 8px 10px; border-radius: var(--r-md); text-align: left; }
.ag-item:hover { background: var(--bg-hover); }
.ag-item.done .ag-title { text-decoration: line-through; color: var(--text-3); }
.ag-bar { width: 4px; height: 22px; border-radius: 2px; }
.ag-bar.task { background: var(--text-3); }
.ag-time { font-size: var(--fs-sm); color: var(--text-2); font-variant-numeric: tabular-nums; }
.ag-title { font-weight: 500; }
.ag-loc { display: inline-flex; align-items: center; gap: 4px; font-size: var(--fs-sm); color: var(--text-3); max-width: 220px; }

/* Détail */
.ev { padding: 12px 14px; display: flex; flex-direction: column; gap: 6px; }
.ev-head { display: flex; align-items: flex-start; gap: 8px; }
.ev-head h4 { margin: 0; font-size: 15px; line-height: 1.35; }
.ev-dot { width: 10px; height: 10px; border-radius: 3px; margin-top: 5px; flex-shrink: 0; }
.ev-when { margin: 0; color: var(--text-2); font-size: var(--fs-sm); text-transform: capitalize; }
.ev-line { margin: 0; display: flex; align-items: center; gap: 6px; font-size: var(--fs-sm); }
.ev-desc { max-height: 200px; overflow-y: auto; }
.ev-related { margin-top: 6px; padding-top: 8px; border-top: 1px solid var(--border); }
.ev-cm-toggle { display: inline-flex; align-items: center; gap: 6px; align-self: flex-start; margin-top: 4px; font-size: var(--fs-sm); color: var(--text-2); padding: 3px 6px; margin-left: -6px; border-radius: var(--r-sm); }
.ev-cm-toggle:hover { background: var(--bg-hover); }
.ev-cm-n { font-size: 11px; font-weight: 600; padding: 0 6px; border-radius: var(--r-full); background: var(--accent-soft); }
.ev-comments { max-height: 360px; }
.ev-link { display: inline-flex; align-items: center; gap: 6px; font-size: var(--fs-sm); color: var(--accent); }

@media (max-width: 767px) {
  .cal.side { grid-template-columns: minmax(0, 1fr); }
  .cal-side { position: fixed; inset: 48px auto 0 0; width: min(80vw, 280px); z-index: 30; background: var(--bg-sidebar); box-shadow: var(--shadow-lg); }
  .ag { padding: 8px 12px 40px; }
  .ag-item { grid-template-columns: 4px 84px minmax(0, 1fr); }
  .ag-loc { display: none; }
  .mo-cell { min-height: 64px; }
  .mo-cell .chip span:not(.dot):not(.truncate) { display: none; }
}
</style>
