/** Page affichée dans l'application Android native (écran web). */
export const isNativeApp = typeof window !== 'undefined' && !!(window as any).__lootyNative

/** Retour : historique de la page, sinon retour à l'écran natif. */
export function nativeBack() {
  const app = (window as any).LootyApp
  if (app) app.postMessage(JSON.stringify({ type: 'back' }))
  else history.back()
}
