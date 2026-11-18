// Calcul pur du rangement d'un élément de l'arbre (pages et entraînements mêlés).

export interface TreeEntry { id: string; parent_id: string | null; position: number; study?: boolean }
export type DropWhere = 'before' | 'after' | 'inside'

/** Un entraînement n'a pas de sous-éléments : déposé « dedans », il se range après lui. */
export function effectiveWhere(target: TreeEntry, where: DropWhere): DropWhere {
  return target.study && where === 'inside' ? 'after' : where
}

/** Nouveau parent et nouvelle position, ou `null` si le déplacement est interdit (cycle, soi-même). */
export function computeMove(all: TreeEntry[], id: string, targetId: string, whereIn: DropWhere): { parentId: string | null; position: number } | null {
  const target = all.find((p) => p.id === targetId)
  if (!target || id === targetId) return null
  const where = effectiveWhere(target, whereIn)
  let parentId: string | null
  let position: number
  if (where === 'inside') {
    parentId = target.id
    const kids = all.filter((p) => p.parent_id === target.id && p.id !== id)
    position = (kids.reduce((m, p) => Math.max(m, p.position), 0) || 0) + 65536
  } else {
    parentId = target.parent_id ?? null
    const siblings = all.filter((p) => (p.parent_id ?? null) === parentId && p.id !== id).sort((a, b) => a.position - b.position)
    const idx = siblings.findIndex((p) => p.id === target.id)
    const before = where === 'before' ? siblings[idx - 1] : target
    const after = where === 'before' ? target : siblings[idx + 1]
    const lo = before?.position ?? 0
    const hi = after?.position ?? lo + 131072
    position = (lo + hi) / 2
  }
  // Interdit de déplacer une page dans sa propre descendance.
  let cur: string | null = parentId
  let guard = 0
  while (cur && guard++ < 256) {
    if (cur === id) return null
    cur = all.find((p) => p.id === cur)?.parent_id ?? null
  }
  return { parentId, position }
}

/** Position en dernier parmi les éléments racine. */
export function lastRootPosition(all: TreeEntry[]): number {
  return all.filter((p) => !p.parent_id).reduce((m, p) => Math.max(m, p.position), 0) + 65536
}
