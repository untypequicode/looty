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
  /** Calendrier (abonnement, ou calendrier Looty de l'espace si `looty`). */
  calendar_id: string
  /** Événement Looty de l'espace (modifiable selon le rôle). */
  looty?: boolean
  members?: string[]
  task_count?: number
  /** Étiquettes du tableau du calendrier. */
  labels?: { id: string; name: string; color: string }[]
  event_id?: string
  recurring?: boolean
  can_edit?: boolean
  summary: string
  description?: string | null
  location?: string | null
  url?: string | null
  starts_at: string
  ends_at: string
  all_day: boolean
}

/** Événement du calendrier de l'espace, pour le formulaire. */
export interface EventDetail {
  id: string
  title: string
  description?: string | null
  location?: string | null
  start: string
  end: string
  all_day: boolean
  tzid?: string | null
  repeat?: { freq: 'daily' | 'weekly' | 'monthly' | 'yearly'; interval: number; until?: string | null } | null
  complex_recurrence: boolean
  created_by_name?: string | null
  updated_at: string
  calendar_id?: string | null
  members?: string[]
  /** Étiquettes du tableau (calendrier d'un tableau). */
  label_ids?: string[]
  board_id?: string | null
  tasks?: LinkedTask[]
}
export interface LinkedTask { id: string; title: string; is_completed: boolean; event_start?: string | null; due_date?: string | null; board_id: string; board_name: string }

/** Calendrier Looty de l'espace (par défaut, thématique ou d'un tableau). */
export interface LootyCalendar { id: string; name: string; color: string; board_id: string | null; board_name: string | null; is_default: boolean; position: number; event_count: number }

export function useLootyCalendars(ws: Ref<string>) {
  return useQuery({
    queryKey: computed(() => ['ws', ws.value, 'looty-calendars']),
    queryFn: () => api<LootyCalendar[]>(`/api/workspaces/${ws.value}/planning/calendars`),
    enabled: computed(() => !!ws.value),
    staleTime: 60_000,
  })
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
    qc.invalidateQueries({ queryKey: ['ws', ws, 'looty-calendars'] })
  }
}

// --- Dates (semaine commençant le lundi, heure locale) ---------------------------

export function startOfDay(d: Date) { const x = new Date(d); x.setHours(0, 0, 0, 0); return x }
export function addDays(d: Date, n: number) { const x = new Date(d); x.setDate(x.getDate() + n); return x }
export function startOfWeek(d: Date) { const x = startOfDay(d); return addDays(x, -((x.getDay() + 6) % 7)) }
export function sameDay(a: Date, b: Date) { return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate() }

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
