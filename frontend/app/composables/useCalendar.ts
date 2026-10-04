import { computed, type Ref } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { useLocalStorage } from '@vueuse/core'
import { api } from './api'

export interface CalendarSource {
  id: string
  name: string
  color: string
  kind: 'ics' | 'caldav'
  scope: 'workspace' | 'personal'
  url_host: string
  username?: string | null
  enabled: boolean
  last_sync_at?: string | null
  last_error?: string | null
  event_count: number
  can_manage: boolean
}

export interface CalEvent {
  id: string
  calendar_id: string
  summary: string
  description?: string | null
  location?: string | null
  url?: string | null
  starts_at: string
  ends_at: string
  all_day: boolean
}

export interface CalTask {
  id: string
  title: string
  due_date: string
  is_completed: boolean
  board_id: string
  board_name: string
}

export const calKeys = {
  list: (ws: string) => ['ws', ws, 'calendars'] as const,
  events: (ws: string, from: string, to: string) => ['ws', ws, 'calendar-events', from, to] as const,
}

export function useCalendars(ws: Ref<string>) {
  return useQuery({ queryKey: computed(() => calKeys.list(ws.value)), queryFn: () => api<CalendarSource[]>(`/api/workspaces/${ws.value}/calendars`), enabled: computed(() => !!ws.value) })
}

export function useCalendarEvents(ws: Ref<string>, from: Ref<Date>, to: Ref<Date>) {
  return useQuery({
    queryKey: computed(() => calKeys.events(ws.value, from.value.toISOString(), to.value.toISOString())),
    queryFn: () => api<{ events: CalEvent[]; tasks: CalTask[] }>(`/api/workspaces/${ws.value}/calendars/events?from=${encodeURIComponent(from.value.toISOString())}&to=${encodeURIComponent(to.value.toISOString())}`),
    enabled: computed(() => !!ws.value),
    placeholderData: (prev) => prev,
    staleTime: 60_000,
  })
}

/** Calendriers masqués et affichage des tâches (préférences locales, par espace). */
export function useCalendarPrefs(ws: Ref<string>) {
  const hidden = useLocalStorage<Record<string, string[]>>('looty.calendar.hidden', {})
  const showTasks = useLocalStorage('looty.calendar.tasks', true)
  const hiddenIds = computed(() => new Set(hidden.value[ws.value] ?? []))
  function toggle(id: string) {
    const cur = new Set(hidden.value[ws.value] ?? [])
    cur.has(id) ? cur.delete(id) : cur.add(id)
    hidden.value = { ...hidden.value, [ws.value]: [...cur] }
  }
  return { hiddenIds, toggle, showTasks }
}

export function useCalendarInvalidate() {
  const qc = useQueryClient()
  return (ws: string) => {
    qc.invalidateQueries({ queryKey: calKeys.list(ws) })
    qc.invalidateQueries({ queryKey: ['ws', ws, 'calendar-events'] })
  }
}

// --- Dates (semaine commençant le lundi, heure locale) ---------------------------

export function startOfDay(d: Date) { const x = new Date(d); x.setHours(0, 0, 0, 0); return x }
export function addDays(d: Date, n: number) { const x = new Date(d); x.setDate(x.getDate() + n); return x }
export function startOfWeek(d: Date) { const x = startOfDay(d); return addDays(x, -((x.getDay() + 6) % 7)) }
export function sameDay(a: Date, b: Date) { return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate() }
export function dayKey(d: Date) { return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}` }

/** Événement « journée entière » : dates stockées à minuit UTC, affichées sans décalage. */
export function eventDays(e: CalEvent): { start: Date; end: Date } {
  if (!e.all_day) return { start: new Date(e.starts_at), end: new Date(e.ends_at) }
  const s = new Date(e.starts_at), en = new Date(e.ends_at)
  return {
    start: new Date(s.getUTCFullYear(), s.getUTCMonth(), s.getUTCDate()),
    end: new Date(en.getUTCFullYear(), en.getUTCMonth(), en.getUTCDate()),
  }
}

/** L'événement couvre-t-il ce jour (local) ? */
export function onDay(e: CalEvent, day: Date) {
  const { start, end } = eventDays(e)
  const d0 = startOfDay(day), d1 = addDays(d0, 1)
  return e.all_day ? start < d1 && end > d0 : start < d1 && (end > d0 || +end === +start && start >= d0)
}
