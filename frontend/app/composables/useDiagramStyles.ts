import { computed, onBeforeUnmount, watch, type Ref } from 'vue'
import { useQuery } from '@tanstack/vue-query'
import { api } from './api'
import { wsKeys } from './useWorkspace'
import { setDiagramStyleSettings, type DiagramStyleSettings } from '~/editor/diagramStyles'

export const fetchDiagramStyles = (wsId: string) => api<DiagramStyleSettings>(`/api/workspaces/${wsId}/diagram-styles`, { silent: true })

/** Styles de diagramme de l'espace affiché : appliqués à tous ses diagrammes et blocs graphe. */
export function useWorkspaceDiagramStyles(wsId: Ref<string | undefined | null>) {
  const q = useQuery({
    queryKey: computed(() => wsKeys.diagramStyles(wsId.value ?? '')),
    queryFn: () => fetchDiagramStyles(wsId.value!),
    enabled: computed(() => !!wsId.value),
  })
  watch(() => q.data.value, (s) => setDiagramStyleSettings(s), { immediate: true })
  onBeforeUnmount(() => setDiagramStyleSettings(null))
  return q
}
