import { computed, type Ref } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import type { BoardSummary, Page, Workspace, WorkspaceMember, MyTask } from '~/types'
import { api } from './api'

export const wsKeys = {
  list: ['workspaces'] as const,
  one: (id: string) => ['ws', id] as const,
  members: (id: string) => ['ws', id, 'members'] as const,
  boards: (id: string) => ['ws', id, 'boards'] as const,
  pages: (id: string) => ['ws', id, 'pages'] as const,
  myTasks: (id: string) => ['ws', id, 'my-tasks'] as const,
  notifications: (id: string) => ['ws', id, 'notifications'] as const,
}

export function useWorkspaces() {
  return useQuery({
    queryKey: wsKeys.list,
    queryFn: async () => (await api<{ workspaces: Workspace[] }>('/api/workspaces')).workspaces,
  })
}

/** Identifiant du workspace courant, déduit de la route `/w/:ws/...`. */
export function useWorkspaceId(): Ref<string> {
  const route = useRoute()
  return computed(() => (route.params.ws as string) || '')
}

/**
 * Données partagées du workspace courant (membres, tableaux, pages).
 * Les requêtes sont mises en cache par Vue Query et tenues à jour par le temps réel.
 */
export function useWorkspace() {
  const wsId = useWorkspaceId()
  const enabled = computed(() => !!wsId.value)
  const qc = useQueryClient()

  const workspace = useQuery({
    queryKey: computed(() => wsKeys.one(wsId.value)),
    queryFn: () => api<Workspace>(`/api/workspaces/${wsId.value}`, { silent: true }),
    enabled,
  })
  const members = useQuery({
    queryKey: computed(() => wsKeys.members(wsId.value)),
    queryFn: async () => (await api<{ members: WorkspaceMember[] }>(`/api/workspaces/${wsId.value}/members`)).members,
    enabled,
  })
  const boards = useQuery({
    queryKey: computed(() => wsKeys.boards(wsId.value)),
    queryFn: () => api<BoardSummary[]>(`/api/workspaces/${wsId.value}/boards`),
    enabled,
  })
  const pages = useQuery({
    queryKey: computed(() => wsKeys.pages(wsId.value)),
    queryFn: () => api<Page[]>(`/api/workspaces/${wsId.value}/pages`),
    enabled,
  })

  const role = computed(() => workspace.data.value?.role ?? null)
  const canWrite = computed(() => role.value === 'owner' || role.value === 'admin' || role.value === 'member')
  const isAdmin = computed(() => role.value === 'owner' || role.value === 'admin')

  const memberById = computed(() => {
    const m = new Map<string, WorkspaceMember>()
    for (const x of members.data.value ?? []) m.set(x.user_id, x)
    return m
  })

  function member(id: string | null | undefined) {
    return id ? memberById.value.get(id) : undefined
  }

  function invalidate(part: 'members' | 'boards' | 'pages' | 'one') {
    const k = part === 'one' ? wsKeys.one(wsId.value) : (wsKeys as any)[part](wsId.value)
    return qc.invalidateQueries({ queryKey: k })
  }

  return { wsId, workspace, members, boards, pages, role, canWrite, isAdmin, member, memberById, invalidate }
}

export function useMyTasks() {
  const wsId = useWorkspaceId()
  return useQuery({
    queryKey: computed(() => wsKeys.myTasks(wsId.value)),
    queryFn: async () => (await api<{ tasks: MyTask[] }>(`/api/workspaces/${wsId.value}/my_tasks`)).tasks,
    enabled: computed(() => !!wsId.value),
  })
}

/** Liens canoniques : chaque ressource a sa propre URL partageable. */
export const links = {
  home: (ws: string) => `/w/${ws}`,
  inbox: (ws: string) => `/w/${ws}/inbox`,
  myTasks: (ws: string) => `/w/${ws}/my-tasks`,
  docs: (ws: string) => `/w/${ws}/docs`,
  board: (ws: string, board: string) => `/w/${ws}/board/${board}`,
  task: (ws: string, board: string, task: string) => `/w/${ws}/board/${board}?task=${task}`,
  taskById: (ws: string, task: string) => `/w/${ws}/task/${task}`,
  doc: (ws: string, page: string, anchor?: string) => `/w/${ws}/doc/${page}${anchor ? `#${anchor}` : ''}`,
  settings: (ws: string, section = 'profile') => `/w/${ws}/settings/${section}`,
}
