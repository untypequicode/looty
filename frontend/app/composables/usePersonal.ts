import { computed, ref, watch } from 'vue'
import { useQuery } from '@tanstack/vue-query'
import type { BoardSummary, Page } from '~/types'
import { api } from './api'
import { links, useWorkspaceId, wsKeys } from './useWorkspace'

const ID_KEY = 'looty.personalSpace'
const SHELL_KEY = 'looty.shellWs'
const FULL_KEY = 'looty.personalFull'

/**
 * Espace perso : un espace privé propre au compte (créé à la première demande), joignable
 * depuis n'importe quel espace. Il réutilise tout ce qu'offre un espace : documents,
 * tableaux, projets, calendrier, mail, drive, canevas.
 */
export function usePersonalSpace() {
  const current = useWorkspaceId()
  const space = useQuery({
    queryKey: ['personal-space'],
    queryFn: async () => {
      const r = await api<{ id: string }>('/api/workspaces/personal', { silent: true })
      localStorage.setItem(ID_KEY, r.id)
      return r
    },
    // Connu dès le chargement : la coque ne clignote pas sur un lien perso rechargé.
    placeholderData: () => (localStorage.getItem(ID_KEY) ? { id: localStorage.getItem(ID_KEY)! } : undefined),
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

/** Dernier espace partagé visité : il reste autour du contenu perso. */
const lastShared = ref(localStorage.getItem(SHELL_KEY) ?? '')
/** L'espace perso ouvert pour lui-même (menu des espaces) : sa propre barre latérale. */
const full = ref(sessionStorage.getItem(FULL_KEY) === '1')
function setFull(v: boolean) {
  full.value = v
  if (v) sessionStorage.setItem(FULL_KEY, '1')
  else sessionStorage.removeItem(FULL_KEY)
}

/**
 * Coque de l'application (barre latérale, menu, onglets, présence) : un élément perso ouvert
 * depuis un autre espace s'affiche sans quitter cet espace ; seul le menu des espaces ouvre
 * l'espace perso pour lui-même.
 */
export function usePersonalShell() {
  const route = useRoute()
  const { id: personalId } = usePersonalSpace()
  const routeWs = computed(() => (route.params.ws as string) || '')
  watch([routeWs, personalId], ([ws, p]) => {
    if (!ws || !p || ws === p) return
    lastShared.value = ws
    localStorage.setItem(SHELL_KEY, ws)
    setFull(false)
  }, { immediate: true })
  /** Contenu perso affiché dans la coque d'un autre espace. */
  const borrowing = computed(() => !!personalId.value && routeWs.value === personalId.value && !full.value && !!lastShared.value && lastShared.value !== personalId.value)
  const id = computed(() => (borrowing.value ? lastShared.value : routeWs.value))
  /** L'espace gardé autour n'est plus accessible : l'espace perso s'ouvre pour lui-même. */
  function dropShell() {
    lastShared.value = ''
    localStorage.removeItem(SHELL_KEY)
  }
  return { id, routeWs, borrowing, dropShell }
}

/** Élément perso affiché dans la coque d'un autre espace (repère dans l'en-tête). */
export function usePersonalBorrowed() {
  const { id, inside } = usePersonalSpace()
  return computed(() => inside.value && !full.value && !!lastShared.value && lastShared.value !== id.value)
}

/** Ouvre l'espace perso pour lui-même, avec sa propre barre latérale. */
export function openPersonalSpace(id: string) {
  setFull(true)
  return navigateTo(links.home(id))
}
