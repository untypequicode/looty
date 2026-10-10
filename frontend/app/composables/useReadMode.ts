import { computed, type Ref } from 'vue'
import { toast } from './useToast'
import { translate } from './useI18n'
import { useAccess } from './useAccess'

/**
 * Mode lecture de tout l'espace (barre latérale, Ctrl+Maj+E, palette) : met tous les
 * domaines en lecture, ou tous en édition. C'est le même état que le bouton « Modifier »
 * de l'en-tête des pages, domaine par domaine.
 */
export function useReadingMode(wsId: Ref<string>) {
  const { reading, setReading, editableAreas } = useAccess(wsId)
  function toggleReading() {
    if (!editableAreas.value.length) return false
    const on = !reading.value
    setReading(on)
    toast(translate(on ? 'readMode.on' : 'readMode.off'))
    return true
  }
  return { reading, toggleReading, canToggle: computed(() => editableAreas.value.length > 0) }
}
