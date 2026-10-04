import { useLocalStorage } from '@vueuse/core'
import { toast } from './useToast'
import { translate } from './useI18n'

/**
 * Mode lecture : toute modification est désactivée (documents, tâches, tableaux) pour
 * lire sans risque de fausse manipulation. Réglage propre à l'appareil.
 */
export const readMode = useLocalStorage('looty.readMode', false)

export function toggleReadMode() {
  readMode.value = !readMode.value
  toast(translate(readMode.value ? 'readMode.on' : 'readMode.off'))
}
