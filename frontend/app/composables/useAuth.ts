import { computed } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import type { Me } from '~/types'
import { api } from './api'
import { draftKeyFor, LEGACY_DRAFT_KEY } from '~/utils/assistantDraft'

export const ME_KEY = ['me']

export function fetchMe(): Promise<Me> {
  return api<Me>('/api/auth/me', { silent: true })
}

export function useAuth() {
  const qc = useQueryClient()
  const query = useQuery({ queryKey: ME_KEY, queryFn: fetchMe, staleTime: 5 * 60_000 })
  const user = computed(() => query.data.value?.user ?? null)
  const isAuthenticated = computed(() => !!query.data.value?.authenticated)

  async function logout() {
    await api('/api/auth/logout', { method: 'POST', silent: true }).catch(() => {})
    // Le brouillon de l'Assistant de cette personne ne reste pas dans le navigateur.
    try { if (user.value) localStorage.removeItem(draftKeyFor(user.value.id)); localStorage.removeItem(LEGACY_DRAFT_KEY) } catch { /* stockage indisponible */ }
    qc.clear()
    window.location.href = '/login'
  }

  return { me: query.data, user, isAuthenticated, isLoading: query.isLoading, refresh: query.refetch, logout }
}
