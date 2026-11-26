import { computed, type Ref } from 'vue'
import { useQuery } from '@tanstack/vue-query'
import { api } from './api'
import type { WorkspaceMember } from '~/types'
import { moveKind, sortMoveChanges, type AccessSummary, type MoveChange, type SharedItem } from '~/utils/shareAccess'
import { confirmDialog } from './useConfirm'
import { translate } from './useI18n'

/** Pages partagées avec moi dans des espaces dont je ne suis pas membre (invité). */
export type SharedPage = SharedItem & { workspace_id: string; parent_id: string | null; role: 'viewer' | 'editor' }

export function useSharedPages() {
  // Actualisé quand la fenêtre reprend le focus et à intervalle : un partage ajouté ou retiré apparaît sans recharger.
  return useQuery({ queryKey: ['shared-pages'], queryFn: () => api<SharedPage[]>('/api/shared', { silent: true }), staleTime: 15_000, refetchOnWindowFocus: true, refetchInterval: 60_000 })
}

export const accessSummaryKey = (ws: string, page: string) => ['page-access-summary', ws, page]

/** Qui a accès à la page, d'où vient chaque droit (réservé aux personnes qui gèrent la page). */
export function useAccessSummary(ws: Ref<string>, page: Ref<string>, enabled: Ref<boolean>) {
  return useQuery({
    queryKey: computed(() => accessSummaryKey(ws.value, page.value)),
    queryFn: () => api<AccessSummary>(`/api/workspaces/${ws.value}/pages/${page.value}/access-summary`, { silent: true }),
    enabled: computed(() => enabled.value && !!ws.value && !!page.value),
    staleTime: 0,
  })
}

export interface PageRequest { id: string; user_id: string; name: string; email: string; avatar_url?: string | null; message: string; created_at: string }
export function useAccessRequests(ws: Ref<string>, page: Ref<string>, enabled: Ref<boolean>) {
  return useQuery({
    queryKey: computed(() => ['page-access-requests', ws.value, page.value]),
    queryFn: () => api<PageRequest[]>(`/api/workspaces/${ws.value}/pages/${page.value}/access-requests`, { silent: true }),
    enabled: computed(() => enabled.value && !!ws.value && !!page.value),
    staleTime: 0,
  })
}

/** Personnes extérieures à l'espace qui peuvent lire la page : elles se mentionnent comme les membres. */
export function usePageOutsiders(pageId: () => string | undefined, wsId: () => string) {
  const q = useQuery({
    queryKey: computed(() => ['page-mentionable', pageId()]),
    queryFn: () => api<{ user_id: string; name: string; email: string; avatar_url?: string | null; is_member: boolean }[]>(`/api/workspaces/${wsId()}/pages/${pageId()}/mentionable`, { silent: true }),
    enabled: computed(() => !!pageId() && !!wsId()),
    staleTime: 30_000,
  })
  return computed<WorkspaceMember[]>(() => (q.data.value ?? []).filter((p) => !p.is_member).map((p) => ({ user_id: p.user_id, name: p.name, email: p.email, avatar_url: p.avatar_url, role: 'guest' as WorkspaceMember['role'] })))
}

/**
 * Avant de ranger une page sous un autre parent : qui gagne ou perd l'accès (aperçu du serveur), et confirmation.
 * Rien à confirmer si personne n'est touché. `true` : on peut déplacer.
 */
export async function confirmMoveAccess(ws: string, page: { id: string; title: string }, parentId: string | null): Promise<boolean> {
  let changes: MoveChange[] = []
  try {
    const q = parentId ? `?parent_id=${parentId}` : ''
    changes = (await api<{ changes: MoveChange[] }>(`/api/workspaces/${ws}/pages/${page.id}/move-preview${q}`, { silent: true })).changes
  } catch {
    return true // l'aperçu est une aide : le serveur reste juge du déplacement
  }
  if (!changes.length) return true
  const lines = sortMoveChanges(changes).slice(0, 8).map((c) => {
    const kind = moveKind(c)
    const lv = (l: string) => translate(`sharing.level.${l === 'none' ? 'deny' : l}`)
    return `• ${kind === 'lost' ? translate('move.lost', { name: c.name }) : kind === 'gained' ? translate('move.gained', { name: c.name, level: lv(c.after) }) : translate('move.changed', { name: c.name, before: lv(c.before), after: lv(c.after) })}`
  })
  if (changes.length > 8) lines.push(`• … (+${changes.length - 8})`)
  return confirmDialog({ title: translate('move.accessTitle'), message: `${translate('move.accessBody', { title: page.title || translate('docs.untitled') })}\n${lines.join('\n')}`, confirmLabel: translate('move.confirm') })
}

/** Déplacer vers un autre espace retire les partages individuels de la page et de ses sous-pages : on le dit avant. */
export function confirmMoveSpace(page: { title: string }, shared: boolean): Promise<boolean> | boolean {
  if (!shared) return true
  return confirmDialog({ title: translate('move.sharesDropTitle'), message: translate('move.sharesDropBody', { title: page.title || translate('docs.untitled') }), confirmLabel: translate('move.confirm') })
}
