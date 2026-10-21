import { computed, type Ref } from 'vue'
import { useMemberPrefs } from './usePrefs'
import { bottomOf, GRID_COLS, type GridBox } from './usePanelGrid'

/**
 * Accueil personnalisable (par membre et par espace) : widgets placés librement sur la même
 * grille de 12 colonnes que le tableau de bord Moodle (`AppPanelGrid`). Chaque changement est
 * enregistré tout de suite ; tant que rien n'est enregistré, la disposition par défaut s'affiche.
 */
export type WidgetId =
  | 'greeting' | 'stats' | 'agenda' | 'myTasks' | 'docs' | 'boards' | 'activity'
  | 'overdue' | 'toApprove' | 'favorites' | 'inbox' | 'notepad' | 'online' | 'month' | 'mail' | 'myEvents' | 'classes'
/** Un widget : `i` est son identifiant (un seul exemplaire de chaque). */
export interface WidgetConf extends GridBox { i: WidgetId }
/** `grid` marque la disposition sur grille ; l'ancienne (liste `{ id, size }`) est convertie à la lecture. */
export interface DashboardPrefs { widgets: (WidgetConf | LegacyWidget)[]; saved: boolean; grid?: boolean; /** Disposition choisie en connaissant le widget des cours. */ knowsClasses?: boolean }
interface LegacyWidget { id: WidgetId; size: 'half' | 'full' }

/** Taille d'un widget ajouté (colonnes × lignes). */
export const WIDGETS: { id: WidgetId; w: number; h: number }[] = [
  { id: 'greeting', w: 12, h: 3 },
  { id: 'stats', w: 12, h: 4 },
  { id: 'classes', w: 12, h: 8 },
  { id: 'agenda', w: 6, h: 9 },
  { id: 'myTasks', w: 6, h: 9 },
  { id: 'docs', w: 6, h: 9 },
  { id: 'toApprove', w: 6, h: 9 },
  { id: 'overdue', w: 6, h: 9 },
  { id: 'myEvents', w: 6, h: 9 },
  { id: 'month', w: 6, h: 9 },
  { id: 'inbox', w: 6, h: 9 },
  { id: 'favorites', w: 6, h: 9 },
  { id: 'notepad', w: 6, h: 9 },
  { id: 'mail', w: 6, h: 9 },
  { id: 'online', w: 6, h: 9 },
  { id: 'boards', w: 12, h: 9 },
  { id: 'activity', w: 12, h: 10 },
]
const sizeOf = (id: WidgetId) => WIDGETS.find((w) => w.id === id)!

/** Place des widgets les uns après les autres, de gauche à droite, une ligne à la fois. */
function flow(list: { id: WidgetId; w?: number }[]): WidgetConf[] {
  const out: WidgetConf[] = []
  let x = 0, y = 0, rowH = 0
  for (const it of list) {
    const d = sizeOf(it.id)
    const w = it.w ?? d.w
    if (x + w > GRID_COLS) { y += rowH; x = 0; rowH = 0 }
    out.push({ i: it.id, x, y, w, h: d.h })
    x += w
    rowH = Math.max(rowH, d.h)
  }
  return out
}
export const DEFAULT_LAYOUT: WidgetConf[] = flow(['greeting', 'classes', 'stats', 'agenda', 'myTasks', 'docs', 'toApprove', 'boards', 'activity'].map((id) => ({ id: id as WidgetId })))

export function useDashboard(wsId: Ref<string>) {
  const { prefs, loaded, update } = useMemberPrefs<{ dashboard?: DashboardPrefs | null }>(wsId)
  const saved = computed(() => !!prefs.value?.dashboard?.saved)
  const layout = computed<WidgetConf[]>(() => {
    const d = prefs.value?.dashboard
    if (!d?.widgets?.length) return DEFAULT_LAYOUT
    const known = (id: string) => WIDGETS.some((x) => x.id === id)
    if (d.grid) return (d.widgets as WidgetConf[]).filter((x) => known(x.i))
    // Ancienne liste (ordre + demi/pleine largeur) : convertie en grille, dans le même ordre.
    const list = (d.widgets as LegacyWidget[]).filter((x) => known(x.id))
    // Disposition antérieure au widget des cours : il s'ajoute après la salutation (il ne
    // s'affiche que s'il y a un cours relié aujourd'hui) jusqu'à la prochaine personnalisation.
    if (!d.knowsClasses && !list.some((x) => x.id === 'classes')) {
      list.splice(list.findIndex((x) => x.id === 'greeting') + 1, 0, { id: 'classes', size: 'full' })
    }
    return flow(list.map((x) => ({ id: x.id, w: x.size === 'full' ? GRID_COLS : 6 })))
  })
  const save = (widgets: WidgetConf[]) => update({ dashboard: { widgets, saved: true, grid: true, knowsClasses: true } })
  /** Nouvelles positions après un déplacement ou un redimensionnement. */
  function move(boxes: GridBox[]) {
    const at = new Map(boxes.map((b) => [b.i, b]))
    return save(layout.value.map((w) => ({ ...w, ...at.get(w.i) })))
  }
  const has = (id: WidgetId) => layout.value.some((w) => w.i === id)
  const remove = (id: WidgetId) => save(layout.value.filter((w) => w.i !== id))
  function add(id: WidgetId) {
    if (has(id)) return Promise.resolve()
    const d = sizeOf(id)
    return save([...layout.value, { i: id, x: 0, y: bottomOf(layout.value), w: d.w, h: d.h }])
  }
  /** Retour à la disposition par défaut (la clé est supprimée). */
  const reset = () => update({ dashboard: null })
  return { layout, saved, loaded, move, add, remove, has, reset }
}
