import { computed } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import type { Me } from '~/types'
import { api } from './api'

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
    qc.clear()
    window.location.href = '/login'
  }

  return { me: query.data, user, isAuthenticated, isLoading: query.isLoading, refresh: query.refetch, logout }
}
