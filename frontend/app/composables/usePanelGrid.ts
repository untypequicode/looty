/** Position d'un panneau sur une grille de 12 colonnes (`AppPanelGrid`). */
export interface GridBox { i: string; x: number; y: number; w: number; h: number }

export const GRID_COLS = 12

/** Ligne sous tous les panneaux : un nouveau panneau s'y ajoute (la grille le remonte s'il y a de la place). */
export const bottomOf = (boxes: GridBox[]) => boxes.reduce((m, b) => Math.max(m, b.y + b.h), 0)
