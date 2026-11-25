/**
 * Capacités de l'application native (Android) annoncées à la page : `window.__lootyCaps` est posé par la
 * WebView avant le chargement. Sur le web et le bureau il n'existe pas : tout est permis.
 * Android : `getDisplayMedia` (partage d'écran) n'existe pas dans la WebView, le bouton doit être masqué.
 */
export function canShareScreen(): boolean {
  if (typeof window === 'undefined') return false
  const caps = (window as unknown as { __lootyCaps?: { screenShare?: boolean } }).__lootyCaps
  if (caps && caps.screenShare === false) return false
  return typeof navigator !== 'undefined' && !!navigator.mediaDevices?.getDisplayMedia
}
