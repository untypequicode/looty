import * as Y from 'yjs'
import { LOCAL, metaOf, slideIds, slidesOf } from './model.ts'

/**
 * Mode présentation (ADR 0023, issue #303) : transitions, animations d'éléments et machine d'états de
 * navigation. Tout est additif dans le document Yjs (pas de nouvelle version de format) :
 *   slide.transition  'none' | 'fade' | 'slide' | 'zoom' | '' (vide : celle de la présentation)
 *   meta.transition   transition par défaut de la présentation
 *   slide.anims       Y.Array d'objets { id, el, kind, trigger } dans l'ordre de lecture
 * La position dans la présentation est (diapositive, étape) : une étape par animation « au clic ».
 */

export const TRANSITIONS = ['none', 'fade', 'slide', 'zoom'] as const
export type Transition = (typeof TRANSITIONS)[number]
export const ANIM_KINDS = ['appear', 'fade', 'move'] as const
export type AnimKind = (typeof ANIM_KINDS)[number]
export const ANIM_TRIGGERS = ['click', 'after'] as const
export type AnimTrigger = (typeof ANIM_TRIGGERS)[number]
/** Durée d'une animation d'entrée (ms) ; « apparition » est instantanée. */
export const ANIM_MS = 500
export const MAX_ANIMS = 100

export interface Anim { id: string; el: string; kind: AnimKind; trigger: AnimTrigger }
/** Animation placée dans le temps : étape d'entrée (0 : dès l'arrivée) et délai dans l'étape. */
export interface Scheduled extends Anim { step: number; delay: number; ms: number }
export interface Pos { slide: number; step: number }

const isTransition = (v: unknown): v is Transition => (TRANSITIONS as readonly unknown[]).includes(v)
const isKind = (v: unknown): v is AnimKind => (ANIM_KINDS as readonly unknown[]).includes(v)
const isTrigger = (v: unknown): v is AnimTrigger => (ANIM_TRIGGERS as readonly unknown[]).includes(v)

// --- Transitions -------------------------------------------------------------------------------

/** Transition propre à la diapositive ('' : celle de la présentation). */
export function slideTransition(doc: Y.Doc, slideId: string): Transition | '' {
  const v = slidesOf(doc).get(slideId)?.get('transition')
  return isTransition(v) ? v : ''
}
export function deckTransition(doc: Y.Doc): Transition {
  const v = metaOf(doc).get('transition')
  return isTransition(v) ? v : 'none'
}
/** Transition effective pour entrer sur la diapositive. */
export function effectiveTransition(doc: Y.Doc, slideId: string): Transition {
  return slideTransition(doc, slideId) || deckTransition(doc)
}
export function setSlideTransition(doc: Y.Doc, slideId: string, v: Transition | ''): void {
  const s = slidesOf(doc).get(slideId)
  if (s) doc.transact(() => s.set('transition', v), LOCAL)
}
export function setDeckTransition(doc: Y.Doc, v: Transition): void {
  doc.transact(() => metaOf(doc).set('transition', v), LOCAL)
}

// --- Notes de l'orateur --------------------------------------------------------------------------

export function getNotes(doc: Y.Doc, slideId: string): string {
  const n = slidesOf(doc).get(slideId)?.get('notes')
  return n instanceof Y.Text ? n.toString() : typeof n === 'string' ? n : ''
}

/** Remplace les notes en ne réécrivant que la partie qui change (fusion fine entre personnes). */
export function setNotes(doc: Y.Doc, slideId: string, text: string): void {
  const s = slidesOf(doc).get(slideId)
  if (!s) return
  let n = s.get('notes')
  doc.transact(() => {
    if (!(n instanceof Y.Text)) { n = new Y.Text(); s.set('notes', n) }
    const t = n as Y.Text
    const old = t.toString()
    if (old === text) return
    let a = 0
    while (a < old.length && a < text.length && old[a] === text[a]) a++
    let b = 0
    while (b < old.length - a && b < text.length - a && old[old.length - 1 - b] === text[text.length - 1 - b]) b++
    if (old.length - a - b > 0) t.delete(a, old.length - a - b)
    const ins = text.slice(a, text.length - b)
    if (ins) t.insert(a, ins)
  }, LOCAL)
}

// --- Animations ----------------------------------------------------------------------------------

export function readAnims(doc: Y.Doc, slideId: string): Anim[] {
  const s = slidesOf(doc).get(slideId)
  const arr = s?.get('anims')
  if (!(arr instanceof Y.Array)) return []
  const els = s!.get('elements') as Y.Map<unknown> | undefined
  const out: Anim[] = []
  for (const a of arr.toArray() as Record<string, unknown>[]) {
    if (!a || typeof a.id !== 'string' || typeof a.el !== 'string' || !els?.has(a.el)) continue
    out.push({ id: a.id, el: a.el, kind: isKind(a.kind) ? a.kind : 'appear', trigger: isTrigger(a.trigger) ? a.trigger : 'click' })
  }
  return out
}

function animArray(doc: Y.Doc, slideId: string): Y.Array<Anim> | null {
  const s = slidesOf(doc).get(slideId)
  if (!s) return null
  let arr = s.get('anims')
  if (!(arr instanceof Y.Array)) { arr = new Y.Array<Anim>(); s.set('anims', arr) }
  return arr as Y.Array<Anim>
}

/** Ajoute une animation d'entrée en fin de liste. Renvoie son identifiant (null : limite atteinte). */
export function addAnim(doc: Y.Doc, slideId: string, el: string, kind: AnimKind = 'fade', trigger: AnimTrigger = 'click'): string | null {
  let id: string | null = null
  doc.transact(() => {
    const arr = animArray(doc, slideId)
    if (!arr || arr.length >= MAX_ANIMS) return
    id = `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`
    arr.push([{ id, el, kind, trigger }])
  }, LOCAL)
  return id
}

function indexOfAnim(arr: Y.Array<Anim>, id: string): number {
  return arr.toArray().findIndex((a) => a.id === id)
}

export function updateAnim(doc: Y.Doc, slideId: string, id: string, patch: Partial<Pick<Anim, 'kind' | 'trigger'>>): void {
  doc.transact(() => {
    const arr = animArray(doc, slideId)
    if (!arr) return
    const i = indexOfAnim(arr, id)
    if (i < 0) return
    const cur = arr.get(i)
    arr.delete(i, 1)
    arr.insert(i, [{ ...cur, ...patch }])
  }, LOCAL)
}

export function removeAnim(doc: Y.Doc, slideId: string, id: string): void {
  doc.transact(() => {
    const arr = animArray(doc, slideId)
    const i = arr ? indexOfAnim(arr, id) : -1
    if (arr && i >= 0) arr.delete(i, 1)
  }, LOCAL)
}

/** Monte (-1) ou descend (+1) une animation dans l'ordre de lecture. */
export function moveAnim(doc: Y.Doc, slideId: string, id: string, dir: -1 | 1): void {
  doc.transact(() => {
    const arr = animArray(doc, slideId)
    if (!arr) return
    const i = indexOfAnim(arr, id)
    const j = i + dir
    if (i < 0 || j < 0 || j >= arr.length) return
    const cur = arr.get(i)
    arr.delete(i, 1)
    arr.insert(j, [cur])
  }, LOCAL)
}

/**
 * Place les animations dans le temps (fonction pure). Chaque animation « au clic » ouvre une étape ;
 * une animation « après la précédente » rejoint l'étape en cours et démarre quand la précédente se
 * termine (avant tout clic : elle joue dès l'arrivée sur la diapositive, étape 0).
 */
export function schedule(anims: readonly Anim[]): Scheduled[] {
  const out: Scheduled[] = []
  let step = 0
  let end = 0
  for (const a of anims) {
    const ms = a.kind === 'appear' ? 0 : ANIM_MS
    if (a.trigger === 'click') { step++; end = 0 }
    const delay = end
    out.push({ ...a, step, delay, ms })
    end = delay + ms
  }
  return out
}

/** Nombre d'étapes « au clic » d'une diapositive. */
export const stepCount = (anims: readonly Anim[]): number => anims.filter((a) => a.trigger === 'click').length

/** Éléments encore cachés à une étape : ceux dont la première entrée est après `step`. */
export function hiddenAt(sched: readonly Scheduled[], step: number): Set<string> {
  const first = new Map<string, number>()
  for (const a of sched) if (!first.has(a.el)) first.set(a.el, a.step)
  const out = new Set<string>()
  for (const [el, s] of first) if (s > step) out.add(el)
  return out
}

/** Entrées qui jouent en arrivant à l'étape `step` (leur première animation d'entrée). */
export function enteringAt(sched: readonly Scheduled[], step: number): Scheduled[] {
  const seen = new Set<string>()
  const out: Scheduled[] = []
  for (const a of sched) {
    if (seen.has(a.el)) continue
    seen.add(a.el)
    if (a.step === step) out.push(a)
  }
  return out
}

// --- Machine d'états de navigation -----------------------------------------------------------------

/** Ramène une position dans les bornes (`steps[i]` : nombre d'étapes de la diapositive i). */
export function clampPos(p: Pos, steps: readonly number[]): Pos {
  if (!steps.length) return { slide: 0, step: 0 }
  const slide = Math.max(0, Math.min(steps.length - 1, Math.trunc(p.slide) || 0))
  return { slide, step: Math.max(0, Math.min(steps[slide]!, Math.trunc(p.step) || 0)) }
}

/** Suivant : l'étape suivante de la diapositive, sinon la diapositive suivante ; la fin reste en place. */
export function nextPos(p: Pos, steps: readonly number[]): Pos {
  const c = clampPos(p, steps)
  if (c.step < (steps[c.slide] ?? 0)) return { slide: c.slide, step: c.step + 1 }
  if (c.slide < steps.length - 1) return { slide: c.slide + 1, step: 0 }
  return c
}

/** Précédent : l'étape précédente, sinon la diapositive précédente entièrement révélée. */
export function prevPos(p: Pos, steps: readonly number[]): Pos {
  const c = clampPos(p, steps)
  if (c.step > 0) return { slide: c.slide, step: c.step - 1 }
  if (c.slide > 0) return { slide: c.slide - 1, step: steps[c.slide - 1] ?? 0 }
  return c
}

/** Saute à une diapositive (numéro à partir de 0), au début de ses étapes. */
export const gotoSlide = (i: number, steps: readonly number[]): Pos => clampPos({ slide: i, step: 0 }, steps)
export const firstPos = (steps: readonly number[]): Pos => clampPos({ slide: 0, step: 0 }, steps)
export const lastPos = (steps: readonly number[]): Pos => clampPos({ slide: steps.length - 1, step: Infinity }, steps)
export const samePos = (a: Pos, b: Pos): boolean => a.slide === b.slide && a.step === b.step

/** Écran noir ou blanc : la touche bascule, la même touche ou l'autre couleur change l'écran. */
export type Screen = '' | 'black' | 'white'
export function toggleScreen(cur: Screen, want: 'black' | 'white'): Screen {
  return cur === want ? '' : want
}

/** Nombre d'étapes de chaque diapositive, dans l'ordre. */
export function stepsOf(doc: Y.Doc): number[] {
  return slideIds(doc).map((id) => stepCount(readAnims(doc, id)))
}

/** Vrai quand le visiteur demande moins d'animations. */
export function prefersReducedMotion(): boolean {
  return typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches
}
