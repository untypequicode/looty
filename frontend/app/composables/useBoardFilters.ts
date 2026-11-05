import { computed, type Ref } from 'vue'
import type { Board, Task } from '~/types'
import { useAuth } from './useAuth'
import { useWorkspace } from './useWorkspace'
import { useGroups } from './useGroups'
import { TASK_SORT_KEYS, defaultSortDir, sortTasks, type SortDir, type TaskSortKey } from '~/utils/taskSort'

export type DueFilter = 'overdue' | 'today' | 'week' | 'none'

/**
 * Filtres et tri du tableau, stockés dans l'URL (?q=&assignee=&label=&priority=&due=&done=&sort=&dir=)
 * pour qu'un lien partagé ouvre exactement la même vue filtrée et triée.
 */
export function useBoardFilters(board: Ref<Board | null | undefined>) {
  const route = useRoute()
  const router = useRouter()

  const list = (k: string) => computed<string[]>({
    get: () => {
      const v = route.query[k]
      return (Array.isArray(v) ? v : v ? [v] : []).flatMap((x) => String(x).split(',')).filter(Boolean)
    },
    set: (vals) => setQuery({ [k]: vals.length ? vals.join(',') : undefined }),
  })

  function setQuery(patch: Record<string, string | undefined>) {
    router.replace({ query: { ...route.query, ...patch }, hash: route.hash })
  }

  const q = computed<string>({ get: () => (route.query.q as string) || '', set: (v) => setQuery({ q: v || undefined }) })
  const assignees = list('assignee')
  const labels = list('label')
  const priorities = list('priority')
  const due = list('due') as unknown as Ref<DueFilter[]>
  /** Tâches terminées : affichées, masquées, ou masquées dès que ma part est faite. */
  const doneMode = computed<'show' | 'hide' | 'mine'>({
    get: () => (route.query.done === '0' ? 'hide' : route.query.done === 'me' ? 'mine' : 'show'),
    set: (v) => setQuery({ done: v === 'hide' ? '0' : v === 'mine' ? 'me' : undefined }),
  })
  const showDone = computed<boolean>({ get: () => doneMode.value === 'show', set: (v) => (doneMode.value = v ? 'show' : 'hide') })
  const { user } = useAuth()
  const showSubtasks = computed<boolean>({ get: () => route.query.sub === '1', set: (v) => setQuery({ sub: v ? '1' : undefined }) })

  /** Tri dans chaque colonne ; absent de l'URL = ordre manuel. Choisir un tri reprend son sens par défaut. */
  const sort = computed<TaskSortKey>({
    get: () => (TASK_SORT_KEYS.includes(route.query.sort as TaskSortKey) ? (route.query.sort as TaskSortKey) : 'manual'),
    set: (v) => setQuery({ sort: v === 'manual' ? undefined : v, dir: undefined }),
  })
  const sortDir = computed<SortDir>({
    get: () => (route.query.dir === 'asc' || route.query.dir === 'desc' ? route.query.dir : defaultSortDir(sort.value)),
    set: (v) => setQuery({ dir: v === defaultSortDir(sort.value) ? undefined : v }),
  })
  const { wsId, member } = useWorkspace()
  const { data: groups } = useGroups(wsId)
  const names = {
    member: (id: string) => member(id)?.name,
    group: (id: string) => groups.value?.find((g) => g.id === id)?.name,
  }

  const activeCount = computed(() => assignees.value.length + labels.value.length + priorities.value.length + due.value.length + (q.value ? 1 : 0) + (showDone.value ? 0 : 1))

  function clear() {
    setQuery({ q: undefined, assignee: undefined, label: undefined, priority: undefined, due: undefined, done: undefined })
  }

  function matches(t: Task): boolean {
    if (!showDone.value && t.is_completed) return false
    if (doneMode.value === 'mine' && t.is_per_person && user.value && t.validations.includes(user.value.id)) return false
    if (q.value && !t.title.toLowerCase().includes(q.value.toLowerCase())) return false
    if (assignees.value.length) {
      const wantNone = assignees.value.includes('none')
      const hit = t.assignees.some((a) => assignees.value.includes(a)) || (wantNone && !t.assignees.length)
      if (!hit) return false
    }
    if (labels.value.length && !t.labels.some((l) => labels.value.includes(l.id))) return false
    if (priorities.value.length && !priorities.value.includes(String(t.priority))) return false
    if (due.value.length) {
      const now = new Date()
      const today = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime()
      const d = t.due_date ? new Date(t.due_date) : null
      const day = d ? new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime() : null
      const ok = due.value.some((f) => {
        if (f === 'none') return day === null
        if (day === null) return false
        if (f === 'overdue') return day < today && !t.is_completed
        if (f === 'today') return day === today
        if (f === 'week') return day >= today && day <= today + 7 * 86400000
        return false
      })
      if (!ok) return false
    }
    return true
  }

  /** Tableau filtré : sous-tâches masquées par défaut (affichées sous leur parente). */
  const filtered = computed<Board | null>(() => {
    const b = board.value
    if (!b) return null
    return {
      ...b,
      columns: b.columns.map((c) => ({
        ...c,
        tasks: sortTasks(c.tasks.filter((t) => (showSubtasks.value || !t.parent_id) && matches(t)), sort.value, sortDir.value, names),
      })),
    }
  })

  return { q, assignees, labels, priorities, due, showDone, doneMode, showSubtasks, activeCount, clear, filtered, matches, sort, sortDir }
}

export type BoardFilters = ReturnType<typeof useBoardFilters>
