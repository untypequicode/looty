import { computed, type Ref } from 'vue'
import { useLocalStorage } from '@vueuse/core'

export interface Tab {
  id: string
  /** Chemin complet de la page (avec ?task=…). */
  path: string
}
interface TabState {
  tabs: Tab[]
  active: string
}

const MAX_TABS = 20
const newId = () => Math.random().toString(36).slice(2, 10)

/** Afficher la barre d'onglets (réglage de l'appareil). */
export const tabsEnabled = useLocalStorage('looty.tabs.enabled', true)

const states = new Map<string, Ref<TabState>>()
/** Navigation lancée par un onglet (pas une navigation « dans » l'onglet actif). */
let pending: string | null = null

/**
 * Onglets de l'espace : chaque onglet garde sa propre page ; naviguer dans l'onglet
 * actif le fait changer de page, comme un navigateur. Conservés par espace.
 */
export function useTabs(ws: Ref<string>) {
  const route = useRoute()
  const state = computed(() => {
    const key = ws.value
    if (!states.has(key)) {
      states.set(key, useLocalStorage<TabState>(`looty.tabs.${key}`, { tabs: [], active: '' }, { mergeDefaults: true }))
    }
    return states.get(key)!.value
  })
  const tabs = computed(() => state.value.tabs)
  const activeId = computed(() => state.value.active)
  const active = computed(() => state.value.tabs.find((t) => t.id === state.value.active) ?? null)

  function ensureOne(path: string) {
    const s = state.value
    if (!s.tabs.length) {
      const t = { id: newId(), path }
      s.tabs.push(t)
      s.active = t.id
    } else if (!s.tabs.some((t) => t.id === s.active)) {
      s.active = s.tabs[0]!.id
    }
  }

  /** Route changée : soit un changement d'onglet, soit l'onglet actif qui change de page. */
  function onNavigate(path: string) {
    ensureOne(path)
    if (pending === path) {
      pending = null
      return
    }
    pending = null
    const t = state.value.tabs.find((x) => x.id === state.value.active)
    if (t) t.path = path
  }

  function go(path: string) {
    pending = path
    return navigateTo(path)
  }

  function activate(id: string) {
    const t = state.value.tabs.find((x) => x.id === id)
    if (!t) return
    state.value.active = id
    if (route.fullPath !== t.path) go(t.path)
  }

  /** Ouvre une page dans un nouvel onglet (ou active celui qui l'affiche déjà). */
  function open(path: string, opts: { background?: boolean; duplicate?: boolean } = {}) {
    const s = state.value
    const existing = opts.duplicate ? null : s.tabs.find((t) => t.path === path)
    if (existing) {
      if (!opts.background) activate(existing.id)
      return
    }
    if (s.tabs.length >= MAX_TABS) {
      // Le plus ancien onglet inactif laisse sa place.
      const drop = s.tabs.findIndex((t) => t.id !== s.active)
      if (drop >= 0) s.tabs.splice(drop, 1)
    }
    const t = { id: newId(), path }
    const at = s.tabs.findIndex((x) => x.id === s.active)
    s.tabs.splice(at < 0 ? s.tabs.length : at + 1, 0, t)
    if (!opts.background) {
      s.active = t.id
      go(path)
    }
  }

  function close(id: string, fallback: string) {
    const s = state.value
    const i = s.tabs.findIndex((t) => t.id === id)
    if (i < 0) return
    s.tabs.splice(i, 1)
    if (!s.tabs.length) {
      const t = { id: newId(), path: fallback }
      s.tabs.push(t)
      s.active = t.id
      go(fallback)
      return
    }
    if (s.active === id) {
      const next = s.tabs[Math.min(i, s.tabs.length - 1)]!
      s.active = next.id
      go(next.path)
    }
  }

  function closeOthers(id: string) {
    const s = state.value
    s.tabs = s.tabs.filter((t) => t.id === id)
    activate(id)
  }

  function closeRight(id: string) {
    const s = state.value
    const i = s.tabs.findIndex((t) => t.id === id)
    if (i < 0) return
    const keep = s.tabs.slice(0, i + 1)
    const activeKept = keep.some((t) => t.id === s.active)
    s.tabs = keep
    if (!activeKept) activate(id)
  }

  function move(id: string, to: number) {
    const s = state.value
    const i = s.tabs.findIndex((t) => t.id === id)
    if (i < 0 || to < 0 || to >= s.tabs.length || i === to) return
    const [t] = s.tabs.splice(i, 1)
    s.tabs.splice(to, 0, t!)
  }

  function cycle(dir: 1 | -1) {
    const s = state.value
    if (s.tabs.length < 2) return
    const i = s.tabs.findIndex((t) => t.id === s.active)
    activate(s.tabs[(i + dir + s.tabs.length) % s.tabs.length]!.id)
  }

  function nth(n: number) {
    const s = state.value
    const t = n >= 9 ? s.tabs[s.tabs.length - 1] : s.tabs[n - 1]
    if (t) activate(t.id)
  }

  return { tabs, activeId, active, onNavigate, activate, open, close, closeOthers, closeRight, move, cycle, nth }
}

/** Lien interne de l'espace (pour l'ouverture en onglet). */
export function isWorkspaceLink(href: string | null, ws: string): href is string {
  return !!href && href.startsWith(`/w/${ws}`) && !href.startsWith(`/w/${ws}/settings`)
}

