// Logique pure du sélecteur « @ » de l'Assistant (filtrage par type, lieu affiché).

export interface PickerHit {
  kind: string
  ws: string
  wsName: string
  r: { type: string; title: string; date?: string | null; path?: string[] | null }
}

/** Résultats du type choisi (tous si `all`), limités en nombre. */
export function filterHits<T extends { kind: string }>(hits: readonly T[], filter: string, limit = 40): T[] {
  return hits.filter((h) => filter === 'all' || h.kind === filter).slice(0, limit)
}

/**
 * Lieu d'un résultat : espace puis chemin complet (pages parentes d'une page ou d'un entraînement,
 * tableau d'une tâche, calendrier d'un événement) ; la date s'ajoute pour un événement.
 * L'espace n'est mentionné que si la personne en a plusieurs.
 */
export function hitPlace(h: PickerHit, opts: { multiWorkspace?: boolean; formatDate?: (iso: string) => string } = {}): string {
  const parts = [...(opts.multiWorkspace === false ? [] : [h.wsName]), ...(h.r.path ?? [])].filter(Boolean)
  const place = parts.join(' › ')
  if (h.r.type === 'event' && h.r.date) {
    const d = (opts.formatDate ?? ((iso: string) => new Date(iso).toLocaleDateString()))(h.r.date)
    return place ? `${place} · ${d}` : d
  }
  return place
}
