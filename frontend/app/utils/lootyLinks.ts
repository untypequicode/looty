/**
 * Liens Looty (adresses de l'interface) → élément pointé. Sert aux cartes vivantes du
 * canevas : un lien collé ou inséré devient la carte de la tâche, du document, etc.
 */
export type LootyKind = 'page' | 'task' | 'board' | 'event' | 'project' | 'canvas'
export interface LootyRef { kind: LootyKind; ws: string; id: string; boardId?: string }

const UUID = '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}'
const RE = {
  task: new RegExp(`^/w/(${UUID})/task/(${UUID})`, 'i'),
  boardTask: new RegExp(`^/w/(${UUID})/board/(${UUID})`, 'i'),
  page: new RegExp(`^/w/(${UUID})/(?:doc|read)/(${UUID})`, 'i'),
  project: new RegExp(`^/w/(${UUID})/projects/(${UUID})`, 'i'),
  calendar: new RegExp(`^/w/(${UUID})/calendar`, 'i'),
}

/** Lien vers un élément de Looty sur ce serveur, ou `null`. */
export function parseLootyLink(raw: string, origin = typeof location !== 'undefined' ? location.origin : ''): LootyRef | null {
  let url: URL
  try { url = new URL(raw, origin || undefined) } catch { return null }
  if (origin && url.origin !== origin) return null
  const p = url.pathname
  let m = p.match(RE.task)
  if (m) return { kind: 'task', ws: m[1]!, id: m[2]! }
  m = p.match(RE.boardTask)
  if (m) {
    const task = url.searchParams.get('task')
    return task ? { kind: 'task', ws: m[1]!, id: task, boardId: m[2]! } : { kind: 'board', ws: m[1]!, id: m[2]! }
  }
  m = p.match(RE.page)
  if (m) return { kind: url.searchParams.get('view') === 'canvas' ? 'canvas' : 'page', ws: m[1]!, id: m[2]! }
  m = p.match(RE.project)
  if (m) return { kind: 'project', ws: m[1]!, id: m[2]! }
  m = p.match(RE.calendar)
  const ev = url.searchParams.get('event')
  if (m && ev && new RegExp(`^${UUID}$`, 'i').test(ev)) return { kind: 'event', ws: m[1]!, id: ev }
  return null
}

/** Adresse de l'élément (pour l'ouvrir dans Looty). */
export function lootyPath(r: LootyRef): string {
  switch (r.kind) {
    case 'task': return r.boardId ? `/w/${r.ws}/board/${r.boardId}?task=${r.id}` : `/w/${r.ws}/task/${r.id}`
    case 'board': return `/w/${r.ws}/board/${r.id}`
    case 'page': return `/w/${r.ws}/doc/${r.id}`
    case 'canvas': return `/w/${r.ws}/doc/${r.id}?view=canvas`
    case 'project': return `/w/${r.ws}/projects/${r.id}`
    case 'event': return `/w/${r.ws}/calendar?event=${r.id}`
  }
}

export const lootyUrl = (r: LootyRef) => `${location.origin}${lootyPath(r)}`
