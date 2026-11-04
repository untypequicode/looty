import { computed, type Ref } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from './api'

/** Élément qui peut porter des mots-clés (GLOSSARY). */
export type KeywordKind = 'page' | 'task' | 'board' | 'project' | 'file' | 'event'
export interface KeywordUse { keyword: string; count: number }
export interface ItemKeywords { keywords: string[]; can_edit: boolean }

export const MAX_KEYWORDS = 20
export const KEYWORD_MAX_LEN = 50
export const keywordsKey = (ws: string) => ['ws', ws, 'keywords'] as const
const itemKey = (kind: KeywordKind, id: string) => ['keywords', kind, id] as const

/** Mots-clés d'un élément (lecture, droit de modifier, enregistrement) et ceux de l'espace (autocomplétion). */
export function useKeywords(ws: Ref<string>, kind: Ref<KeywordKind>, id: Ref<string | null | undefined>) {
  const qc = useQueryClient()
  const item = useQuery({
    queryKey: computed(() => itemKey(kind.value, id.value ?? '')),
    queryFn: () => api<ItemKeywords>(`/api/keywords/${kind.value}/${id.value}`, { silent: true }),
    enabled: computed(() => !!id.value),
    staleTime: 30_000,
  })
  const known = useQuery({
    queryKey: computed(() => keywordsKey(ws.value)),
    queryFn: () => api<KeywordUse[]>(`/api/workspaces/${ws.value}/keywords`, { silent: true }),
    enabled: computed(() => !!ws.value),
    staleTime: 60_000,
  })

  async function save(keywords: string[]) {
    if (!id.value) return
    const k = kind.value, i = id.value
    const prev = qc.getQueryData<ItemKeywords>(itemKey(k, i))
    qc.setQueryData<ItemKeywords>(itemKey(k, i), { keywords, can_edit: true })
    try {
      const r = await api<ItemKeywords>(`/api/keywords/${k}/${i}`, { method: 'PUT', body: { keywords } })
      qc.setQueryData(itemKey(k, i), r)
      qc.invalidateQueries({ queryKey: keywordsKey(ws.value) })
    } catch (e) {
      qc.setQueryData(itemKey(k, i), prev)
      throw e
    }
  }

  return {
    keywords: computed(() => item.data.value?.keywords ?? []),
    canEdit: computed(() => !!item.data.value?.can_edit),
    known: computed(() => known.data.value ?? []),
    save,
  }
}
