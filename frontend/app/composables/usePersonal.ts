import { computed } from 'vue'
import { useQuery } from '@tanstack/vue-query'
import type { BoardSummary, Page } from '~/types'
import { api } from './api'
import { useWorkspaceId, wsKeys } from './useWorkspace'

/**
 * Espace perso : un espace privé propre au compte (créé à la première demande), joignable
 * depuis n'importe quel espace. Il réutilise tout ce qu'offre un espace : documents,
 * tableaux, projets, calendrier, mail, drive, canevas.
 */
export function usePersonalSpace() {
  const current = useWorkspaceId()
  const space = useQuery({
    queryKey: ['personal-space'],
    queryFn: () => api<{ id: string }>('/api/workspaces/personal', { silent: true }),
    staleTime: Infinity,
  })
  const id = computed(() => space.data.value?.id ?? '')
  /** L'espace affiché est l'espace perso. */
  const inside = computed(() => !!id.value && id.value === current.value)
  // Mêmes clés que `useWorkspace` : le cache est partagé quand on est dans l'espace perso.
  const enabled = computed(() => !!id.value)
  const boards = useQuery({
    queryKey: computed(() => wsKeys.boards(id.value)),
    queryFn: () => api<BoardSummary[]>(`/api/workspaces/${id.value}/boards`),
    enabled,
  })
  const pages = useQuery({
    queryKey: computed(() => wsKeys.pages(id.value)),
    queryFn: () => api<Page[]>(`/api/workspaces/${id.value}/pages`),
    enabled,
  })
  return { id, inside, boards, pages }
}
