/**
 * Volet secondaire de l'écran partagé : une copie de l'application intégrée (cadre) qui
 * affiche un autre onglet à côté de l'onglet actif. Elle se reconnaît à `?pane=1`,
 * n'affiche que la page (ni barre latérale ni onglets), ne signale pas sa position
 * (la présence reste celle de la fenêtre principale) et dialogue avec elle par messages.
 */
export const isPane = typeof window !== 'undefined' && new URLSearchParams(window.location.search).has('pane')

export type PaneMessage =
  | { type: 'ready' }
  /** Le volet a changé de page. */
  | { type: 'nav'; path: string }
  /** L'utilisateur a pris la main dans le volet. */
  | { type: 'focus' }
  /** Ctrl/⌘ + clic sur un lien : ouvrir dans un onglet de la fenêtre principale. */
  | { type: 'open-tab'; href: string }
  /** Raccourci d'onglets tapé dans le volet : à rejouer dans la fenêtre principale. */
  | { type: 'key'; key: string; code: string; alt: boolean; ctrl: boolean; meta: boolean; shift: boolean }

export function postToParent(msg: PaneMessage) {
  if (isPane && window.parent !== window) window.parent.postMessage({ looty: 'pane', ...msg }, window.location.origin)
}

/** Chemin avec le marqueur de volet. */
export function withPane(path: string) {
  const u = new URL(path, window.location.origin)
  u.searchParams.set('pane', '1')
  return u.pathname + u.search + u.hash
}
/** Chemin sans le marqueur de volet (celui qu'enregistre l'onglet). */
export function withoutPane(path: string) {
  const u = new URL(path, window.location.origin)
  u.searchParams.delete('pane')
  return u.pathname + u.search + u.hash
}
