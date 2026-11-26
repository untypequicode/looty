
/**
 * Ancres des commentaires d'une présentation. Les commentaires sont ceux de la page (table
 * item_comments) ; la colonne `anchor` désigne une diapositive (`slide:<id>`) ou un élément
 * (`el:<diapositive>:<élément>`). Sans ancre, le commentaire concerne toute la présentation.
 * Le même format est validé par backend/src/comments.rs.
 */

export const slideAnchor = (slideId: string) => `slide:${slideId}`
export const elementAnchor = (slideId: string, elementId: string) => `el:${slideId}:${elementId}`

const ID = '[A-Za-z0-9_-]{1,64}'
const RE_SLIDE = new RegExp(`^slide:(${ID})$`)
const RE_EL = new RegExp(`^el:(${ID}):(${ID})$`)

export type ParsedAnchor = { kind: 'slide'; slide: string } | { kind: 'element'; slide: string; element: string }

export function parseAnchor(a: string | null | undefined): ParsedAnchor | null {
  if (!a) return null
  const s = RE_SLIDE.exec(a)
  if (s) return { kind: 'slide', slide: s[1]! }
  const e = RE_EL.exec(a)
  return e ? { kind: 'element', slide: e[1]!, element: e[2]! } : null
}

export interface ThreadLike { anchor: string | null; resolved: boolean }

export interface AnchorCounts {
  /** Fils ouverts par diapositive (ceux de la diapositive et de ses éléments). */
  bySlide: Map<string, number>
  /** Fils ouverts par élément, clé `<diapositive>:<élément>`. */
  byElement: Map<string, number>
  /** Fils ouverts sans ancre. */
  general: number
}

/** Pastilles : nombre de fils ouverts (non résolus) par diapositive et par élément. */
export function anchorCounts(threads: readonly ThreadLike[]): AnchorCounts {
  const out: AnchorCounts = { bySlide: new Map(), byElement: new Map(), general: 0 }
  for (const t of threads) {
    if (t.resolved) continue
    const p = parseAnchor(t.anchor)
    if (!p) { out.general++; continue }
    out.bySlide.set(p.slide, (out.bySlide.get(p.slide) ?? 0) + 1)
    if (p.kind === 'element') out.byElement.set(`${p.slide}:${p.element}`, (out.byElement.get(`${p.slide}:${p.element}`) ?? 0) + 1)
  }
  return out
}

export type AnchorTarget =
  | { state: 'general' }
  | { state: 'slide'; slide: string; index: number }
  | { state: 'element'; slide: string; element: string; index: number }
  | { state: 'missing' }

/** Cible actuelle d'une ancre : elle peut avoir disparu (diapositive ou élément supprimé). */
export function anchorTarget(anchor: string | null | undefined, slides: readonly { id: string; elements: readonly { id: string }[] }[]): AnchorTarget {
  if (!anchor) return { state: 'general' }
  const p = parseAnchor(anchor)
  if (!p) return { state: 'missing' }
  const index = slides.findIndex((s) => s.id === p.slide)
  if (index < 0) return { state: 'missing' }
  if (p.kind === 'slide') return { state: 'slide', slide: p.slide, index }
  return slides[index]!.elements.some((e) => e.id === p.element) ? { state: 'element', slide: p.slide, element: p.element, index } : { state: 'missing' }
}
