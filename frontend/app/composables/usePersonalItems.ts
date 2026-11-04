import { useQueryClient } from '@tanstack/vue-query'
import { api, errorMessage } from './api'
import { confirmDialog } from './useConfirm'
import { toast } from './useToast'
import { translate as t } from './useI18n'
import { useAuth } from './useAuth'

/** Élément qui peut être personnel (GLOSSARY) dans un espace partagé. */
export type PersonalKind = 'page' | 'board' | 'task' | 'project' | 'file'

/**
 * Éléments personnels : visibles de leur seul propriétaire, même dans un espace partagé.
 * Le rendre personnel le retire aux autres (confirmation) ; le repartager le publie.
 */
export function usePersonal() {
  const qc = useQueryClient()
  const { user } = useAuth()
  const isMine = (owner?: string | null) => !!owner && owner === user.value?.id

  async function setPersonal(kind: PersonalKind, id: string, personal: boolean, name?: string) {
    if (personal && !(await confirmDialog({
      title: t('personalItem.makePersonalTitle', { name: name || t('docs.untitled') }),
      message: t(`personalItem.makePersonalHint.${kind}`),
      confirmLabel: t('personalItem.makePersonal'),
    }))) return false
    try {
      await api(`/api/personal/${kind}/${id}`, { method: 'PUT', body: { personal } })
    } catch (e) {
      toast.error(errorMessage(e))
      return false
    }
    toast.success(personal ? t('personalItem.nowPersonal') : t('personalItem.nowShared'))
    // Listes et éléments ouverts rechargés avec les nouveaux droits.
    qc.invalidateQueries({ queryKey: ['ws'] })
    qc.invalidateQueries({ queryKey: ['board'] })
    qc.invalidateQueries({ queryKey: ['page', id] })
    qc.invalidateQueries({ queryKey: ['project', id] })
    return true
  }

  return { setPersonal, isMine }
}
