import { computed, type Component, type Ref } from 'vue'
import { Clock, BookOpen, MessageSquare, Bell, BookMarked, BarChart3, Folder } from 'lucide-vue-next'
import { useMemberPrefs } from './usePrefs'
import { bottomOf, type GridBox } from './usePanelGrid'

/**
 * Tableau de bord Moodle (GLOSSARY) : panneaux placés librement sur une grille de 12
 * colonnes, propres à chaque membre dans chaque espace (préférences de membre, clé
 * `moodleBoard`). Tant que rien n'est enregistré, la disposition par défaut s'affiche.
 */
export type PanelKind = 'deadlines' | 'courses' | 'messages' | 'notifications' | 'course' | 'progress' | 'folder'
export interface BoardPanel extends GridBox {
  kind: PanelKind
  /** Titre choisi par le membre (sinon le titre du type de panneau). */
  title?: string
  /** Panneau « Cours épinglé » : le cours affiché. */
  course?: number
  /** Panneau « Dossier » : le dossier de cours affiché. */
  folder?: string
  /** Réglages du panneau (cours : affichage, tri, filtre, groupement ; échéances : cours masqués). */
  opts?: PanelOpts
}
export type CoursesView = 'cards' | 'list' | 'compact'
export type CoursesSort = 'access' | 'name' | 'progress' | 'deadline' | 'manual'
export type CoursesFilter = 'progress' | 'upcoming' | 'past' | 'favourites' | 'hidden' | 'all'
export type CoursesGroup = 'category' | 'folders' | 'none'
export interface PanelOpts {
  view?: CoursesView
  sort?: CoursesSort
  filter?: CoursesFilter
  group?: CoursesGroup
  /** Échéances : montrer aussi celles des cours masqués. */
  includeHidden?: boolean
}
export const DEFAULT_COURSES: Required<Pick<PanelOpts, 'view' | 'sort' | 'filter' | 'group'>> = { view: 'cards', sort: 'access', filter: 'progress', group: 'category' }
export interface MoodleBoardPrefs { panels: BoardPanel[] }

/** Taille d'un panneau ajouté (colonnes × lignes). */
export const PANEL_KINDS: { kind: PanelKind; w: number; h: number }[] = [
  { kind: 'deadlines', w: 5, h: 12 },
  { kind: 'courses', w: 7, h: 12 },
  { kind: 'messages', w: 6, h: 8 },
  { kind: 'notifications', w: 6, h: 8 },
  { kind: 'course', w: 6, h: 14 },
  { kind: 'progress', w: 4, h: 8 },
  { kind: 'folder', w: 6, h: 12 },
]

export const PANEL_ICONS: Record<PanelKind, Component> = {
  deadlines: Clock, courses: BookOpen, messages: MessageSquare, notifications: Bell, course: BookMarked, progress: BarChart3, folder: Folder,
}

export const DEFAULT_BOARD: BoardPanel[] = [
  { i: 'deadlines', kind: 'deadlines', x: 0, y: 0, w: 5, h: 12 },
  { i: 'courses', kind: 'courses', x: 5, y: 0, w: 7, h: 12 },
  { i: 'messages', kind: 'messages', x: 0, y: 12, w: 6, h: 8 },
  { i: 'notifications', kind: 'notifications', x: 6, y: 12, w: 6, h: 8 },
]

const newId = () => Math.random().toString(36).slice(2, 10)

export function useMoodleBoard(wsId: Ref<string>) {
  const { prefs, loaded, update } = useMemberPrefs<{ moodleBoard?: MoodleBoardPrefs }>(wsId)
  const panels = computed<BoardPanel[]>(() => {
    const saved = prefs.value?.moodleBoard?.panels
    return (saved ?? DEFAULT_BOARD).filter((p) => PANEL_KINDS.some((k) => k.kind === p.kind))
  })
  const customized = computed(() => !!prefs.value?.moodleBoard)

  const save = (list: BoardPanel[]) => update({ moodleBoard: { panels: list } })
  /** Nouvelles positions après un déplacement ou un redimensionnement. */
  function move(boxes: GridBox[]) {
    const at = new Map(boxes.map((b) => [b.i, b]))
    return save(panels.value.map((p) => ({ ...p, ...at.get(p.i) })))
  }
  function patch(id: string, change: Partial<BoardPanel>) {
    return save(panels.value.map((p) => (p.i === id ? { ...p, ...change } : p)))
  }
  function add(kind: PanelKind, extra: Partial<BoardPanel> = {}) {
    const k = PANEL_KINDS.find((x) => x.kind === kind)!
    const panel: BoardPanel = { i: newId(), kind, x: 0, y: bottomOf(panels.value), w: k.w, h: k.h, ...extra }
    return save([...panels.value, panel]).then(() => panel)
  }
  const remove = (id: string) => save(panels.value.filter((p) => p.i !== id))
  /** Retour à la disposition par défaut (la clé est supprimée). */
  const reset = () => update({ moodleBoard: null })

  return { panels, customized, loaded, move, patch, add, remove, reset }
}
