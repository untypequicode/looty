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
  async function poll() {
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
    setTimeout(poll, net.value.online && !net.value.pending ? 4000 : 2000)
  }
  poll()
})
