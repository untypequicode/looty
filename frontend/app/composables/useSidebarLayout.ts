import { computed, ref, type Component, type Ref } from 'vue'
import { useMemberPrefs } from './usePrefs'

/**
 * Barre latérale personnalisable, propre à chaque espace (préférences du membre) :
 * ordre et visibilité des liens principaux et des sections (favoris, tableaux, documents).
 */
export interface NavDef {
  icon: Component
  label: () => string
  to: () => string
  exact?: boolean
  badge?: () => number | undefined
  /** Masqué par le rôle (ex. tableaux non visibles). */
  when?: () => boolean
  /** Absent tant que la personne ne l'ajoute pas. */
  hiddenByDefault?: boolean
}
export interface SidebarPrefs { order?: string[]; hidden?: string[]; shown?: string[]; sectionOrder?: string[]; sectionHidden?: string[] }
interface Prefs { sidebar?: SidebarPrefs; [k: string]: unknown }

export const SECTION_KEYS = ['favorites', 'boards', 'docs'] as const
const customizeOpen = ref(false)
/** Définitions enregistrées par la barre latérale (lues par la fenêtre de personnalisation). */
const registry = ref<Record<string, NavDef>>({})

function ordered(keys: string[], order: string[] | undefined) {
  const pos = new Map((order ?? []).map((k, i) => [k, i]))
  return [...keys].sort((a, b) => (pos.get(a) ?? 1000 + keys.indexOf(a)) - (pos.get(b) ?? 1000 + keys.indexOf(b)))
}

export function useSidebarLayout(wsId: Ref<string>, defs?: Record<string, NavDef>, sectionWhen?: Record<string, () => boolean>) {
  if (defs) registry.value = defs
  const { prefs, update } = useMemberPrefs<Prefs>(wsId)
  const sb = computed<SidebarPrefs>(() => prefs.value?.sidebar ?? {})
  const isHidden = (k: string) => {
    const d = registry.value[k]
    if (sb.value.hidden?.includes(k)) return true
    if (d?.hiddenByDefault && !sb.value.shown?.includes(k)) return true
    return false
  }
  const items = computed(() => ordered(Object.keys(registry.value), sb.value.order)
    .filter((k) => !isHidden(k) && (registry.value[k]!.when?.() ?? true))
    .map((k) => { const d = registry.value[k]!; return { key: k, icon: d.icon, label: d.label(), to: d.to(), exact: d.exact, badge: d.badge?.() } }))
  /** Ordre d'affichage de chaque section visible (undefined = masquée). */
  const sections = computed<Record<string, number | undefined>>(() => {
    const out: Record<string, number | undefined> = {}
    ordered([...SECTION_KEYS], sb.value.sectionOrder).forEach((k, i) => {
      out[k] = sb.value.sectionHidden?.includes(k) || !(sectionWhen?.[k]?.() ?? true) ? undefined : i
    })
    return out
  })
  async function save(next: SidebarPrefs) { await update({ sidebar: next }) }
  async function reset() { await update({ sidebar: null }) }
  return { items, sections, customizeOpen, registry, sb, isHidden, ordered, save, reset }
}
