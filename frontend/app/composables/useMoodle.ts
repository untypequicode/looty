import { computed, type Ref } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from './api'

export interface MoodleConnection {
  base_url: string
  moodle_user_id: number | null
  full_name: string | null
  site_name: string | null
  last_error: string | null
  last_sync_at: string | null
  calendar_id: string | null
  notify: boolean
}
export interface MoodleOverview { url: string | null; default_url: string; can_configure: boolean; connection: MoodleConnection | null }
export interface MoodleSite { base_url: string; site_name: string | null; logo: string | null; sso: boolean; launch_url: string; tokens_url: string }
export interface MoodleCourse { id: number; fullname: string; shortname: string; category: string | null; progress: number | null; favourite: boolean; url: string }
export interface MoodleDeadline { id: number; name: string; event_name: string; course: string; course_id: number; module: string; due: string; overdue: boolean; url: string; action: string | null }
export interface MoodleConversation { id: number; name: string; group: boolean; unread: number; last_text: string | null; last_from_me: boolean | null; last_time: string | null }
export interface MoodleMessage { id: number; mine: boolean; from: string; html: string; time: string }
export interface MoodleNotification { id: number; subject: string; text: string; url: string | null; read: boolean; time: string }

export const moodleKeys = {
  overview: (ws: string) => ['moodle', ws, 'overview'] as const,
  data: (ws: string, kind: string) => ['moodle', ws, kind] as const,
}

export function useMoodle(ws: Ref<string>) {
  return useQuery({
    queryKey: computed(() => moodleKeys.overview(ws.value)),
    queryFn: () => api<MoodleOverview>(`/api/workspaces/${ws.value}/moodle`, { silent: true }),
    enabled: computed(() => !!ws.value),
    staleTime: 60_000,
  })
}

/** Données lues sur Moodle à la demande (cours, échéances, messages, notifications). */
export function useMoodleData<T>(ws: Ref<string>, kind: string, enabled: Ref<boolean>, refetchInterval?: number) {
  return useQuery({
    queryKey: computed(() => moodleKeys.data(ws.value, kind)),
    queryFn: () => api<T>(`/api/workspaces/${ws.value}/moodle/${kind}`, { silent: true }),
    enabled,
    staleTime: 60_000,
    refetchInterval,
    retry: false,
  })
}

export function useMoodleInvalidate(ws: Ref<string>) {
  const qc = useQueryClient()
  return () => qc.invalidateQueries({ queryKey: ['moodle', ws.value] })
}
