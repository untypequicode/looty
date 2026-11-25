import * as Y from 'yjs'
import { align as alignBoxes, distribute as distributeBoxes, type AlignMode, type Box } from './geometry.ts'

/**
 * Modèle d'une présentation (ADR 0022) : un document Yjs « slides:<page> ».
 *
 *   meta      Y.Map   schema, width, height
 *   slides    Y.Map   id -> diapositive (Y.Map : id, pos, layout, background, notes, elements)
 *   elements  Y.Map   id -> élément (Y.Map : id, type, x, y, w, h, rotation, z, locked, groupId, alt,
 *                     props (Y.Map de scalaires), et `text` (Y.XmlFragment) pour les zones de texte)
 *
 * Chaque valeur est sa propre clé : deux personnes qui déplacent et recolorent le même élément
 * fusionnent. L'ordre des diapositives est un indice fractionnaire (`pos`), l'empilement un `z`.
 * Tout ce qui écrit passe par `LOCAL` pour que l'annulation ne suive que soi.
 */

/** Version du format : à augmenter avec `SLIDES_SCHEMA` de collaboration/index.js. */
export const SLIDES_SCHEMA = 1
export const SLIDE_W = 1920
export const SLIDE_H = 1080
export const MAX_SLIDES = 300
export const MAX_ELEMENTS = 500
export const MAX_TEXT = 20000
export const LOCAL = 'looty-slides-local'

export type ElementType = 'text' | 'rect' | 'ellipse' | 'line' | 'image' | 'group'
export type PropValue = string | number | boolean
export type Props = Record<string, PropValue>

export interface SlideElement {
  id: string
  type: ElementType
  x: number
  y: number
  w: number
  h: number
  rotation: number
  z: number
  locked: boolean
  groupId: string
  alt: string
  props: Props
}

export interface Slide {
  id: string
  pos: number
  layout: string
  background: string
  notes: string
  elements: SlideElement[]
}

export interface ElementSpec {
  type: ElementType
  x: number
  y: number
  w: number
  h: number
  rotation?: number
  props?: Props
  alt?: string
  /** Texte initial d'une zone de texte (un paragraphe par ligne). */
  text?: string
}

export type ElementPatch = Partial<Pick<SlideElement, 'x' | 'y' | 'w' | 'h' | 'rotation' | 'locked' | 'groupId' | 'alt'>> & { props?: Record<string, PropValue | undefined> }

export function newId(): string {
  const c = globalThis.crypto
  if (c?.randomUUID) return c.randomUUID()
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`
}

export const metaOf = (doc: Y.Doc) => doc.getMap<any>('meta')
export const slidesOf = (doc: Y.Doc) => doc.getMap<Y.Map<any>>('slides')

function slideMap(doc: Y.Doc, id: string): Y.Map<any> | undefined {
  return slidesOf(doc).get(id)
}
function elementsMap(doc: Y.Doc, slideId: string): Y.Map<Y.Map<any>> | undefined {
  return slideMap(doc, slideId)?.get('elements')
}

const num = (v: unknown, d = 0) => (typeof v === 'number' && Number.isFinite(v) ? v : d)
const str = (v: unknown, d = '') => (typeof v === 'string' ? v : d)

function readElement(m: Y.Map<any>): SlideElement {
  const props: Props = {}
  const p = m.get('props')
  if (p instanceof Y.Map) p.forEach((v, k) => { props[k] = v })
  return {
    id: str(m.get('id')),
    type: str(m.get('type'), 'rect') as ElementType,
    x: num(m.get('x')), y: num(m.get('y')), w: num(m.get('w'), 100), h: num(m.get('h'), 100),
    rotation: num(m.get('rotation')), z: num(m.get('z')),
    locked: m.get('locked') === true, groupId: str(m.get('groupId')), alt: str(m.get('alt')),
    props,
  }
}

/** Instantané d'une diapositive (éléments triés du fond vers le dessus). */
export function readSlide(doc: Y.Doc, id: string): Slide | null {
  const s = slideMap(doc, id)
  if (!s) return null
  const els = s.get('elements') as Y.Map<Y.Map<any>> | undefined
  const elements: SlideElement[] = []
  els?.forEach((m) => { if (m instanceof Y.Map) elements.push(readElement(m)) })
  elements.sort((a, b) => a.z - b.z || (a.id < b.id ? -1 : 1))
  const notes = s.get('notes')
  return {
    id, pos: num(s.get('pos')), layout: str(s.get('layout'), 'blank'), background: str(s.get('background'), 'default'),
    notes: notes instanceof Y.Text ? notes.toString() : str(notes), elements,
  }
}

/** Identifiants des diapositives dans l'ordre (indice, puis identifiant pour départager). */
export function slideIds(doc: Y.Doc): string[] {
  const rows: { id: string; pos: number }[] = []
  slidesOf(doc).forEach((s, id) => rows.push({ id, pos: num(s.get('pos')) }))
  rows.sort((a, b) => a.pos - b.pos || (a.id < b.id ? -1 : 1))
  return rows.map((r) => r.id)
}

export function listSlides(doc: Y.Doc): Slide[] {
  return slideIds(doc).map((id) => readSlide(doc, id)!).filter(Boolean)
}

export function slideSize(doc: Y.Doc): { w: number; h: number } {
  const m = metaOf(doc)
  return { w: num(m.get('width'), SLIDE_W), h: num(m.get('height'), SLIDE_H) }
}

export function textOf(doc: Y.Doc, slideId: string, elementId: string): Y.XmlFragment | null {
  const f = elementsMap(doc, slideId)?.get(elementId)?.get('text')
  return f instanceof Y.XmlFragment ? f : null
}

/** Document neuf : méta et une diapositive vide. Sans effet sur un document déjà initialisé. */
export function initDoc(doc: Y.Doc): void {
  doc.transact(() => {
    const meta = metaOf(doc)
    if (!meta.has('schema')) {
      meta.set('schema', SLIDES_SCHEMA)
      meta.set('width', SLIDE_W)
      meta.set('height', SLIDE_H)
    }
    if (slidesOf(doc).size === 0) createSlide(doc)
  }, LOCAL)
}

/** Version du format du document (0 : jamais initialisé). */
export function docSchema(doc: Y.Doc): number {
  return num(metaOf(doc).get('schema'))
}

/** Document écrit par une version plus récente que ce client : à ne pas modifier. */
export function isNewerThanClient(doc: Y.Doc): boolean {
  return docSchema(doc) > SLIDES_SCHEMA
}

/** Point d'entrée des migrations (ADR 0022) : une seule version pour l'instant, rien à convertir. */
export function migrate(_doc: Y.Doc): boolean {
  return false
}

/** Indice entre deux voisins (le premier et le dernier ouvrent la suite). */
export function posBetween(before?: number, after?: number): number {
  if (before === undefined && after === undefined) return 1
  if (before === undefined) return after! - 1
  if (after === undefined) return before + 1
  return (before + after) / 2
}

function renumber(doc: Y.Doc): void {
  slideIds(doc).forEach((id, i) => slideMap(doc, id)!.set('pos', i + 1))
}

/** Les indices se rapprochent après beaucoup d'insertions au même endroit : on renumérote alors. */
function needsRenumber(doc: Y.Doc): boolean {
  const pos = slideIds(doc).map((id) => num(slideMap(doc, id)!.get('pos')))
  return pos.some((p, i) => i > 0 && p - pos[i - 1]! < 1e-6)
}

function insertPos(doc: Y.Doc, afterId?: string | null): number {
  const ids = slideIds(doc)
  const at = afterId ? ids.indexOf(afterId) : ids.length - 1
  const before = at >= 0 ? num(slideMap(doc, ids[at]!)!.get('pos')) : undefined
  const next = at >= 0 && at + 1 < ids.length ? num(slideMap(doc, ids[at + 1]!)!.get('pos')) : undefined
  return posBetween(before, next)
}

export interface SlideSpec { background?: string; layout?: string; afterId?: string | null }

/** Nouvelle diapositive vide après `afterId` (à la fin par défaut). Renvoie null au-delà de la limite. */
export function createSlide(doc: Y.Doc, spec: SlideSpec = {}): string | null {
  if (slidesOf(doc).size >= MAX_SLIDES) return null
  const id = newId()
  doc.transact(() => {
    const pos = insertPos(doc, spec.afterId)
    const s = new Y.Map<any>()
    slidesOf(doc).set(id, s)
    s.set('id', id)
    s.set('pos', pos)
    s.set('layout', spec.layout ?? 'blank')
    s.set('background', spec.background ?? 'default')
    s.set('notes', new Y.Text())
    s.set('elements', new Y.Map())
    if (needsRenumber(doc)) renumber(doc)
  }, LOCAL)
  return id
}

function cloneElement(src: Y.Map<any>, id: string, z: number, dx = 0, dy = 0): Y.Map<any> {
  const m = new Y.Map<any>()
  src.forEach((v, k) => {
    if (k === 'props' && v instanceof Y.Map) {
      const p = new Y.Map<any>()
      v.forEach((pv, pk) => p.set(pk, pv))
      m.set('props', p)
    } else if (v instanceof Y.XmlFragment) m.set(k, v.clone())
    else m.set(k, v)
  })
  m.set('id', id)
  m.set('z', z)
  m.set('x', num(src.get('x')) + dx)
  m.set('y', num(src.get('y')) + dy)
  return m
}

/** Copie une diapositive juste après elle (nouveaux identifiants, groupes conservés entre eux). */
export function duplicateSlide(doc: Y.Doc, id: string): string | null {
  const src = slideMap(doc, id)
  if (!src || slidesOf(doc).size >= MAX_SLIDES) return null
  const nid = newId()
  doc.transact(() => {
    const pos = insertPos(doc, id)
    const s = new Y.Map<any>()
    slidesOf(doc).set(nid, s)
    s.set('id', nid)
    s.set('pos', pos)
    s.set('layout', str(src.get('layout'), 'blank'))
    s.set('background', str(src.get('background'), 'default'))
    const notes = new Y.Text()
    s.set('notes', notes)
    const sn = src.get('notes')
    const text = sn instanceof Y.Text ? sn.toString() : str(sn)
    if (text) notes.insert(0, text)
    const els = new Y.Map<Y.Map<any>>()
    s.set('elements', els)
    const groupMap = new Map<string, string>()
    ;(src.get('elements') as Y.Map<Y.Map<any>>).forEach((m) => {
      const e = cloneElement(m, newId(), num(m.get('z')))
      const g = str(m.get('groupId'))
      if (g) {
        if (!groupMap.has(g)) groupMap.set(g, newId())
        e.set('groupId', groupMap.get(g))
      }
      els.set(str(e.get('id')), e)
    })
    if (needsRenumber(doc)) renumber(doc)
  }, LOCAL)
  return nid
}

/** Supprime une diapositive ; la dernière est remplacée par une diapositive vide. Renvoie celle à afficher ensuite. */
export function deleteSlide(doc: Y.Doc, id: string): string | null {
  const ids = slideIds(doc)
  const at = ids.indexOf(id)
  if (at < 0) return ids[0] ?? null
  let next: string | null = null
  doc.transact(() => {
    slidesOf(doc).delete(id)
    next = ids.length === 1 ? createSlide(doc) : ids[at + 1] ?? ids[at - 1] ?? null
  }, LOCAL)
  return next
}

/** Déplace une diapositive à la position `toIndex` (dans la liste sans elle). */
export function moveSlide(doc: Y.Doc, id: string, toIndex: number): void {
  if (!slideMap(doc, id)) return
  const ids = slideIds(doc).filter((x) => x !== id)
  const at = Math.max(0, Math.min(toIndex, ids.length))
  const before = at > 0 ? num(slideMap(doc, ids[at - 1]!)!.get('pos')) : undefined
  const after = at < ids.length ? num(slideMap(doc, ids[at]!)!.get('pos')) : undefined
  doc.transact(() => {
    slideMap(doc, id)!.set('pos', posBetween(before, after))
    if (needsRenumber(doc)) renumber(doc)
  }, LOCAL)
}

export function setSlideBackground(doc: Y.Doc, id: string, background: string): void {
  doc.transact(() => slideMap(doc, id)?.set('background', background), LOCAL)
}

function topZ(els: Y.Map<Y.Map<any>>): number {
  let z = -1
  els.forEach((m) => { z = Math.max(z, num(m.get('z'))) })
  return z + 1
}

/** Ajoute un élément au-dessus des autres. Renvoie son identifiant, ou null au-delà de la limite. */
export function addElement(doc: Y.Doc, slideId: string, spec: ElementSpec): string | null {
  const els = elementsMap(doc, slideId)
  if (!els || els.size >= MAX_ELEMENTS) return null
  const id = newId()
  doc.transact(() => {
    const m = new Y.Map<any>()
    els.set(id, m)
    m.set('id', id)
    m.set('type', spec.type)
    m.set('x', spec.x); m.set('y', spec.y); m.set('w', spec.w); m.set('h', spec.h)
    m.set('rotation', spec.rotation ?? 0)
    m.set('z', topZ(els))
    m.set('locked', false)
    m.set('groupId', '')
    m.set('alt', spec.alt ?? '')
    const p = new Y.Map<any>()
    m.set('props', p)
    for (const [k, v] of Object.entries(spec.props ?? {})) p.set(k, v)
    if (spec.type === 'text') {
      const f = new Y.XmlFragment()
      m.set('text', f)
      if (spec.text) {
        for (const line of spec.text.slice(0, MAX_TEXT).split('\n')) {
          const para = new Y.XmlElement('paragraph')
          f.push([para])
          if (line) {
            const t = new Y.XmlText()
            para.push([t])
            t.insert(0, line)
          }
        }
      }
    }
  }, LOCAL)
  return id
}

/** Modifie des champs d'un élément : seules les clés données sont écrites (fusion fine entre personnes). */
export function updateElement(doc: Y.Doc, slideId: string, id: string, patch: ElementPatch): void {
  const m = elementsMap(doc, slideId)?.get(id)
  if (!m) return
  doc.transact(() => {
    for (const [k, v] of Object.entries(patch)) {
      if (k === 'props' && v) {
        let p = m.get('props')
        if (!(p instanceof Y.Map)) { p = new Y.Map(); m.set('props', p) }
        for (const [pk, pv] of Object.entries(v as Record<string, PropValue | undefined>)) {
          if (pv === undefined || pv === null) p.delete(pk)
          else if (p.get(pk) !== pv) p.set(pk, pv)
        }
      } else if (v !== undefined && m.get(k) !== v) m.set(k, v)
    }
  }, LOCAL)
}

/** Applique des modifications à plusieurs éléments en une transaction (une seule annulation). */
export function updateElements(doc: Y.Doc, slideId: string, patches: Map<string, ElementPatch>): void {
  doc.transact(() => { for (const [id, p] of patches) updateElement(doc, slideId, id, p) }, LOCAL)
}

export function deleteElements(doc: Y.Doc, slideId: string, ids: readonly string[]): void {
  const els = elementsMap(doc, slideId)
  if (!els) return
  doc.transact(() => { for (const id of ids) els.delete(id) }, LOCAL)
}

/** Copie des éléments (même diapositive ou une autre) décalés ; renvoie les nouveaux identifiants. */
export function duplicateElements(doc: Y.Doc, fromSlide: string, ids: readonly string[], toSlide = fromSlide, dx = 24, dy = 24): string[] {
  const src = elementsMap(doc, fromSlide)
  const dst = elementsMap(doc, toSlide)
  if (!src || !dst) return []
  const out: string[] = []
  doc.transact(() => {
    const groups = new Map<string, string>()
    let z = topZ(dst)
    const ordered = ids.map((i) => src.get(i)).filter((m): m is Y.Map<any> => !!m).sort((a, b) => num(a.get('z')) - num(b.get('z')))
    for (const m of ordered) {
      if (dst.size >= MAX_ELEMENTS) break
      const id = newId()
      const c = cloneElement(m, id, z++, dx, dy)
      const g = str(m.get('groupId'))
      if (g) {
        if (!groups.has(g)) groups.set(g, newId())
        c.set('groupId', groups.get(g))
      }
      dst.set(id, c)
      out.push(id)
    }
  }, LOCAL)
  return out
}

export type ZMove = 'front' | 'back' | 'forward' | 'backward'

/** Nouvel ordre des identifiants après un déplacement de la sélection (fonction pure). */
export function reorderIds(order: readonly string[], selected: ReadonlySet<string>, move: ZMove): string[] {
  const rest = order.filter((i) => !selected.has(i))
  const sel = order.filter((i) => selected.has(i))
  if (move === 'front') return [...rest, ...sel]
  if (move === 'back') return [...sel, ...rest]
  const out = [...order]
  if (move === 'forward') {
    for (let i = out.length - 2; i >= 0; i--) if (selected.has(out[i]!) && !selected.has(out[i + 1]!)) [out[i], out[i + 1]] = [out[i + 1]!, out[i]!]
  } else {
    for (let i = 1; i < out.length; i++) if (selected.has(out[i]!) && !selected.has(out[i - 1]!)) [out[i], out[i - 1]] = [out[i - 1]!, out[i]!]
  }
  return out
}

/** Déplace la sélection dans l'empilement et réécrit les rangs en 0..n-1. */
export function moveZ(doc: Y.Doc, slideId: string, ids: readonly string[], move: ZMove): void {
  const s = readSlide(doc, slideId)
  const els = elementsMap(doc, slideId)
  if (!s || !els) return
  const order = reorderIds(s.elements.map((e) => e.id), new Set(ids), move)
  doc.transact(() => order.forEach((id, i) => { const m = els.get(id); if (m && num(m.get('z')) !== i) m.set('z', i) }), LOCAL)
}

/** Ajoute à une sélection tous les éléments des groupes touchés. */
export function expandGroups(elements: readonly SlideElement[], ids: readonly string[]): string[] {
  const groups = new Set(elements.filter((e) => ids.includes(e.id) && e.groupId).map((e) => e.groupId))
  const out = new Set(ids)
  for (const e of elements) if (e.groupId && groups.has(e.groupId)) out.add(e.id)
  return [...out]
}

/** Regroupe deux éléments ou plus ; renvoie l'identifiant du groupe. */
export function groupElements(doc: Y.Doc, slideId: string, ids: readonly string[]): string | null {
  if (ids.length < 2) return null
  const gid = newId()
  updateElements(doc, slideId, new Map(ids.map((i) => [i, { groupId: gid }])))
  return gid
}

export function ungroupElements(doc: Y.Doc, slideId: string, ids: readonly string[]): void {
  const s = readSlide(doc, slideId)
  if (!s) return
  updateElements(doc, slideId, new Map(expandGroups(s.elements, ids).map((i) => [i, { groupId: '' }])))
}

const boxOf = (e: SlideElement): Box => ({ x: e.x, y: e.y, w: e.w, h: e.h, rotation: e.rotation })

export function alignElements(doc: Y.Doc, slideId: string, ids: readonly string[], mode: AlignMode): void {
  const s = readSlide(doc, slideId)
  if (!s) return
  const items = s.elements.filter((e) => ids.includes(e.id) && !e.locked).map((e) => ({ id: e.id, box: boxOf(e) }))
  updateElements(doc, slideId, alignBoxes(items, mode, slideSize(doc)))
}

export function distributeElements(doc: Y.Doc, slideId: string, ids: readonly string[], axis: 'x' | 'y'): void {
  const s = readSlide(doc, slideId)
  if (!s) return
  const items = s.elements.filter((e) => ids.includes(e.id) && !e.locked).map((e) => ({ id: e.id, box: boxOf(e) }))
  updateElements(doc, slideId, distributeBoxes(items, axis))
}

/** Texte brut d'un fragment (paragraphes séparés par des retours à la ligne). */
export function fragmentText(f: Y.XmlFragment): string {
  return f.toString()
    .replace(/<\/(paragraph|heading|listItem|blockquote|codeBlock)>/g, '\n')
    .replace(/<[^>]*>/g, '')
    .replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&amp;/g, '&')
    .replace(/\n{2,}/g, '\n')
    .trim()
}

/** Plan textuel de la présentation (recherche, graphe, export de compte) : une rubrique par diapositive. */
export function outline(doc: Y.Doc): string {
  return listSlides(doc).map((s, i) => {
    const parts: string[] = []
    for (const e of s.elements) {
      if (e.type === 'text') {
        const f = textOf(doc, s.id, e.id)
        const t = f ? fragmentText(f) : ''
        if (t) parts.push(t)
      }
      if (e.alt) parts.push(e.alt)
    }
    if (s.notes) parts.push(s.notes)
    return `## ${i + 1}\n${parts.join('\n')}`
  }).join('\n\n')
}

/** Annulation propre à la personne : ne suit que ses transactions (locales et frappe dans les zones de texte). */
export function createUndo(doc: Y.Doc, extraOrigins: unknown[] = []): Y.UndoManager {
  return new Y.UndoManager([slidesOf(doc), metaOf(doc)], { trackedOrigins: new Set<unknown>([LOCAL, ...extraOrigins]), captureTimeout: 400 })
}
