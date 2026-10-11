import { computed, ref, type Ref } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { useIntervalFn } from '@vueuse/core'
import { api } from './api'

/** Règle : les cours dont le titre correspond au filtre sont reliés à un document. */
export interface ClassLink { id: string; calendar_id: string | null; calendar_name?: string | null; pattern: string; page_id: string; page_title: string; page_icon?: string | null; include_subpages: boolean }
export interface ClassDoc { id: string; title: string; icon?: string | null; link_id: string; children: { id: string; title: string; icon?: string | null; depth: number }[] }
export interface ClassEvent { id: string; calendar_id: string; calendar_name: string; color: string; summary: string; location?: string | null; starts_at: string; ends_at: string; all_day: boolean; documents: ClassDoc[] }

export const classLinksKey = (ws: string) => ['ws', ws, 'class-links'] as const

/** Minuscules sans accents (même règle que le serveur). */
export function foldText(s: string) {
  return s.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/œ/g, 'oe').replace(/æ/g, 'ae')
}
/** Variantes séparées par des virgules ; dans une variante, tous les mots doivent figurer. */
export function matchesPattern(pattern: string, title: string) {
  const t = foldText(title)
  return pattern.split(/[,;]/).map((v) => v.trim()).filter(Boolean).some((v) => {
    const words = v.split(/\s+/).map(foldText)
    return words.length > 0 && words.every((w) => t.includes(w))
  })
}

export function useClassLinks(ws: Ref<string>) {
  const qc = useQueryClient()
  const query = useQuery({
    queryKey: computed(() => classLinksKey(ws.value)),
    queryFn: () => api<ClassLink[]>(`/api/workspaces/${ws.value}/class-links`, { silent: true }),
    enabled: computed(() => !!ws.value),
    staleTime: 60_000,
  })
  const refresh = () => {
    qc.invalidateQueries({ queryKey: classLinksKey(ws.value) })
    qc.invalidateQueries({ queryKey: ['ws', ws.value, 'classes'] })
  }
  async function create(body: { calendar_id?: string | null; pattern: string; page_id: string; include_subpages: boolean }) {
    await api(`/api/workspaces/${ws.value}/class-links`, { method: 'POST', body })
    refresh()
  }
  async function update(id: string, body: Partial<{ pattern: string; page_id: string; include_subpages: boolean }>) {
    await api(`/api/workspaces/${ws.value}/class-links/${id}`, { method: 'PATCH', body })
    refresh()
  }
  async function remove(id: string) {
    await api(`/api/workspaces/${ws.value}/class-links/${id}`, { method: 'DELETE' })
    refresh()
  }
  return { links: query.data, create, update, remove, refresh }
}

/** Cours d'aujourd'hui reliés à des documents : celui en cours et les suivants. */
export function useTodayClasses(ws: Ref<string>) {
  const now = ref(new Date())
  useIntervalFn(() => (now.value = new Date()), 30_000)
  const day = computed(() => { const d = new Date(now.value); d.setHours(0, 0, 0, 0); return d })
  const query = useQuery({
    queryKey: computed(() => ['ws', ws.value, 'classes', +day.value]),
    queryFn: () => {
      const to = new Date(+day.value + 86400000)
      return api<ClassEvent[]>(`/api/workspaces/${ws.value}/classes?from=${encodeURIComponent(day.value.toISOString())}&to=${encodeURIComponent(to.toISOString())}`, { silent: true })
    },
    enabled: computed(() => !!ws.value),
    staleTime: 60_000,
  })
  const timed = computed(() => (query.data.value ?? []).filter((e) => !e.all_day))
  const current = computed(() => timed.value.filter((e) => +new Date(e.starts_at) <= +now.value && +now.value < +new Date(e.ends_at)))
  const next = computed(() => timed.value.filter((e) => +new Date(e.starts_at) > +now.value))
  return { classes: query.data, current, next, now, isLoading: query.isLoading }
}
