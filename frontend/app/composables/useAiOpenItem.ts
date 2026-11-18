import { computed, ref, watch } from 'vue'
import type { AiAttachment } from './useAi'
import { locationOfRoute, useLocationInfo } from './usePresenceInfo'
import { useWorkspace } from './useWorkspace'

/** Dernier élément ouvert hors Assistant : il est joint par défaut à une nouvelle conversation (retirable). */
export const aiOpenItem = ref<AiAttachment | null>(null)

/** À appeler une fois dans la mise en page : suit la page, l'entraînement, le tableau ou la tâche ouverts. */
export function useAiOpenItemTracker() {
  const route = useRoute()
  const { wsId } = useWorkspace()
  const location = computed(() => locationOfRoute({ path: route.path, params: route.params as Record<string, any>, query: route.query as Record<string, any> }))
  const info = useLocationInfo(location)
  watch([location, () => info.value?.label, wsId], () => {
    const [kind, id] = location.value.split(':')
    if (!id) return
    const map: Record<string, AiAttachment['kind']> = { page: 'page', study: 'study', board: 'board', task: 'task' }
    const k = map[kind!]
    if (!k) return
    aiOpenItem.value = { kind: k, id, title: info.value?.label ?? '', ws: wsId.value }
  }, { immediate: true })
}
