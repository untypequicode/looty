import { computed, type Ref } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from './api'

export interface GraphNode {
  id: string
  kind: 'page' | 'task' | 'board' | 'event'
  label: string
  icon?: string
  done?: boolean
  board_id?: string
}
export interface GraphEdge {
  source: string
  target: string
  kind: 'parent' | 'board' | 'link' | 'relation'
  id?: string
}
export interface RelatedItem { node: GraphNode; kind: 'link' | 'relation'; incoming: boolean; link_id?: string | null }

export const graphKeys = {
  graph: (ws: string) => ['ws', ws, 'graph'] as const,
  related: (ws: string, id: string) => ['ws', ws, 'graph', 'related', id] as const,
}

export function useGraph(ws: Ref<string>) {
  return useQuery({
    queryKey: computed(() => graphKeys.graph(ws.value)),
    queryFn: () => api<{ nodes: GraphNode[]; edges: GraphEdge[] }>(`/api/workspaces/${ws.value}/graph`),
    enabled: computed(() => !!ws.value),
    staleTime: 30_000,
  })
}

export function useRelated(ws: Ref<string>, id: Ref<string | null>) {
  return useQuery({
    queryKey: computed(() => graphKeys.related(ws.value, id.value ?? '')),
    queryFn: () => api<{ items: RelatedItem[] }>(`/api/workspaces/${ws.value}/graph/related?id=${id.value}`),
    enabled: computed(() => !!ws.value && !!id.value),
    staleTime: 15_000,
  })
}

export function useGraphActions(ws: Ref<string>) {
  const qc = useQueryClient()
  const refresh = () => qc.invalidateQueries({ queryKey: ['ws', ws.value, 'graph'] })
  async function link(a: Pick<GraphNode, 'id' | 'kind'>, b: Pick<GraphNode, 'id' | 'kind'>) {
    await api(`/api/workspaces/${ws.value}/links`, { method: 'POST', body: { source_kind: a.kind, source_id: a.id, target_kind: b.kind, target_id: b.id } })
    refresh()
  }
  async function unlink(id: string) {
    await api(`/api/workspaces/${ws.value}/links/${id}`, { method: 'DELETE' })
    refresh()
  }
  return { link, unlink, refresh }
}

/** Lien vers un élément du graphe. */
export function nodeLink(ws: string, n: GraphNode): string {
  if (n.kind === 'page') return `/w/${ws}/doc/${n.id}`
  if (n.kind === 'board') return `/w/${ws}/board/${n.id}`
  if (n.kind === 'event') return `/w/${ws}/calendar?event=${n.id}`
  return n.board_id ? `/w/${ws}/board/${n.board_id}?task=${n.id}` : `/w/${ws}/task/${n.id}`
}
