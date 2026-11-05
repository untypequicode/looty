import type { Task } from '~/types'

/** Tri des tâches d'un tableau ; `manual` garde l'ordre choisi par glisser-déposer. */
export type TaskSortKey = 'manual' | 'start' | 'due' | 'created' | 'updated' | 'priority' | 'title' | 'status' | 'assignee' | 'label' | 'group'
export type SortDir = 'asc' | 'desc'

export const TASK_SORT_KEYS: TaskSortKey[] = ['manual', 'due', 'start', 'created', 'updated', 'priority', 'title', 'status', 'assignee', 'label', 'group']

/** Sens proposé quand on choisit un tri : les plus récentes d'abord pour création et modification. */
export function defaultSortDir(key: TaskSortKey): SortDir {
  return key === 'created' || key === 'updated' ? 'desc' : 'asc'
}

/** Noms des personnes et groupes, pour trier par assigné ou par groupe. */
export interface TaskSortNames {
  member: (id: string) => string | undefined
  group: (id: string) => string | undefined
}

type Value = number | string | null

const time = (d?: string | null): Value => {
  if (!d) return null
  const v = new Date(d).getTime()
  return Number.isNaN(v) ? null : v
}
/** Premier nom dans l'ordre alphabétique : une tâche à plusieurs assignés se range sous le premier. */
const firstName = (names: (string | undefined)[]): Value => {
  const list = names.filter((n): n is string => !!n).sort(compareText)
  return list[0] ?? null
}
const collator = new Intl.Collator(undefined, { sensitivity: 'base', numeric: true })
function compareText(a: string, b: string) { return collator.compare(a, b) }

/** Valeur de tri d'une tâche ; `null` = valeur absente (toujours rangée en bas). */
export function sortValue(t: Task, key: TaskSortKey, names: TaskSortNames): Value {
  switch (key) {
    case 'start': return time(t.start_date)
    case 'due': return time(t.due_date)
    case 'created': return time(t.created_at)
    case 'updated': return time(t.updated_at)
    // 1 = urgente … 4 = basse ; 0 = aucune priorité, traitée comme absente.
    case 'priority': return t.priority ? t.priority : null
    case 'title': return t.title.trim() || null
    case 'status': return t.is_completed ? 1 : 0
    case 'assignee': return firstName(t.assignees.map(names.member))
    case 'label': return firstName(t.labels.map((l) => l.name))
    case 'group': return firstName((t.groups ?? []).map(names.group))
    default: return null
  }
}

/**
 * Trie les tâches d'une colonne. Les valeurs absentes (sans date, sans assigné…) restent en
 * bas quel que soit le sens ; à valeur égale, l'ordre manuel départage.
 */
export function sortTasks(tasks: Task[], key: TaskSortKey, dir: SortDir, names: TaskSortNames): Task[] {
  if (key === 'manual') return tasks
  const sign = dir === 'desc' ? -1 : 1
  const keyed = tasks.map((t) => ({ t, v: sortValue(t, key, names) }))
  keyed.sort((a, b) => {
    if (a.v === null || b.v === null) {
      if (a.v !== b.v) return a.v === null ? 1 : -1
      return a.t.position - b.t.position
    }
    const c = typeof a.v === 'string' && typeof b.v === 'string' ? compareText(a.v, b.v) : (a.v as number) - (b.v as number)
    return c ? sign * c : a.t.position - b.t.position
  })
  return keyed.map((x) => x.t)
}
