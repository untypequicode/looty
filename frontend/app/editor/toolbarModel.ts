/**
 * Barre d'outils fixe de l'éditeur de pages : modèle pur (groupes, état de la sélection, repli en
 * menu « Plus », préférence). Sans Vue ni TipTap pour rester testable ; le composant DocToolbar
 * en fait l'affichage.
 */

export type ToolbarGroupId = 'history' | 'block' | 'marks' | 'color' | 'lists' | 'align' | 'insert' | 'table' | 'more'

export interface ToolbarGroup {
  id: ToolbarGroupId
  /** Nombre de boutons du groupe (sert à estimer sa largeur). */
  slots: number
  /** Les groupes de plus grand `priority` sont repliés en premier dans « Plus ». */
  priority: number
}

/** Ordre d'affichage. Les groupes de priorité négative (style de bloc, menu Insérer) ne sont jamais repliés. */
export const TOOLBAR_GROUPS: ToolbarGroup[] = [
  { id: 'history', slots: 2, priority: 2 },
  { id: 'block', slots: 3, priority: -1 },
  { id: 'marks', slots: 5, priority: 1 },
  { id: 'color', slots: 2, priority: 6 },
  { id: 'lists', slots: 5, priority: 3 },
  { id: 'align', slots: 1, priority: 7 },
  { id: 'insert', slots: 5, priority: 4 },
  { id: 'table', slots: 2, priority: 5 },
  { id: 'more', slots: 1, priority: -1 },
]

export const BTN_WIDTH = 29
export const SEP_WIDTH = 9
/** Largeur réservée au bouton « Plus ». */
export const MORE_WIDTH = 38

export function groupWidth(g: ToolbarGroup, btn = BTN_WIDTH): number {
  return g.slots * btn + SEP_WIDTH
}

/**
 * Répartit les groupes entre barre et menu « Plus » selon la largeur disponible : les groupes de
 * priorité la plus haute partent d'abord. L'ordre d'affichage est conservé des deux côtés.
 */
export function splitGroups(available: number, groups: ToolbarGroup[] = TOOLBAR_GROUPS, btn = BTN_WIDTH): { visible: ToolbarGroupId[]; overflow: ToolbarGroupId[] } {
  const hidden = new Set<ToolbarGroupId>()
  const total = () => groups.filter((g) => !hidden.has(g.id)).reduce((n, g) => n + groupWidth(g, btn), 0) + (hidden.size ? MORE_WIDTH : 0)
  const byPriority = [...groups].filter((g) => g.priority >= 0).sort((a, b) => b.priority - a.priority)
  for (const g of byPriority) {
    if (total() <= available) break
    hidden.add(g.id)
  }
  return {
    visible: groups.filter((g) => !hidden.has(g.id)).map((g) => g.id),
    overflow: groups.filter((g) => hidden.has(g.id)).map((g) => g.id),
  }
}

/** Blocs déjà couverts par un bouton de la barre : le menu « Insérer » ne les répète pas. */
export const TOOLBAR_COVERED = ['p', 'h1', 'h2', 'h3', 'ul', 'ol', 'todo', 'quote', 'code', 'table', 'hr', 'image', 'file', 'math'] as const

/** Éléments du menu « Insérer » : la liste du menu « / » moins ce que la barre porte déjà. */
export function insertItems<T extends { id: string }>(all: T[]): T[] {
  const covered = new Set<string>(TOOLBAR_COVERED)
  return all.filter((i) => !covered.has(i.id))
}

export type BlockType = 'paragraph' | 'h1' | 'h2' | 'h3' | 'h4' | 'quote' | 'code'

/** Ce que le modèle lit d'un éditeur (sous-ensemble de l'API TipTap). */
export interface ToolbarSource {
  isActive(name: string, attrs?: Record<string, unknown>): boolean
  getAttributes(name: string): Record<string, any>
  can: {
    undo(): boolean
    redo(): boolean
    indent(): boolean
    outdent(): boolean
    mergeCells(): boolean
    splitCell(): boolean
  }
}

export interface ToolbarState {
  bold: boolean
  italic: boolean
  underline: boolean
  strike: boolean
  code: boolean
  highlight: boolean
  link: boolean
  bulletList: boolean
  orderedList: boolean
  taskList: boolean
  block: BlockType
  align: 'left' | 'center' | 'right' | 'justify'
  color: string | null
  inTable: boolean
  canUndo: boolean
  canRedo: boolean
  canIndent: boolean
  canOutdent: boolean
  canMerge: boolean
  canSplit: boolean
}

export const EMPTY_STATE: ToolbarState = {
  bold: false, italic: false, underline: false, strike: false, code: false, highlight: false, link: false,
  bulletList: false, orderedList: false, taskList: false, block: 'paragraph', align: 'left', color: null,
  inTable: false, canUndo: false, canRedo: false, canIndent: false, canOutdent: false, canMerge: false, canSplit: false,
}

function blockOf(s: ToolbarSource): BlockType {
  if (s.isActive('codeBlock')) return 'code'
  if (s.isActive('blockquote')) return 'quote'
  for (const n of [1, 2, 3, 4] as const) if (s.isActive('heading', { level: n })) return `h${n}` as BlockType
  return 'paragraph'
}

function alignOf(s: ToolbarSource): ToolbarState['align'] {
  const a = s.isActive('heading') ? s.getAttributes('heading').textAlign : s.getAttributes('paragraph').textAlign
  return a === 'center' || a === 'right' || a === 'justify' ? a : 'left'
}

/** Calcule l'état affiché. Les actions de tableau ne sont interrogées que dans un tableau. */
export function toolbarState(s: ToolbarSource): ToolbarState {
  const inTable = s.isActive('table')
  return {
    bold: s.isActive('bold'),
    italic: s.isActive('italic'),
    underline: s.isActive('underline'),
    strike: s.isActive('strike'),
    code: s.isActive('code'),
    highlight: s.isActive('highlight'),
    link: s.isActive('link'),
    bulletList: s.isActive('bulletList'),
    orderedList: s.isActive('orderedList'),
    taskList: s.isActive('taskList'),
    block: blockOf(s),
    align: alignOf(s),
    color: s.getAttributes('textColor').color ?? null,
    inTable,
    canUndo: s.can.undo(),
    canRedo: s.can.redo(),
    canIndent: s.can.indent(),
    canOutdent: s.can.outdent(),
    canMerge: inTable && s.can.mergeCells(),
    canSplit: inTable && s.can.splitCell(),
  }
}

/** Même contenu : évite de réafficher la barre à chaque frappe qui ne change rien. */
export function sameState(a: ToolbarState, b: ToolbarState): boolean {
  return (Object.keys(a) as (keyof ToolbarState)[]).every((k) => a[k] === b[k])
}

/** Préférence « afficher la barre » : affichée tant que la personne ne l'a pas masquée. */
export function toolbarVisible(pref: unknown): boolean {
  return pref !== false
}

/** Index du bouton voisin pour la navigation au clavier (flèches, Début, Fin), en boucle. */
export function rovingNext(current: number, count: number, key: string): number {
  if (count <= 0) return -1
  if (key === 'Home') return 0
  if (key === 'End') return count - 1
  if (key === 'ArrowRight' || key === 'ArrowDown') return (current + 1) % count
  if (key === 'ArrowLeft' || key === 'ArrowUp') return (current - 1 + count) % count
  return current
}

/** Décalage du clavier virtuel (px) à partir de visualViewport : la barre mobile reste au-dessus. */
export function keyboardInset(innerHeight: number, vv: { height: number; offsetTop: number } | null | undefined): number {
  if (!vv) return 0
  return Math.max(0, Math.round(innerHeight - vv.height - vv.offsetTop))
}
