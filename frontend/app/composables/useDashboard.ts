import { computed, type Ref } from 'vue'
import { useMemberPrefs } from './usePrefs'

/**
 * Tableau de bord personnalisable (par espace) : widgets choisis, ordre et largeur.
 * Tant que la disposition n'a jamais été enregistrée, la disposition par défaut s'affiche
 * avec l'invitation à la personnaliser (l'enregistrement est requis, même sans changement).
 */
export type WidgetId =
  | 'greeting' | 'stats' | 'agenda' | 'myTasks' | 'docs' | 'boards' | 'activity'
  | 'overdue' | 'toApprove' | 'favorites' | 'inbox' | 'notepad' | 'online' | 'month' | 'mail' | 'myEvents' | 'classes'
export type WidgetSize = 'half' | 'full'
export interface WidgetConf { id: WidgetId; size: WidgetSize }
export interface DashboardPrefs { widgets: WidgetConf[]; saved: boolean; /** Disposition choisie en connaissant le widget des cours. */ knowsClasses?: boolean }

export const WIDGETS: { id: WidgetId; size: WidgetSize; fixedSize?: boolean }[] = [
  { id: 'greeting', size: 'full', fixedSize: true },
  { id: 'stats', size: 'full', fixedSize: true },
  { id: 'classes', size: 'full', fixedSize: true },
  { id: 'agenda', size: 'half' },
  { id: 'myTasks', size: 'half' },
  { id: 'docs', size: 'half' },
  { id: 'toApprove', size: 'half' },
  { id: 'overdue', size: 'half' },
  { id: 'myEvents', size: 'half' },
  { id: 'month', size: 'half' },
  { id: 'inbox', size: 'half' },
  { id: 'favorites', size: 'half' },
  { id: 'notepad', size: 'half' },
  { id: 'mail', size: 'half' },
  { id: 'online', size: 'half' },
  { id: 'boards', size: 'full' },
  { id: 'activity', size: 'full' },
]
export const DEFAULT_LAYOUT: WidgetConf[] = ['greeting', 'classes', 'stats', 'agenda', 'myTasks', 'docs', 'toApprove', 'boards', 'activity']
  .map((id) => ({ id: id as WidgetId, size: WIDGETS.find((w) => w.id === id)!.size }))

export function useDashboard(wsId: Ref<string>) {
  const { prefs, loaded, update } = useMemberPrefs<{ dashboard?: DashboardPrefs }>(wsId)
  const saved = computed(() => !!prefs.value?.dashboard?.saved)
  const layout = computed<WidgetConf[]>(() => {
    const w = prefs.value?.dashboard?.widgets
    const list = (w?.length ? w : DEFAULT_LAYOUT).filter((x) => WIDGETS.some((d) => d.id === x.id))
    // Disposition antérieure au widget des cours : il s'ajoute après la salutation (il ne
    // s'affiche que s'il y a un cours relié aujourd'hui) jusqu'à la prochaine personnalisation.
    if (w?.length && !prefs.value?.dashboard?.knowsClasses && !list.some((x) => x.id === 'classes')) {
      const at = list.findIndex((x) => x.id === 'greeting') + 1
      list.splice(at, 0, { id: 'classes', size: 'full' })
    }
    return list
  })
  async function save(widgets: WidgetConf[]) { await update({ dashboard: { widgets, saved: true, knowsClasses: true } }) }
  return { layout, saved, loaded, save }
}
