/**
 * Les requêtes à rafraîchissement périodique (refetchInterval) sont suspendues par
 * TanStack Query quand l'onglet est masqué (refetchIntervalInBackground: false). Au retour,
 * celles dont les données sont plus vieilles que leur intervalle sont rechargées une fois.
 */
export interface PollableQuery {
  state: { dataUpdatedAt: number; fetchStatus?: string }
  getObserversCount(): number
  observers: { options: { refetchInterval?: unknown } }[]
}

/** Intervalle numérique le plus court parmi les observateurs, ou null. */
export function pollMs(q: PollableQuery): number | null {
  const ms = q.observers.map((o) => o.options.refetchInterval).filter((v): v is number => typeof v === 'number' && v > 0)
  return ms.length ? Math.min(...ms) : null
}

/** Requêtes actives à rafraîchissement périodique dont les données sont périmées. */
export function stalePolled<T extends PollableQuery>(queries: T[], now: number): T[] {
  return queries.filter((q) => {
    if (q.getObserversCount() === 0 || q.state.fetchStatus === 'fetching') return false
    const ms = pollMs(q)
    return ms !== null && now - q.state.dataUpdatedAt >= ms
  })
}
