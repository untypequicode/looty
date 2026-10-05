import { computed, watch, type Ref } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import type { Editor } from '@tiptap/vue-3'
import type { Me } from '~/types'
import { api } from './api'
import { ME_KEY, useAuth } from './useAuth'

/**
 * Préférences synchronisées entre appareils.
 * - `useUserPrefs` : réglages globaux (users.prefs, chargés avec /api/auth/me) ;
 * - `useMemberPrefs` : réglages propres à un espace (workspace_members.prefs).
 * Mise à jour fusionnée clé par clé ; `null` supprime une clé.
 */
export interface UserPrefs {
  spellcheck?: boolean
  /** Langue de vérification : '' = langue de l'interface. */
  spellLang?: string
  [key: string]: unknown
}

export function useUserPrefs() {
  const qc = useQueryClient()
  const { me } = useAuth()
  const prefs = computed<UserPrefs>(() => (me.value?.user?.prefs ?? {}) as UserPrefs)
  async function update(patch: Partial<UserPrefs> | Record<string, unknown>) {
    const prev = qc.getQueryData<Me>(ME_KEY)
    if (prev?.user) qc.setQueryData<Me>(ME_KEY, { ...prev, user: { ...prev.user, prefs: { ...(prev.user.prefs ?? {}), ...patch } } })
    const next = await api<UserPrefs>('/api/user/prefs', { method: 'PATCH', body: patch })
    const cur = qc.getQueryData<Me>(ME_KEY)
    if (cur?.user) qc.setQueryData<Me>(ME_KEY, { ...cur, user: { ...cur.user, prefs: next } })
  }
  return { prefs, update }
}

export function useMemberPrefs<T extends Record<string, unknown> = Record<string, unknown>>(wsId: Ref<string>) {
  const qc = useQueryClient()
  const key = computed(() => ['ws', wsId.value, 'prefs'])
  const query = useQuery({ queryKey: key, queryFn: () => api<T>(`/api/workspaces/${wsId.value}/prefs`), enabled: computed(() => !!wsId.value), staleTime: 5 * 60_000 })
  async function update(patch: Partial<T> | Record<string, unknown>) {
    qc.setQueryData<T>(key.value, (p) => ({ ...(p ?? {}), ...patch }) as T)
    const next = await api<T>(`/api/workspaces/${wsId.value}/prefs`, { method: 'PATCH', body: patch })
    qc.setQueryData<T>(key.value, next)
  }
  return { prefs: computed(() => query.data.value), loaded: computed(() => query.isSuccess.value), update }
}

/** Applique le correcteur orthographique (préférence) à un éditeur TipTap. */
export function useEditorSpellcheck(editor: Ref<Editor | undefined | null>) {
  const { prefs } = useUserPrefs()
  watch([editor, prefs], ([e, p]) => {
    const dom = e?.view?.dom as HTMLElement | undefined
    if (!dom) return
    dom.setAttribute('spellcheck', p.spellcheck === false ? 'false' : 'true')
    if (p.spellLang) dom.setAttribute('lang', p.spellLang)
    else dom.removeAttribute('lang')
  }, { immediate: true })
}
