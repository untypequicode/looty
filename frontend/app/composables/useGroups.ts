import { computed, type Ref } from 'vue'
import { useQuery } from '@tanstack/vue-query'
import { api } from './api'
import { translate as t } from './useI18n'

/** Groupe de membres : ensemble nommé de membres, sans droit (mentions, ajout groupé). */
export interface MemberGroup { id: string; name: string; color: string; members: string[] }

export const groupsKey = (ws: string) => ['ws', ws, 'groups'] as const

/** Groupes de l'espace, tenus à jour par le temps réel (GROUPS_UPDATED). */
export function useGroups(wsId: Ref<string>) {
  return useQuery({
    queryKey: computed(() => groupsKey(wsId.value)),
    queryFn: () => api<MemberGroup[]>(`/api/workspaces/${wsId.value}/groups`, { silent: true }),
    enabled: computed(() => !!wsId.value),
    staleTime: 60_000,
  })
}

/** Préfixe des valeurs « groupe » dans les listes de choix de personnes. */
export const GROUP_PREFIX = 'group:'
const GROUP = GROUP_PREFIX

/**
 * Choix de personnes avec groupes (assignés d'une tâche, participants d'un événement).
 * Sans `linked`, choisir un groupe ajoute ses membres du moment (`expand`) ; avec, le groupe
 * est relié et l'élément suit ses membres (GLOSSARY).
 */
export function useGroupChoices(wsId: Ref<string>, selected: () => string[], linked: () => string[] | undefined = () => undefined) {
  const { data: groups } = useGroups(wsId)
  const items = computed(() => {
    const cur = new Set(selected())
    const link = linked()
    return (groups.value ?? [])
      .filter((g) => g.members.length || link?.includes(g.id))
      .map((g) => {
        if (link) return { value: GROUP + g.id, label: `@${g.name}`, color: g.color, hint: t('memberGroups.linkedHint', { count: g.members.length }), keywords: g.name, section: t('memberGroups.section') }
        const missing = g.members.filter((m) => !cur.has(m)).length
        return {
          value: GROUP + g.id,
          label: `@${g.name}`,
          color: g.color,
          hint: missing ? t('memberGroups.addGroup', { count: missing }) : t('memberGroups.allIn'),
          keywords: g.name,
          section: t('memberGroups.section'),
        }
      })
  })
  /** Remplace les groupes choisis par leurs membres (sans doublon). */
  function expand(values: string[]): string[] {
    const out: string[] = []
    for (const v of values) {
      const ids = v.startsWith(GROUP) ? groups.value?.find((g) => GROUP + g.id === v)?.members ?? [] : [v]
      for (const id of ids) if (!out.includes(id)) out.push(id)
    }
    return out
  }
  return { groups, items, expand }
}
