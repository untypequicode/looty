import { computed, type Ref } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from './api'

/** Annonce de l'espace (GLOSSARY). `body` : HTML nettoyé côté serveur. */
export interface Announcement {
  id: string
  workspace_id: string
  author_id: string | null
  author_name: string | null
  title: string
  body: string
  pinned: boolean
  expires_at: string | null
  created_at: string
  updated_at: string
  read: boolean
  expired: boolean
  poll: Poll | null
}

/** Sondage d'une annonce, tel que le membre le voit (totaux absents tant qu'ils sont cachés). */
export interface Poll {
  question: string
  multiple: boolean
  anonymous: boolean
  results_after_vote: boolean
  closes_at: string | null
  closed: boolean
  results_visible: boolean
  voter_count: number
  my_votes: string[]
  options: { id: string; label: string; votes: number | null; voters: string[] | null }[]
}
export interface PollInput { question: string; options: string[]; multiple: boolean; anonymous: boolean; results_after_vote: boolean; closes_at: string | null }
export interface AnnouncementList { announcements: Announcement[]; unread: number; can_manage: boolean }
export interface AnnouncementInput { title?: string; body?: string; pinned?: boolean; expires_at?: string | null; poll?: PollInput | null }

export const announcementsKey = (ws: string) => ['ws', ws, 'announcements'] as const

/** Annonces de l'espace, nombre de non lues et actions (publier, modifier, lire…). */
export function useAnnouncements(ws: Ref<string>, enabled?: Ref<boolean>) {
  const qc = useQueryClient()
  const base = () => `/api/workspaces/${ws.value}/announcements`
  const query = useQuery({
    queryKey: computed(() => announcementsKey(ws.value)),
    queryFn: () => api<AnnouncementList>(base(), { silent: true }),
    enabled: computed(() => !!ws.value && (enabled?.value ?? true)),
    staleTime: 60_000,
  })
  const refresh = () => qc.invalidateQueries({ queryKey: announcementsKey(ws.value) })
  const patchLocal = (fn: (l: AnnouncementList) => void) => qc.setQueryData<AnnouncementList>(announcementsKey(ws.value), (old) => {
    if (!old) return old
    const copy: AnnouncementList = { ...old, announcements: old.announcements.map((a) => ({ ...a })) }
    fn(copy)
    copy.unread = copy.announcements.filter((a) => !a.read && !a.expired).length
    return copy
  })

  async function create(body: AnnouncementInput) {
    const a = await api<Announcement>(base(), { method: 'POST', body })
    await refresh()
    return a
  }
  async function update(id: string, body: AnnouncementInput) {
    const a = await api<Announcement>(`${base()}/${id}`, { method: 'PUT', body })
    await refresh()
    return a
  }
  async function remove(id: string) {
    await api(`${base()}/${id}`, { method: 'DELETE' })
    await refresh()
  }
  async function markRead(id: string) {
    patchLocal((l) => { const a = l.announcements.find((x) => x.id === id); if (a) a.read = true })
    await api(`${base()}/${id}/read`, { method: 'POST', silent: true }).catch(() => {})
    qc.invalidateQueries({ queryKey: ['ws', ws.value, 'notifications'] })
  }
  async function markAllRead() {
    patchLocal((l) => l.announcements.forEach((a) => { a.read = true }))
    await api(`${base()}/read-all`, { method: 'POST', silent: true }).catch(() => {})
    qc.invalidateQueries({ queryKey: ['ws', ws.value, 'notifications'] })
  }

  async function vote(id: string, options: string[]) {
    await api(`${base()}/${id}/vote`, { method: 'POST', body: { options } })
    await refresh()
  }
  async function closePoll(id: string) {
    await api(`${base()}/${id}/poll/close`, { method: 'POST' })
    await refresh()
  }

  return {
    query,
    list: computed(() => query.data.value?.announcements ?? []),
    unread: computed(() => query.data.value?.unread ?? 0),
    canManage: computed(() => !!query.data.value?.can_manage),
    create, update, remove, markRead, markAllRead, refresh, vote, closePoll,
  }
}
