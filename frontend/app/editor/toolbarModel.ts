/**
 * Barre d'outils fixe de l'éditeur de pages : modèle pur (outils, groupes, préférences de personnalisation,
 * repli en menu « Plus », phases d'animation). Sans Vue ni TipTap pour rester testable ; le composant
 * DocToolbar en fait l'affichage, en barre compacte ou en ruban (même modèle, aucune logique dupliquée).
 */

export type ToolbarGroupId = 'history' | 'block' | 'marks' | 'color' | 'lists' | 'align' | 'insert' | 'blocks' | 'table'

export type ToolId =
  | 'undo' | 'redo' | 'block' | 'bold' | 'italic' | 'underline' | 'strike' | 'code' | 'color' | 'highlight'
  | 'bullet' | 'ordered' | 'todo' | 'outdent' | 'indent' | 'align'
  | 'link' | 'image' | 'file' | 'formula' | 'divider' | 'table' | 'insert' | 'table-actions'

/** Outils de chaque groupe, dans l'ordre d'affichage. */
export const GROUP_TOOLS: Record<ToolbarGroupId, ToolId[]> = {
  history: ['undo', 'redo'],
  block: ['block'],
  marks: ['bold', 'italic', 'underline', 'strike', 'code'],
  color: ['color', 'highlight'],
  lists: ['bullet', 'ordered', 'todo', 'outdent', 'indent'],
  align: ['align'],
  insert: ['link', 'image', 'file', 'formula', 'divider', 'table'],
  blocks: ['insert'],
  table: ['table-actions'],
}
export const ALL_GROUPS = Object.keys(GROUP_TOOLS) as ToolbarGroupId[]
export const ALL_TOOLS = ALL_GROUPS.flatMap((g) => GROUP_TOOLS[g])

/** Largeur d'un outil, en boutons : les menus à flèche et les boutons à libellé sont plus larges. */
const TOOL_SLOTS: Partial<Record<ToolId, number>> = { color: 1.6, align: 1.6, 'table-actions': 1.6 }
/** Plus un groupe a de priorité, plus tôt il est replié dans « Plus » ; négative : jamais replié. Classé par utilité. */
const GROUP_PRIORITY: Record<ToolbarGroupId, number> = { block: -1, blocks: -1, table: 0, marks: 1, color: 2, lists: 3, history: 4, insert: 5, align: 6 }

/** Ordre d'affichage par défaut : les actions principales d'abord (style, mise en forme, couleur, listes). */
export const DEFAULT_ORDER: ToolbarGroupId[] = ['history', 'block', 'marks', 'color', 'lists', 'align', 'insert', 'blocks', 'table']

/** Onglets du ruban : l'onglet Tableau n'existe que dans un tableau. */
export type RibbonTab = 'home' | 'insert' | 'table'
export const TAB_GROUPS: Record<RibbonTab, ToolbarGroupId[]> = {
  home: ['history', 'block', 'marks', 'color', 'lists', 'align'],
  insert: ['insert', 'blocks'],
  table: ['table'],
}
export function ribbonTabs(inTable: boolean): RibbonTab[] {
  return inTable ? ['home', 'insert', 'table'] : ['home', 'insert']
}
/** L'onglet courant, ramené à un onglet qui existe (sorti du tableau, on revient à l'accueil). */
export function ribbonTabOr(tab: RibbonTab, inTable: boolean): RibbonTab {
  return ribbonTabs(inTable).includes(tab) ? tab : 'home'
}

// --- Personnalisation (préférence `editorToolbarCfg` du compte) -----------------------------------

export interface ToolbarConfig {
  /** Barre compacte Looty ou ruban à onglets ; sur téléphone, toujours la barre compacte. */
  style: 'bar' | 'ribbon'
  /** 'auto' : en haut, et en bas sur téléphone (au-dessus du clavier). */
  position: 'auto' | 'top' | 'bottom'
  size: 'compact' | 'comfortable'
  /** Libellés à côté des icônes des menus (style de bloc, Insérer) et sous les boutons du ruban. */
  labels: boolean
  /** Ordre des groupes (les groupes masqués y restent). */
  order: ToolbarGroupId[]
  hidden: ToolId[]
  /** null : suit `prefers-reduced-motion` (animée sauf demande contraire du système). */
  animate: boolean | null
  /** Pendant la frappe, la barre se réduit puis revient au mouvement de la souris. */
  reduce: boolean
  /** Pendant la frappe, la barre devient translucide (opaque au survol et au focus). */
  translucent: boolean
  opacity: number
  autoHide: boolean
  /** Ruban replié : seuls les onglets restent affichés. */
  collapsed: boolean
}

export const DEFAULT_CONFIG: ToolbarConfig = {
  style: 'bar', position: 'auto', size: 'compact', labels: true, order: [...DEFAULT_ORDER], hidden: [],
  animate: null, reduce: false, translucent: false, opacity: 0.55, autoHide: false, collapsed: false,
}
export const MIN_OPACITY = 0.25
export const MAX_OPACITY = 0.95

const oneOf = <T extends string>(v: unknown, list: readonly T[], d: T): T => (typeof v === 'string' && (list as readonly string[]).includes(v) ? (v as T) : d)
const bool = (v: unknown, d: boolean) => (typeof v === 'boolean' ? v : d)

/**
 * Lit la préférence enregistrée, quelle qu'elle soit : valeurs inconnues ou périmées ignorées, groupes
 * apparus depuis l'enregistrement placés en fin de liste, doublons retirés. Ne lève jamais.
 */
export function cleanConfig(raw: unknown): ToolbarConfig {
  const r = raw && typeof raw === 'object' && !Array.isArray(raw) ? (raw as Record<string, unknown>) : {}
  const order: ToolbarGroupId[] = []
  if (Array.isArray(r.order)) for (const g of r.order) if (ALL_GROUPS.includes(g as ToolbarGroupId) && !order.includes(g as ToolbarGroupId)) order.push(g as ToolbarGroupId)
  for (const g of DEFAULT_ORDER) if (!order.includes(g)) order.push(g)
  const hidden: ToolId[] = []
  if (Array.isArray(r.hidden)) for (const t of r.hidden) if (ALL_TOOLS.includes(t as ToolId) && !hidden.includes(t as ToolId)) hidden.push(t as ToolId)
  const op = typeof r.opacity === 'number' && Number.isFinite(r.opacity) ? r.opacity : DEFAULT_CONFIG.opacity
  return {
    style: oneOf(r.style, ['bar', 'ribbon'], 'bar'),
    position: oneOf(r.position, ['auto', 'top', 'bottom'], 'auto'),
    size: oneOf(r.size, ['compact', 'comfortable'], 'compact'),
    labels: bool(r.labels, true),
    order, hidden,
    animate: typeof r.animate === 'boolean' ? r.animate : null,
    reduce: bool(r.reduce, false),
    translucent: bool(r.translucent, false),
    opacity: Math.round(Math.min(MAX_OPACITY, Math.max(MIN_OPACITY, op)) * 100) / 100,
    autoHide: bool(r.autoHide, false),
    collapsed: bool(r.collapsed, false),
  }
}

/** Ne garde que ce qui diffère des valeurs par défaut ; null s'il n'y a rien (la clé est alors supprimée du compte). */
export function serializeConfig(cfg: ToolbarConfig): Record<string, unknown> | null {
  const out: Record<string, unknown> = {}
  const d = DEFAULT_CONFIG
  for (const k of ['style', 'position', 'size', 'labels', 'animate', 'reduce', 'translucent', 'opacity', 'autoHide', 'collapsed'] as const) if (cfg[k] !== d[k]) out[k] = cfg[k]
  if (cfg.order.join() !== d.order.join()) out.order = cfg.order
  if (cfg.hidden.length) out.hidden = cfg.hidden
  return Object.keys(out).length ? out : null
}

/** Déplace un groupe d'un cran ; renvoie un nouveau tableau (inchangé aux extrémités). */
export function moveGroup(order: ToolbarGroupId[], id: ToolbarGroupId, dir: -1 | 1): ToolbarGroupId[] {
  const i = order.indexOf(id)
  const j = i + dir
  if (i < 0 || j < 0 || j >= order.length) return order
  const next = [...order]
  ;[next[i], next[j]] = [next[j]!, next[i]!]
  return next
}

export function setToolHidden(hidden: ToolId[], tool: ToolId, hide: boolean): ToolId[] {
  const rest = hidden.filter((t) => t !== tool)
  return hide ? [...rest, tool] : rest
}
export function setGroupHidden(hidden: ToolId[], group: ToolbarGroupId, hide: boolean): ToolId[] {
  const rest = hidden.filter((t) => !GROUP_TOOLS[group].includes(t))
  return hide ? [...rest, ...GROUP_TOOLS[group]] : rest
}
export function groupHidden(hidden: ToolId[], group: ToolbarGroupId): boolean {
  return GROUP_TOOLS[group].every((t) => hidden.includes(t))
}

export const effectiveStyle = (cfg: ToolbarConfig, isMobile: boolean): 'bar' | 'ribbon' => (isMobile ? 'bar' : cfg.style)
export const effectivePosition = (cfg: ToolbarConfig, isMobile: boolean): 'top' | 'bottom' => (cfg.position === 'auto' ? (isMobile ? 'bottom' : 'top') : cfg.position)
export const animateOn = (cfg: ToolbarConfig, reducedMotion: boolean): boolean => (cfg.animate === null ? !reducedMotion : cfg.animate)

export interface BarPhase { shrunk: boolean; faded: boolean; hidden: boolean }
/**
 * Apparence pendant la frappe : rien ne change tant que la personne survole la barre ou y a le focus ;
 * sinon, selon les options, la barre se réduit, devient translucide ou se masque.
 */
export function barPhase(typing: boolean, active: boolean, cfg: Pick<ToolbarConfig, 'reduce' | 'translucent' | 'autoHide'>): BarPhase {
  const on = typing && !active
  return { shrunk: on && cfg.reduce, faded: on && cfg.translucent, hidden: on && cfg.autoHide }
}

/** Couleurs récentes : la plus récente en tête, sans doublon, bornées. */
export function pushRecent(list: string[], key: string, max = 5): string[] {
  return [key, ...list.filter((k) => k !== key)].slice(0, max)
}

// --- Groupes et repli adaptatif ---------------------------------------------------------------------

export interface ToolbarGroup {
  id: ToolbarGroupId
  /** Largeur du groupe, en boutons. */
  slots: number
  /** Les groupes de plus grand `priority` sont repliés en premier dans « Plus ». */
  priority: number
}

/** Outils affichés d'un groupe : ni masqués, et le groupe Tableau seulement dans un tableau. */
export function groupTools(g: ToolbarGroupId, cfg: Pick<ToolbarConfig, 'hidden'>, inTable = false): ToolId[] {
  if (g === 'table' && !inTable) return []
  return GROUP_TOOLS[g].filter((t) => !cfg.hidden.includes(t))
}

/** Groupes affichables dans l'ordre choisi, avec leur largeur ; un groupe sans outil disparaît. */
export function toolbarGroups(cfg: Pick<ToolbarConfig, 'hidden' | 'order' | 'labels'>, inTable = false, only?: ToolbarGroupId[], compact = false): ToolbarGroup[] {
  // Menus à libellé (style de bloc, Insérer) : sans libellé (réglage ou téléphone), icône et flèche seulement.
  const wide = compact ? 1.1 : !cfg.labels ? 1.5 : 3
  const out: ToolbarGroup[] = []
  for (const id of cfg.order) {
    if (only && !only.includes(id)) continue
    const tools = groupTools(id, cfg, inTable)
    if (!tools.length) continue
    const slots = tools.reduce((n, t) => n + (t === 'block' || t === 'insert' ? wide : (TOOL_SLOTS[t] ?? 1)), 0)
    out.push({ id, slots, priority: GROUP_PRIORITY[id] })
  }
  return out
}

/** Largeur d'un bouton selon la taille choisie et le type d'écran (zone tactile : 40 px, bureau : 32 px). */
export function buttonWidth(size: ToolbarConfig['size'], touch: boolean): number {
  return touch ? 40 : size === 'comfortable' ? 38 : 32
}
export const SEP_WIDTH = 9
/** Séparateur plus étroit sur téléphone, où chaque pixel compte. */
export const SEP_WIDTH_TOUCH = 5
export const BTN_WIDTH = 32
/** Largeur réservée au bouton « Plus ». */
export const MORE_WIDTH = 40

export function groupWidth(g: ToolbarGroup, btn = BTN_WIDTH, sep = SEP_WIDTH): number {
  return Math.ceil(g.slots * btn) + sep
}

/**
 * Répartit les groupes entre barre et menu « Plus » selon la largeur disponible : les groupes de
 * priorité la plus haute partent d'abord. L'ordre d'affichage est conservé dans la barre ; le menu
 * range les groupes repliés du plus utile au moins utile.
 */
export function splitGroups(available: number, groups: ToolbarGroup[], btn = BTN_WIDTH, reserved = 0, sep = SEP_WIDTH): { visible: ToolbarGroupId[]; overflow: ToolbarGroupId[] } {
  const hidden = new Set<ToolbarGroupId>()
  const total = () => groups.filter((g) => !hidden.has(g.id)).reduce((n, g) => n + groupWidth(g, btn, sep), 0) + (hidden.size ? MORE_WIDTH : 0) + reserved
  const byPriority = [...groups].filter((g) => g.priority >= 0).sort((a, b) => b.priority - a.priority)
  for (const g of byPriority) {
    if (total() <= available) break
    hidden.add(g.id)
  }
  return {
    visible: groups.filter((g) => !hidden.has(g.id)).map((g) => g.id),
    overflow: groups.filter((g) => hidden.has(g.id)).sort((a, b) => a.priority - b.priority).map((g) => g.id),
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
  highlightColor: string | null
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
  bulletList: false, orderedList: false, taskList: false, block: 'paragraph', align: 'left', color: null, highlightColor: null,
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
    highlightColor: s.getAttributes('highlight').color ?? null,
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

/** Clés de traduction des outils et des groupes (panneau de personnalisation, menu « Plus »). */
export const TOOL_LABEL: Record<ToolId, string> = {
  undo: 'toolbar.undo', redo: 'toolbar.redo', block: 'toolbar.blockStyle', bold: 'editor.bold', italic: 'editor.italic',
  underline: 'editor.underline', strike: 'editor.strike', code: 'editor.code', color: 'toolbar.colorTool', highlight: 'editor.highlight',
  bullet: 'editor.slash.bullet', ordered: 'editor.slash.ordered', todo: 'editor.slash.todo', outdent: 'toolbar.outdent', indent: 'toolbar.indent',
  align: 'toolbar.align', link: 'toolbar.link', image: 'toolbar.image', file: 'toolbar.file', formula: 'toolbar.formula',
  divider: 'toolbar.divider', table: 'toolbar.table', insert: 'toolbar.insert', 'table-actions': 'toolbar.tableActions',
}
export const GROUP_LABEL: Record<ToolbarGroupId, string> = {
  history: 'toolbar.groups.history', block: 'toolbar.groups.block', marks: 'toolbar.groups.marks', color: 'toolbar.groups.color',
  lists: 'toolbar.groups.lists', align: 'toolbar.groups.align', insert: 'toolbar.groups.insert', blocks: 'toolbar.groups.blocks', table: 'toolbar.groups.table',
}
