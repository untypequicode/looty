import type { QueryClient } from '@tanstack/vue-query'
import { stalePolled } from '~/composables/pollRefresh'

/** Au retour sur l'onglet, recharge une fois les données périodiques devenues périmées. */
export default defineNuxtPlugin((nuxtApp) => {
  const qc = nuxtApp.$queryClient as QueryClient
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState !== 'visible') return
    for (const q of stalePolled(qc.getQueryCache().getAll() as any[], Date.now())) q.fetch().catch(() => {})
  })
})
