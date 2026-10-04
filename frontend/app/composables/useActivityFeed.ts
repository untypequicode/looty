import { computed, type Ref } from 'vue'
import { useInfiniteQuery } from '@tanstack/vue-query'
import { api } from './api'
import { translate } from './useI18n'

export interface FeedItem {
  id: string
  source: 'task' | 'comment' | 'page'
  type: string
  created_at: string
  user_id: string
  user_name?: string | null
  target_id: string
  title: string
  icon?: string | null
  board_id?: string | null
  board_name?: string | null
  old_value?: any
  new_value?: any
  snippet?: string | null
}

export function useActivityFeed(ws: Ref<string>, user: Ref<string | null>, limit = 50) {
  return useInfiniteQuery({
    queryKey: computed(() => ['ws', ws.value, 'activity', user.value ?? 'all', limit]),
    queryFn: ({ pageParam }) =>
      api<{ items: FeedItem[]; has_more: boolean }>(
        `/api/workspaces/${ws.value}/activity?limit=${limit}${user.value ? `&user=${user.value}` : ''}${pageParam ? `&before=${encodeURIComponent(pageParam)}` : ''}`,
      ),
    initialPageParam: '',
    getNextPageParam: (last) => (last.has_more && last.items.length ? last.items[last.items.length - 1]!.created_at : undefined),
    enabled: computed(() => !!ws.value),
    staleTime: 15_000,
  })
}

/** Verbe d'une action (« a terminé », « a modifié »…), à placer entre le nom et la cible. */
export function feedVerb(i: FeedItem): string {
  if (i.source === 'comment') return translate('feed.commented')
  if (i.source === 'page') return translate(`feed.page.${i.type}`) || translate('feed.page.edit')
  switch (i.type) {
    case 'create': return translate('feed.task.created')
    case 'move_column': return translate('feed.task.moved')
    case 'toggle_validation': return i.new_value?.is_completed ? translate('feed.task.completed') : translate('feed.task.reopened')
    case 'add_assignee': return translate('feed.task.assigned')
    case 'remove_assignee': return translate('feed.task.unassigned')
    case 'add_label': case 'remove_label': return translate('feed.task.labeled')
    case 'upload_attachment': return translate('feed.task.attached')
    case 'update_title': return translate('feed.task.renamed')
    case 'update_description': return translate('feed.task.described')
    case 'update_priority': return translate('feed.task.prioritized')
    case 'update_due_date': return translate('feed.task.scheduled')
    default: return translate('feed.task.updated')
  }
}
