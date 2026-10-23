import { computed, ref, type Ref } from 'vue'
import { useLocalStorage } from '@vueuse/core'
import { isDesktop } from './useDesktop'

export interface Tab {
  id: string
  /** Chemin complet de la page (avec ?task=…). */
  path: string
}
interface TabState {
  tabs: Tab[]
  active: string
  /** Écran partagé : onglet affiché dans le volet de droite (à côté de l'onglet actif). */
  split: string | null
  /** Part de la largeur du volet principal (0,25 à 0,75). */
  ratio: number
  /** Volet qui a la main : les changements d'onglet s'y appliquent. */
  focus: 'main' | 'side'
}

/** Onglet en cours de glissement (pour le déposer sur le côté de la page). */
export const draggingTab = ref<string | null>(null)

const MAX_TABS = 20
const newId = () => Math.random().toString(36).slice(2, 10)

/**
 * Touche des raccourcis d'onglets : Ctrl dans l'application de bureau (aucun navigateur
 * ne se les réserve), Alt sur le site (Ctrl+T / Ctrl+W appartiennent au navigateur).
 */
export const tabMod: 'mod' | 'alt' = isDesktop ? 'mod' : 'alt'
export const tabKeyLabel = (key: string) => [tabMod, key]

/** Afficher la barre d'onglets (réglage de l'appareil). */
export const tabsEnabled = useLocalStorage('looty.tabs.enabled', true)

const states = new Map<string, Ref<TabState>>()
/**
 * Page que le dernier changement d'onglet veut afficher (une navigation « dans » l'onglet actif n'en a pas).
 * Si l'on change d'onglet vite, une navigation plus ancienne peut arriver après : on repart alors
 * vers la dernière destination. `tries` évite de boucler sur une redirection ; l'entrée expire vite.
 */
let target: { path: string; at: number; tries: number } | null = null
const TARGET_TTL = 1500
let settleTimer: ReturnType<typeof setTimeout> | null = null
const liveTarget = () => (target && Date.now() - target.at < TARGET_TTL ? target : null)
/** Onglets fermés récemment, par espace (pour les rouvrir : Ctrl+Maj+T). */
const closedStacks = new Map<string, string[]>()
function remember(ws: string, paths: string[]) {
  const stack = closedStacks.get(ws) ?? []
  stack.push(...paths)
  closedStacks.set(ws, stack.slice(-20))
}

/**
 * Onglets de l'espace : chaque onglet garde sa propre page ; naviguer dans l'onglet
 * actif le fait changer de page, comme un navigateur. Conservés par espace.
 */
export function useTabs(ws: Ref<string>) {
  const route = useRoute()
  const router = useRouter()
  const state = computed(() => {
    const key = ws.value
    if (!states.has(key)) {
      states.set(key, useLocalStorage<TabState>(`looty.tabs.${key}`, { tabs: [], active: '', split: null, ratio: 0.5, focus: 'main' }, { mergeDefaults: true }))
    }
    return states.get(key)!.value
  })
  const tabs = computed(() => state.value.tabs)
  const activeId = computed(() => state.value.active)
  const active = computed(() => state.value.tabs.find((t) => t.id === state.value.active) ?? null)
  /** Onglet du volet de droite (jamais l'onglet actif). */
  const splitTab = computed(() => {
    const s = state.value
    return s.split && s.split !== s.active ? s.tabs.find((t) => t.id === s.split) ?? null : null
  })
  const ratio = computed(() => Math.min(0.75, Math.max(0.25, state.value.ratio || 0.5)))
  const focusSide = computed(() => !!splitTab.value && state.value.focus === 'side')

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
    const tg = liveTarget()
    target = null
    if (tg) {
      if (path === tg.path) return
      // Navigation d'un onglet précédent arrivée en retard : on rejoint la page de l'onglet actif.
      if (tg.tries < 2) {
        target = { ...tg, tries: tg.tries + 1 }
        void navigateTo(tg.path)
        return
      }
    }
    const t = state.value.tabs.find((x) => x.id === state.value.active)
    if (t) t.path = path
  }

  /**
   * Filet de sécurité : peu après la dernière bascule d'onglet, la route et l'adresse du navigateur
   * doivent être celles de l'onglet actif ; sinon (navigations qui se sont croisées) on les réaligne.
   */
  function settleSoon() {
    if (!import.meta.client) return
    if (settleTimer) clearTimeout(settleTimer)
    settleTimer = setTimeout(() => {
      settleTimer = null
      const t = state.value.tabs.find((x) => x.id === state.value.active)
      if (!t) return
      const want = router.resolve(t.path).fullPath
      if (router.currentRoute.value.fullPath !== want || router.resolve(location.pathname + location.search + location.hash).fullPath !== want) {
        target = null
        void router.replace({ ...router.resolve(t.path), force: true })
      }
    }, 250)
  }

  function go(path: string) {
    settleSoon()
    const inflight = !!liveTarget()
    if (route.fullPath === path && !inflight) return
    target = { path, at: Date.now(), tries: 0 }
    // Déjà sur cette page mais une autre navigation est en vol : on la force pour l'annuler.
    if (route.fullPath === path) {
      return router.push({ ...router.resolve(path), force: true }).finally(() => { if (target?.path === path && route.fullPath === path) target = null })
    }
    return navigateTo(path)
  }
  /** Affiche la page de l'onglet. */
  const show = (path: string) => go(path)

  function activate(id: string) {
    const s = state.value
    const t = s.tabs.find((x) => x.id === id)
    if (!t) return
    if (splitTab.value) {
      // Écran partagé : le changement d'onglet s'applique au volet qui a la main ; l'onglet
      // affiché dans l'autre volet passe de l'autre côté.
      const side = s.focus === 'side'
      if (id === (side ? s.split : s.active)) return
      if (id === (side ? s.active : s.split)) return swapSplit()
      if (side) { s.split = id; return }
    }
    s.active = id
    show(t.path)
  }

  /** Échange les deux volets. */
  function swapSplit() {
    const s = state.value
    const side = splitTab.value
    if (!side) return
    const mainId = s.active
    s.active = side.id
    s.split = mainId
    go(side.path)
  }

  /** Affiche un onglet à côté de l'onglet actif. */
  function openSide(id: string) {
    const s = state.value
    let target = s.tabs.find((t) => t.id === id)
    if (!target) return
    if (id === s.active) {
      // Même page des deux côtés : un nouvel onglet pour le volet de droite.
      const copy = { id: newId(), path: target.path }
      s.tabs.splice(s.tabs.indexOf(target) + 1, 0, copy)
      target = copy
    }
    s.split = target.id
    s.focus = 'side'
  }
  /** Ouvre une page (lien) dans le volet de droite. */
  function openSidePath(path: string) {
    const s = state.value
    const same = s.tabs.find((t) => t.path === path && t.id !== s.active)
    if (same) return openSide(same.id)
    const t = { id: newId(), path }
    const at = s.tabs.findIndex((x) => x.id === s.active)
    s.tabs.splice(at < 0 ? s.tabs.length : at + 1, 0, t)
    openSide(t.id)
  }
  function closeSplit() {
    state.value.split = null
    state.value.focus = 'main'
  }
  const setFocus = (f: 'main' | 'side') => { if (state.value.split) state.value.focus = f }
  function setRatio(r: number) { state.value.ratio = Math.min(0.75, Math.max(0.25, r)) }
  /** Le volet de droite a changé de page : son onglet suit. */
  function sideNavigated(path: string) {
    const t = splitTab.value
    if (t && t.path !== path) t.path = path
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
    remember(ws.value, [s.tabs[i]!.path])
    s.tabs.splice(i, 1)
    if (s.split === id) closeSplit()
    if (!s.tabs.length) {
      const t = { id: newId(), path: fallback }
      s.tabs.push(t)
      s.active = t.id
      go(fallback)
      return
    }
    if (s.active === id) {
      // Le voisin n'est pas celui du volet de droite, sauf s'il n'y en a pas d'autre.
      const around = [s.tabs[Math.min(i, s.tabs.length - 1)]!, ...s.tabs]
      const next = around.find((t) => t.id !== s.split) ?? around[0]!
      if (next.id === s.split) closeSplit()
      s.active = next.id
      go(next.path)
    }
  }

  function closeOthers(id: string) {
    const s = state.value
    remember(ws.value, s.tabs.filter((t) => t.id !== id).map((t) => t.path))
    s.tabs = s.tabs.filter((t) => t.id === id)
    closeSplit()
    s.active = id
    const t = s.tabs[0]
    if (t) show(t.path)
  }

  function closeRight(id: string) {
    const s = state.value
    const i = s.tabs.findIndex((t) => t.id === id)
    if (i < 0) return
    const keep = s.tabs.slice(0, i + 1)
    remember(ws.value, s.tabs.slice(i + 1).map((t) => t.path))
    const activeKept = keep.some((t) => t.id === s.active)
    s.tabs = keep
    if (s.split && !keep.some((t) => t.id === s.split)) closeSplit()
    if (!activeKept) { s.active = id; const t = keep.find((x) => x.id === id); if (t) show(t.path) }
  }

  /** Rouvre le dernier onglet fermé. */
  function reopen() {
    const path = closedStacks.get(ws.value)?.pop()
    if (path) open(path, { duplicate: true })
    return !!path
  }
  const canReopen = () => !!closedStacks.get(ws.value)?.length

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

  return {
    tabs, activeId, active, onNavigate, activate, open, close, closeOthers, closeRight, reopen, canReopen, move, cycle, nth,
    splitTab, ratio, focusSide, openSide, openSidePath, closeSplit, swapSplit, setFocus, setRatio, sideNavigated,
  }
}

/** Lien interne de l'espace (pour l'ouverture en onglet). */
export function isWorkspaceLink(href: string | null, ws: string): href is string {
  return !!href && href.startsWith(`/w/${ws}`) && !href.startsWith(`/w/${ws}/settings`)
}

