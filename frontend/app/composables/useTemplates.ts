import { computed, type Ref } from 'vue'
import { useQuery } from '@tanstack/vue-query'
import { api } from './api'
import { promptDialog } from './useConfirm'
import { translate } from './useI18n'
import { toast } from './useToast'

/** Modèle de tableau ou de document (GLOSSARY : Modèle) : fourni, ou enregistré par un administrateur de l'espace. */
export interface Template { id: string; kind: 'board' | 'page'; name: string; builtin: boolean; columns?: string[] }

export const templatesKey = (ws: string) => ['ws', ws, 'templates'] as const

/** Modèles que la personne peut utiliser dans l'espace (selon ce qu'elle a le droit de créer). */
export function useTemplates(wsId: Ref<string>, enabled: Ref<boolean> | (() => boolean) = () => true) {
  return useQuery({
    queryKey: computed(() => templatesKey(wsId.value)),
    queryFn: () => api<Template[]>(`/api/workspaces/${wsId.value}/templates`, { silent: true }),
    enabled: computed(() => !!wsId.value && (typeof enabled === 'function' ? enabled() : enabled.value)),
    staleTime: 30_000,
  })
}

/** Enregistre un tableau ou un document comme modèle de l'espace (administrateurs). */
export async function saveAsTemplate(wsId: string, kind: 'board' | 'page', sourceId: string, defaultName: string): Promise<boolean> {
  const name = await promptDialog({ title: translate('tpl.saveAs'), message: translate(kind === 'board' ? 'tpl.saveBoardHint' : 'tpl.savePageHint'), initial: defaultName, placeholder: translate('tpl.saveName'), confirmLabel: translate('common.save') })
  if (!name?.trim()) return false
  await api(`/api/workspaces/${wsId}/templates`, { method: 'POST', body: { kind, source_id: sourceId, name: name.trim() } })
  toast.success(translate('tpl.saved'))
  return true
}
