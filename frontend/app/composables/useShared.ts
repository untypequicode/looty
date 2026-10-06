import { useQuery } from '@tanstack/vue-query'
import { api } from './api'

/** Pages partagées avec moi dans des espaces dont je ne suis pas membre (invité). */
export interface SharedPage { id: string; workspace_id: string; workspace_name: string; parent_id: string | null; title: string; icon: string | null; updated_at: string; role: 'viewer' | 'editor'; root: boolean }

export function useSharedPages() {
  return useQuery({ queryKey: ['shared-pages'], queryFn: () => api<SharedPage[]>('/api/shared', { silent: true }), staleTime: 60_000 })
}
