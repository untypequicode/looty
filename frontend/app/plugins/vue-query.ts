import { VueQueryPlugin, QueryClient } from '@tanstack/vue-query'

export default defineNuxtPlugin((nuxtApp) => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        // Les mises à jour arrivent par WebSocket : pas besoin de refetch agressif.
        staleTime: 60_000,
        refetchOnWindowFocus: false,
        retry: (count, err: any) => {
          const s = err?.status ?? err?.statusCode
          if (s === 401 || s === 403 || s === 404) return false
          return count < 2
        },
      },
    },
  })
  nuxtApp.vueApp.use(VueQueryPlugin, { queryClient })
  return { provide: { queryClient } }
})
