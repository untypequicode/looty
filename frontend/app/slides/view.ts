/**
 * Affichage du canevas : zoom, grille magnétique et gestes tactiles (fonctions pures, testées).
 * Le zoom est l'échelle de la diapositive logique (1920 px de large) vers l'écran : 1 = taille réelle.
 */

export const ZOOM_MIN = 0.1
export const ZOOM_MAX = 4
export const ZOOM_STEPS = [0.1, 0.25, 0.33, 0.5, 0.67, 0.75, 1, 1.25, 1.5, 2, 3, 4] as const
export const GRID = 40

export const clampZoom = (z: number) => (Number.isFinite(z) ? Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, z)) : 1)

/** Cran de zoom suivant ou précédent (à partir d'une valeur quelconque, par exemple celle de l'ajustement). */
export function stepZoom(current: number, dir: 1 | -1): number {
  const z = clampZoom(current)
  if (dir > 0) return ZOOM_STEPS.find((s) => s > z + 0.005) ?? ZOOM_MAX
  for (let i = ZOOM_STEPS.length - 1; i >= 0; i--) if (ZOOM_STEPS[i]! < z - 0.005) return ZOOM_STEPS[i]!
  return ZOOM_MIN
}

/** Zoom après un cran de molette (Ctrl + molette) : proportionnel, borné. */
export const wheelZoom = (current: number, deltaY: number) => clampZoom(current * Math.exp(-Math.max(-300, Math.min(300, deltaY)) * 0.0022))

export interface Pt { x: number; y: number }
export const distance = (a: Pt, b: Pt) => Math.hypot(a.x - b.x, a.y - b.y)
export const midpoint = (a: Pt, b: Pt): Pt => ({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 })

/** Zoom d'un pincement : proportionnel à l'écart des deux doigts depuis le début du geste. */
export const pinchZoom = (startZoom: number, startDistance: number, dist: number) => (startDistance > 0 ? clampZoom(startZoom * (dist / startDistance)) : clampZoom(startZoom))

/** Valeur alignée sur la grille. */
export const snapValue = (v: number, grid = GRID) => Math.round(v / grid) * grid

/** Déplacement corrigé pour que le coin haut gauche d'une boîte tombe sur la grille. */
export function snapMove(box: { x: number; y: number }, dx: number, dy: number, grid = GRID): { dx: number; dy: number } {
  return { dx: snapValue(box.x + dx, grid) - box.x, dy: snapValue(box.y + dy, grid) - box.y }
}

/** Boîte redimensionnée dont les bords tombent sur la grille (taille minimale : une case). */
export function snapRect(r: { x: number; y: number; w: number; h: number }, grid = GRID): { x: number; y: number; w: number; h: number } {
  const x = snapValue(r.x, grid)
  const y = snapValue(r.y, grid)
  const right = snapValue(r.x + r.w, grid)
  const bottom = snapValue(r.y + r.h, grid)
  return { x, y, w: Math.max(grid, right - x), h: Math.max(grid, bottom - y) }
}
