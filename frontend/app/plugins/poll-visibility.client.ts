import type { QueryClient } from '@tanstack/vue-query'
import { stalePolled } from '~/composables/pollRefresh'

/** Au retour sur l'onglet, recharge une fois les données périodiques devenues périmées. */
export default defineNuxtPlugin((nuxtApp) => {
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState !== 'visible') return
    // Le client est fourni par plugins/vue-query.ts, chargé après celui-ci : on le lit à l'usage.
    const qc = nuxtApp.$queryClient as QueryClient | undefined
    if (!qc) return
    for (const q of stalePolled(qc.getQueryCache().getAll() as any[], Date.now())) q.fetch().catch(() => {})
  })
})
