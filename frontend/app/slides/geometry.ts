/**
 * Géométrie pure des diapositives (aucune dépendance à Vue ni à Yjs) : boîtes tournées,
 * redimensionnement par poignées, alignement, répartition, guides magnétiques, sélection au lasso.
 * Unités logiques du canevas (1920 x 1080 par défaut). Voir l'ADR 0023.
 */

export interface Box { x: number; y: number; w: number; h: number; rotation?: number }
export interface Rect { x: number; y: number; w: number; h: number }
export type Handle = 'nw' | 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w'
export const HANDLES: readonly Handle[] = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w']
export type AlignMode = 'left' | 'center' | 'right' | 'top' | 'middle' | 'bottom'

export const MIN_SIZE = 8

const rad = (deg: number) => (deg * Math.PI) / 180
const round = (n: number) => Math.round(n * 1000) / 1000

export function centerOf(b: Box): { x: number; y: number } {
  return { x: b.x + b.w / 2, y: b.y + b.h / 2 }
}

/** Les quatre coins d'une boîte une fois tournée autour de son centre. */
export function corners(b: Box): { x: number; y: number }[] {
  const c = centerOf(b)
  const a = rad(b.rotation ?? 0)
  const cos = Math.cos(a)
  const sin = Math.sin(a)
  return [[-b.w / 2, -b.h / 2], [b.w / 2, -b.h / 2], [b.w / 2, b.h / 2], [-b.w / 2, b.h / 2]].map(([dx, dy]) => ({
    x: round(c.x + dx! * cos - dy! * sin),
    y: round(c.y + dx! * sin + dy! * cos),
  }))
}

/** Rectangle englobant (aligné sur les axes) d'une boîte tournée. */
export function rotatedBounds(b: Box): Rect {
  if (!b.rotation) return { x: b.x, y: b.y, w: b.w, h: b.h }
  const pts = corners(b)
  const xs = pts.map((p) => p.x)
  const ys = pts.map((p) => p.y)
  const x = Math.min(...xs)
  const y = Math.min(...ys)
  return { x, y, w: Math.max(...xs) - x, h: Math.max(...ys) - y }
}

/** Rectangle englobant de plusieurs boîtes (null si aucune). */
export function boundsOf(boxes: readonly Box[]): Rect | null {
  if (!boxes.length) return null
  const rs = boxes.map(rotatedBounds)
  const x = Math.min(...rs.map((r) => r.x))
  const y = Math.min(...rs.map((r) => r.y))
  return { x, y, w: Math.max(...rs.map((r) => r.x + r.w)) - x, h: Math.max(...rs.map((r) => r.y + r.h)) - y }
}

export function rectsIntersect(a: Rect, b: Rect): boolean {
  return a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

/** Lasso : identifiants des boîtes dont le rectangle englobant touche le rectangle donné (dans tous les sens de tracé). */
export function marquee(rect: Rect, items: readonly { id: string; box: Box }[]): string[] {
  const norm: Rect = { x: Math.min(rect.x, rect.x + rect.w), y: Math.min(rect.y, rect.y + rect.h), w: Math.abs(rect.w), h: Math.abs(rect.h) }
  return items.filter((i) => rectsIntersect(norm, rotatedBounds(i.box))).map((i) => i.id)
}

export function rotatePoint(p: { x: number; y: number }, c: { x: number; y: number }, deg: number): { x: number; y: number } {
  const a = rad(deg)
  const dx = p.x - c.x
  const dy = p.y - c.y
  return { x: c.x + dx * Math.cos(a) - dy * Math.sin(a), y: c.y + dx * Math.sin(a) + dy * Math.cos(a) }
}

/** Angle normalisé dans [0, 360), arrondi au pas si demandé (touche Maj). */
export function normalizeAngle(deg: number, step = 0): number {
  let a = ((deg % 360) + 360) % 360
  if (step > 0) a = (Math.round(a / step) * step) % 360
  return round(a)
}

/** Angle (degrés, 0 = vers le haut) du pointeur autour du centre : poignée de rotation. */
export function angleFromCenter(center: { x: number; y: number }, pointer: { x: number; y: number }): number {
  return (Math.atan2(pointer.y - center.y, pointer.x - center.x) * 180) / Math.PI + 90
}

const ANCHOR: Record<Handle, { sx: number; sy: number }> = {
  nw: { sx: -1, sy: -1 }, n: { sx: 0, sy: -1 }, ne: { sx: 1, sy: -1 }, e: { sx: 1, sy: 0 },
  se: { sx: 1, sy: 1 }, s: { sx: 0, sy: 1 }, sw: { sx: -1, sy: 1 }, w: { sx: -1, sy: 0 },
}

export interface ResizeOptions { keepRatio?: boolean; fromCenter?: boolean; min?: number }

/**
 * Redimensionne une boîte en tirant une poignée de (dx, dy) en coordonnées du canevas. Le déplacement
 * est ramené dans le repère de la boîte tournée ; le côté ou coin opposé reste fixe (ou le centre).
 */
export function resizeBox(box: Box, handle: Handle, dx: number, dy: number, opts: ResizeOptions = {}): Box {
  const min = opts.min ?? MIN_SIZE
  const rot = box.rotation ?? 0
  const a = rad(-rot)
  const ldx = dx * Math.cos(a) - dy * Math.sin(a)
  const ldy = dx * Math.sin(a) + dy * Math.cos(a)
  const { sx, sy } = ANCHOR[handle]
  const k = opts.fromCenter ? 2 : 1
  let w = sx ? box.w + sx * ldx * k : box.w
  let h = sy ? box.h + sy * ldy * k : box.h
  if (opts.keepRatio && box.w > 0 && box.h > 0) {
    const ratio = box.w / box.h
    if (sx && sy) {
      if (Math.abs(w / box.w - 1) >= Math.abs(h / box.h - 1)) h = w / ratio
      else w = h * ratio
    } else if (sx) h = w / ratio
    else w = h * ratio
  }
  w = Math.max(min, w)
  h = Math.max(min, h)
  const c0 = centerOf(box)
  let cx: number
  let cy: number
  if (opts.fromCenter) {
    cx = c0.x
    cy = c0.y
  } else {
    // Le point fixe (côté opposé) ne bouge pas dans le repère du canevas.
    const fixedWorld = rotatePoint({ x: c0.x - sx * (box.w / 2), y: c0.y - sy * (box.h / 2) }, c0, rot)
    const off = rotatePoint({ x: -sx * (w / 2), y: -sy * (h / 2) }, { x: 0, y: 0 }, rot)
    cx = fixedWorld.x - off.x
    cy = fixedWorld.y - off.y
  }
  return { x: round(cx - w / 2), y: round(cy - h / 2), w: round(w), h: round(h), rotation: box.rotation }
}

/** Positions (x, y) à donner à chaque boîte pour l'aligner : sur leur rectangle commun, ou sur la diapositive si une seule. */
export function align(items: readonly { id: string; box: Box }[], mode: AlignMode, slide?: { w: number; h: number }): Map<string, { x: number; y: number }> {
  const out = new Map<string, { x: number; y: number }>()
  if (!items.length) return out
  const target: Rect = items.length === 1 && slide ? { x: 0, y: 0, w: slide.w, h: slide.h } : boundsOf(items.map((i) => i.box))!
  for (const { id, box } of items) {
    const r = rotatedBounds(box)
    const ox = r.x - box.x
    const oy = r.y - box.y
    let nx = r.x
    let ny = r.y
    if (mode === 'left') nx = target.x
    else if (mode === 'center') nx = target.x + (target.w - r.w) / 2
    else if (mode === 'right') nx = target.x + target.w - r.w
    else if (mode === 'top') ny = target.y
    else if (mode === 'middle') ny = target.y + (target.h - r.h) / 2
    else ny = target.y + target.h - r.h
    out.set(id, { x: round(nx - ox), y: round(ny - oy) })
  }
  return out
}

/** Répartit au moins trois boîtes avec des écarts égaux, les extrémités restant en place. */
export function distribute(items: readonly { id: string; box: Box }[], axis: 'x' | 'y'): Map<string, { x: number; y: number }> {
  const out = new Map<string, { x: number; y: number }>()
  if (items.length < 3) return out
  const rects = items.map((i) => ({ i, r: rotatedBounds(i.box) }))
  const start = axis
  const size = axis === 'x' ? 'w' : 'h'
  rects.sort((a, b) => a.r[start] - b.r[start])
  const first = rects[0]!.r
  const last = rects[rects.length - 1]!.r
  const total = last[start] + last[size] - first[start]
  const sum = rects.reduce((s, e) => s + e.r[size], 0)
  const gap = (total - sum) / (rects.length - 1)
  let cursor = first[start]
  for (const { i, r } of rects) {
    const delta = cursor - r[start]
    out.set(i.id, { x: round(i.box.x + (axis === 'x' ? delta : 0)), y: round(i.box.y + (axis === 'y' ? delta : 0)) })
    cursor += r[size] + gap
  }
  return out
}

export interface Guide { axis: 'x' | 'y'; pos: number; from: number; to: number }
export interface SnapResult { dx: number; dy: number; guides: Guide[] }

function lines(r: Rect, axis: 'x' | 'y'): number[] {
  return axis === 'x' ? [r.x, r.x + r.w / 2, r.x + r.w] : [r.y, r.y + r.h / 2, r.y + r.h]
}

/**
 * Aimantation d'un rectangle en déplacement sur les bords et centres de la diapositive et des
 * autres éléments. Renvoie la correction à appliquer et les guides à tracer (unités du canevas).
 */
export function snap(moving: Rect, others: readonly Rect[], slide: { w: number; h: number }, threshold = 8): SnapResult {
  const targets: Rect[] = [{ x: 0, y: 0, w: slide.w, h: slide.h }, ...others]
  const result: SnapResult = { dx: 0, dy: 0, guides: [] }
  for (const axis of ['x', 'y'] as const) {
    let best: { delta: number; pos: number } | null = null
    const mine = lines(moving, axis)
    for (const t of targets) {
      for (const tl of lines(t, axis)) {
        for (const ml of mine) {
          const d = tl - ml
          if (Math.abs(d) <= threshold && (!best || Math.abs(d) < Math.abs(best.delta))) best = { delta: d, pos: tl }
        }
      }
    }
    if (!best) continue
    if (axis === 'x') result.dx = best.delta
    else result.dy = best.delta
    const snapped: Rect = axis === 'x' ? { ...moving, x: moving.x + best.delta } : { ...moving, y: moving.y + best.delta }
    const span: number[] = []
    for (const r of [snapped, ...targets]) {
      if (lines(r, axis).some((l) => Math.abs(l - best!.pos) < 0.5)) {
        span.push(axis === 'x' ? r.y : r.x, axis === 'x' ? r.y + r.h : r.x + r.w)
      }
    }
    result.guides.push({ axis, pos: round(best.pos), from: Math.min(...span), to: Math.max(...span) })
  }
  return result
}

/** Zoom qui fait tenir une diapositive dans un conteneur, avec une marge. */
export function fitScale(container: { w: number; h: number }, slide: { w: number; h: number }, margin = 0): number {
  const s = Math.min((container.w - margin * 2) / slide.w, (container.h - margin * 2) / slide.h)
  return s > 0 ? s : 0.01
}

/** Garde une boîte en partie visible : au moins `keep` unités sur la diapositive. */
export function clampToSlide(b: Box, slide: { w: number; h: number }, keep = 40): Box {
  return { ...b, x: Math.min(Math.max(b.x, keep - b.w), slide.w - keep), y: Math.min(Math.max(b.y, keep - b.h), slide.h - keep) }
}
