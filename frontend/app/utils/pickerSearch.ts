// Logique pure du sélecteur « @ » de l'Assistant (filtrage par type, lieu affiché).

export interface PickerHit {
  kind: string
  ws: string
  wsName: string
  r: { type: string; title: string; date?: string | null; path?: string[] | null }
}

/** Le filtre « Moodle » couvre les cours et les fichiers de cours. */
export const matchesFilter = (kind: string, filter: string) => filter === 'all' || kind === filter || (filter === 'moodle' && kind.startsWith('moodle_'))

/** Résultats du type choisi (tous si `all`), limités en nombre. */
export function filterHits<T extends { kind: string }>(hits: readonly T[], filter: string, limit = 40): T[] {
  return hits.filter((h) => matchesFilter(h.kind, filter)).slice(0, limit)
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

// --- Regroupement par type, récents et favoris (palette « @ » de l'Assistant) ----------------------------

/** Ordre des groupes dans la liste : pages, entraînements, tâches, fichiers, le reste, puis Moodle (cours, fichiers de cours). */
export const GROUP_ORDER = ['page', 'study', 'task', 'file', 'board', 'event', 'project', 'moodle_course', 'moodle_file'] as const

/** Un élément de la liste, qu'il vienne de la recherche, des récents ou des favoris. */
export interface PickerEntry {
  key: string
  kind: string
  id: string
  ws: string
  title: string
  /** Lieu affiché (espace › pages parentes…). */
  place: string
  /** Extrait pour le volet d'aperçu. */
  excerpt: string
  /** Moodle : cours (identifiant et nom), et pour un fichier son adresse d'origine et sa taille. */
  courseId?: number
  courseName?: string
  url?: string
  size?: number
  /** Moodle : section et type d'un fichier, pour l'aperçu. */
  section?: string
  type?: string
}

export interface PickerSection<T> { id: string; items: T[] }

/** Résultats d'une recherche rangés par type (ordre `GROUP_ORDER`), `limit` éléments au plus en tout. */
export function groupByKind<T extends { kind: string }>(items: readonly T[], filter = 'all', limit = 40): PickerSection<T>[] {
  const kept = filterHits(items, filter, limit)
  const rank = (k: string) => { const i = (GROUP_ORDER as readonly string[]).indexOf(k); return i < 0 ? GROUP_ORDER.length : i }
  const ids = [...new Set(kept.map((h) => h.kind))].sort((a, b) => rank(a) - rank(b))
  return ids.map((id) => ({ id, items: kept.filter((h) => h.kind === id) }))
}

/** Éléments dans l'ordre d'affichage (pour la navigation au clavier). */
export const flatten = <T>(sections: readonly PickerSection<T>[]): T[] => sections.flatMap((s) => s.items)

/** Mémoire des éléments joints récemment (le plus récent d'abord, sans doublon, `max` au plus). */
export interface RecentItem { kind: string; id: string; ws: string; title: string }

export function pushRecent(list: readonly RecentItem[], item: RecentItem, max = 8): RecentItem[] {
  return [item, ...list.filter((r) => !(r.kind === item.kind && r.id === item.id))].slice(0, max)
}

/** Lecture tolérante du stockage local : tout ce qui n'a pas la bonne forme est écarté. */
export function parseRecents(raw: string | null | undefined): RecentItem[] {
  try {
    const v = JSON.parse(raw ?? '[]')
    return Array.isArray(v) ? v.filter((r) => r && typeof r.kind === 'string' && typeof r.id === 'string' && typeof r.ws === 'string' && typeof r.title === 'string').slice(0, 8) : []
  } catch { return [] }
}

/**
 * Liste affichée quand la recherche est vide : récents puis favoris (sans doublon avec les récents).
 * `favorites` vient des pages favorites de l'espace courant.
 */
export function emptyQuerySections<T extends { kind: string; id: string }>(recents: readonly T[], favorites: readonly T[], max = 6): PickerSection<T>[] {
  const shown = recents.slice(0, max)
  const seen = new Set(shown.map((r) => `${r.kind}:${r.id}`))
  const favs = favorites.filter((f) => !seen.has(`${f.kind}:${f.id}`)).slice(0, max)
  return [{ id: 'recent', items: [...shown] }, { id: 'favorite', items: favs }].filter((s) => s.items.length > 0)
}
