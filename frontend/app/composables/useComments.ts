import { computed, type Ref } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from './api'

/** Commentaires d'une page ou d'un événement (les tâches ont les leurs). */
export type CommentKind = 'page' | 'event'
export interface ItemComment {
  id: string
  target_kind: CommentKind
  target_id: string
  parent_id: string | null
  user_id: string | null
  user_name: string | null
  avatar_url: string | null
  content: string
  quote: string | null
  resolved_at: string | null
  resolved_by: string | null
  created_at: string
  updated_at: string
}
export interface CommentThread { root: ItemComment; replies: ItemComment[] }

export const commentKeys = {
  list: (ws: string, kind: CommentKind, id: string) => ['ws', ws, 'comments', kind, id],
  counts: (ws: string, kind: CommentKind) => ['ws', ws, 'comment-counts', kind],
}

export function useComments(ws: Ref<string>, kind: Ref<CommentKind>, id: Ref<string | null | undefined>) {
  const qc = useQueryClient()
  const key = computed(() => commentKeys.list(ws.value, kind.value, id.value ?? ''))
  const query = useQuery({
    queryKey: key,
    queryFn: () => api<ItemComment[]>(`/api/workspaces/${ws.value}/comments?kind=${kind.value}&id=${id.value}`, { silent: true }),
    enabled: computed(() => !!ws.value && !!id.value),
  })
  const threads = computed<CommentThread[]>(() => {
    const all = query.data.value ?? []
    return all.filter((c) => !c.parent_id).map((root) => ({ root, replies: all.filter((c) => c.parent_id === root.id) }))
  })
  const openCount = computed(() => threads.value.filter((t) => !t.root.resolved_at).length)

  function refresh() {
    qc.invalidateQueries({ queryKey: key.value })
    qc.invalidateQueries({ queryKey: commentKeys.counts(ws.value, kind.value) })
  }
  async function add(content: string, opts: { parentId?: string; quote?: string } = {}) {
    const c = await api<ItemComment>(`/api/workspaces/${ws.value}/comments`, { method: 'POST', body: { kind: kind.value, id: id.value, content, parent_id: opts.parentId, quote: opts.quote } })
    qc.setQueryData<ItemComment[]>(key.value, (old) => [...(old ?? []), c])
    refresh()
    return c
  }
  async function edit(c: ItemComment, content: string) {
    await api(`/api/comments/${c.id}`, { method: 'PUT', body: { content } })
    refresh()
  }
  async function remove(c: ItemComment) {
    qc.setQueryData<ItemComment[]>(key.value, (old) => (old ?? []).filter((x) => x.id !== c.id && x.parent_id !== c.id))
    await api(`/api/comments/${c.id}`, { method: 'DELETE' })
    refresh()
  }
  async function resolve(c: ItemComment, resolved: boolean) {
    qc.setQueryData<ItemComment[]>(key.value, (old) => (old ?? []).map((x) => (x.id === c.id ? { ...x, resolved_at: resolved ? new Date().toISOString() : null } : x)))
    await api(`/api/comments/${c.id}/resolve`, { method: 'POST', body: { resolved } })
    refresh()
  }
  return { comments: query.data, threads, openCount, isLoading: query.isLoading, add, edit, remove, resolve }
}

/** Nombre de fils ouverts par élément (pastilles). */
export function useCommentCounts(ws: Ref<string>, kind: CommentKind) {
  return useQuery({
    queryKey: computed(() => commentKeys.counts(ws.value, kind)),
    queryFn: () => api<Record<string, number>>(`/api/workspaces/${ws.value}/comments/counts?kind=${kind}`, { silent: true }),
    enabled: computed(() => !!ws.value),
    staleTime: 60_000,
  })
}
