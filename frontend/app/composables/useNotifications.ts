import { computed } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import type { Notification } from '~/types'
import { api } from './api'
import { useWorkspaceId, wsKeys } from './useWorkspace'

export function useNotifications() {
  const wsId = useWorkspaceId()
  const qc = useQueryClient()
  const query = useQuery({
    queryKey: computed(() => wsKeys.notifications(wsId.value)),
    queryFn: () => api<Notification[]>(`/api/notifications?workspace_id=${wsId.value}`, { silent: true }),
    enabled: computed(() => !!wsId.value),
  })
  const unread = computed(() => (query.data.value ?? []).filter((n) => !n.read_at).length)

  function patch(fn: (list: Notification[]) => Notification[]) {
    qc.setQueryData<Notification[]>(wsKeys.notifications(wsId.value), (old) => (old ? fn(old) : old))
  }

  async function markRead(n: Notification, read = true) {
    patch((l) => l.map((x) => (x.id === n.id ? { ...x, read_at: read ? new Date().toISOString() : null } : x)))
    await api(`/api/notifications/${n.id}/read`, { method: 'PUT', body: { read }, silent: true }).catch(() => {})
  }

  async function markAllRead() {
    const now = new Date().toISOString()
    patch((l) => l.map((x) => ({ ...x, read_at: x.read_at ?? now })))
    await api(`/api/notifications/read-all?workspace_id=${wsId.value}`, { method: 'POST' })
  }

  return { notifications: query.data, isLoading: query.isLoading, unread, markRead, markAllRead }
}
