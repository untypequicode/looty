import { useQueryClient } from '@tanstack/vue-query'
import { isDesktop, net, type NetState } from '~/composables/useDesktop'
import { toast } from '~/composables/useToast'
import { translate } from '~/composables/useI18n'

/**
 * Application de bureau : suit l'état réseau du proxy local. Au retour en ligne, une
 * fois les modifications hors ligne synchronisées, toutes les données sont rechargées.
 */
export default defineNuxtPlugin((nuxtApp) => {
  if (!isDesktop) return
  // Le client de requêtes est fourni par un autre plugin : récupéré au moment voulu.
  const qc = () => nuxtApp.vueApp.runWithContext(() => useQueryClient())
  // Tâche créée depuis l'ajout rapide (autre fenêtre) : données rechargées.
  window.addEventListener('looty:refresh', () => qc().invalidateQueries())
  let lastPending = 0
  let wasOffline = false
  let delay = 2000
  let timer: ReturnType<typeof setTimeout> | undefined
  let running = false
  async function poll() {
    if (running) return
    timer = undefined
    // Fenêtre masquée : plus d'interrogation, reprise au retour.
    if (document.visibilityState === 'hidden') return
    running = true
    const before = JSON.stringify(net.value)
    try {
      const s: NetState = await (await fetch('/__looty/net', { cache: 'no-store' })).json()
      const synced = (wasOffline || lastPending > 0) && s.online && s.pending === 0 && !s.syncing
      if (synced) {
        qc().invalidateQueries()
        if (lastPending > s.failed.length) toast.success(translate('offline.synced'))
      }
      if (s.failed.length) {
        toast.error(translate('offline.refused', { n: s.failed.length, items: s.failed.map((f) => f.label).filter(Boolean).slice(0, 3).join(', ') }))
        fetch('/__looty/net/dismiss', { method: 'POST' })
      }
      wasOffline = !s.online
      lastPending = s.pending
      net.value = s
    } catch {}
    running = false
    // État inchangé : intervalle doublé jusqu'à 30 s ; un changement ou un travail en cours le ramène à 2 s.
    const busy = !net.value.online || net.value.pending > 0 || net.value.syncing
    delay = busy || JSON.stringify(net.value) !== before ? 2000 : Math.min(delay * 2, 30_000)
    timer = setTimeout(poll, delay)
  }
  function resume() {
    if (document.visibilityState !== 'visible') return
    delay = 2000
    if (timer) clearTimeout(timer)
    poll()
  }
  document.addEventListener('visibilitychange', resume)
  window.addEventListener('focus', resume)
  poll()
})
