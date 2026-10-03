/**
 * Après un redéploiement, un onglet resté ouvert référence encore les fichiers de
 * l'ancienne version (/_nuxt/xxxx.js), qui n'existent plus : le chargement d'un
 * module à la demande (Mermaid, Excalidraw, PDF…) échoue. On recharge alors la page
 * une fois pour récupérer la nouvelle version (au plus une fois toutes les 20 s).
 */
const KEY = 'looty.chunkReloadAt'

function reloadOnce() {
  const last = Number(sessionStorage.getItem(KEY) || 0)
  if (Date.now() - last < 20_000) return
  sessionStorage.setItem(KEY, String(Date.now()))
  window.location.reload()
}

export default defineNuxtPlugin((nuxtApp) => {
  window.addEventListener('vite:preloadError', reloadOnce)
  nuxtApp.hook('app:chunkError', reloadOnce)
})
