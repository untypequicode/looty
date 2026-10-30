import { computed, ref, type Ref } from 'vue'
import { useQuery } from '@tanstack/vue-query'
import { FileText, Kanban, CircleDot, Home, CalendarDays, Mail, Inbox, CircleUserRound, Trash2, Settings, EyeOff, Activity, Files, Waypoints, GraduationCap, FolderCode, HardDrive, PenTool } from 'lucide-vue-next'
import type { Task } from '~/types'
import { api } from './api'
import { links, useWorkspace } from './useWorkspace'
import { translate } from './useI18n'

const SCREENS: Record<string, { icon: any; path: (ws: string) => string }> = {
  home: { icon: Home, path: (ws) => links.home(ws) },
  calendar: { icon: CalendarDays, path: (ws) => links.calendar(ws) },
  mail: { icon: Mail, path: (ws) => links.mail(ws) },
  moodle: { icon: GraduationCap, path: (ws) => links.moodle(ws) },
  projects: { icon: FolderCode, path: (ws) => `/w/${ws}/projects` },
  drive: { icon: HardDrive, path: (ws) => `/w/${ws}/drive` },
  inbox: { icon: Inbox, path: (ws) => links.inbox(ws) },
  'my-tasks': { icon: CircleUserRound, path: (ws) => links.myTasks(ws) },
  docs: { icon: Files, path: (ws) => links.docs(ws) },
  trash: { icon: Trash2, path: (ws) => `/w/${ws}/trash` },
  settings: { icon: Settings, path: (ws) => links.settings(ws) },
  activity: { icon: Activity, path: (ws) => `/w/${ws}/activity` },
  graph: { icon: Waypoints, path: (ws) => `/w/${ws}/graph` },
  canvas: { icon: PenTool, path: (ws) => `/w/${ws}/canvas` },
}

/** Libellé, icône et lien d'un emplacement de présence. */
export function useLocationInfo(location: Ref<string | undefined>) {
  const { wsId, pages, boards } = useWorkspace()
  const kind = computed(() => location.value?.split(':')[0] ?? '')
  const id = computed(() => location.value?.split(':')[1] ?? '')
  const task = useQuery({
    queryKey: computed(() => ['presence-task', id.value]),
    queryFn: () => api<{ board_id: string; task: Task }>(`/api/boards/tasks/${id.value}`, { silent: true }),
    enabled: computed(() => kind.value === 'task' && !!id.value),
    staleTime: 60_000,
  })
  return computed(() => {
    const loc = location.value
    if (!loc) return null
    if (loc === 'hidden') return { icon: EyeOff, label: translate('presence.hidden'), path: null as string | null }
    const screen = SCREENS[loc]
    if (screen) return { icon: screen.icon, label: translate(`presence.screens.${loc}`), path: screen.path(wsId.value) }
    if (kind.value === 'page') {
      const p = pages.data.value?.find((x) => x.id === id.value)
      // Page inaccessible (droits restreints) : ni titre ni lien.
      return p ? { icon: FileText, emoji: p.icon, label: p.title || translate('docs.untitled'), path: links.doc(wsId.value, p.id) } : { icon: FileText, label: translate('presence.privateDoc'), path: null }
    }
    if (kind.value === 'board') {
      const b = boards.data.value?.find((x) => x.id === id.value)
      return { icon: Kanban, label: b?.name ?? translate('nav.boards'), path: links.board(wsId.value, id.value) }
    }
    if (kind.value === 'task') {
      const r = task.data.value
      return { icon: CircleDot, label: r?.task.title ?? '…', path: r ? links.task(wsId.value, r.board_id, r.task.id) : links.taskById(wsId.value, id.value) }
    }
    return { icon: Home, label: translate('presence.screens.home'), path: links.home(wsId.value) }
  })
}

/** Lien direct vers un emplacement (pour le suivi), sans libellé. */
export function locationPath(ws: string, location: string | undefined): string | null {
  if (!location || location === 'hidden') return null
  const screen = SCREENS[location]
  if (screen) return screen.path(ws)
  const [kind, id] = location.split(':')
  if (!id) return null
  if (kind === 'page') return links.doc(ws, id)
  if (kind === 'board') return links.board(ws, id)
  if (kind === 'task') return links.taskById(ws, id)
  return null
}

/** Emplacement correspondant à une route (même règle que la présence envoyée). */
export function locationOfRoute(route: { path: string; params: Record<string, any>; query: Record<string, any> }): string {
  const screens = ['calendar', 'mail', 'moodle', 'projects', 'drive', 'inbox', 'my-tasks', 'docs', 'trash', 'settings', 'activity', 'graph', 'canvas']
  const screen = route.path.split('/')[3] ?? ''
  const task = (route.query.task as string) || (route.params.task as string)
  return task ? `task:${task}` : route.params.page ? `page:${route.params.page}` : route.params.board ? `board:${route.params.board}` : screens.includes(screen) ? screen : 'home'
}

