import { computed } from 'vue'
import { useQuery } from '@tanstack/vue-query'
import { api } from './api'
import { translate } from './useI18n'

/**
 * Démo (GLOSSARY : *Démo*) : l'instance publique remplie de données fictives. Les fonctions qui chargent le
 * serveur ou sortent vers l'extérieur y sont visibles mais désactivées, avec la mention « Indisponible dans la démo ».
 */
export function useDemo() {
  // Même requête que `app.vue` : un seul appel à /api/settings.
  const { data } = useQuery({
    queryKey: ['public-settings'],
    queryFn: () => api<{ app_name: string; demo?: boolean }>('/api/settings', { silent: true }),
    staleTime: 10 * 60_000,
  })
  const isDemo = computed(() => !!data.value?.demo)
  return { isDemo, unavailableLabel: () => translate('demo.unavailable') }
}
