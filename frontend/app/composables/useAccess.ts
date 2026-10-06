import { computed, type Ref } from 'vue'
import { useQuery } from '@tanstack/vue-query'
import { useLocalStorage, useSessionStorage } from '@vueuse/core'
import { api } from './api'
import { readMode } from './useReadMode'
import { useMemberPrefs } from './usePrefs'

/**
 * Droits effectifs dans l'espace :
 * 1. le **rôle** (préréglage ou rôle créé) fixe ce qui est permis par domaine ;
 * 2. le **mode personnel** de chaque domaine décide si l'édition est active :
 *    - `write` : toujours en édition (sauf mode lecture global) ;
 *    - `read` : lecture par défaut, « Modifier » l'active (mémorisé sur l'appareil) ;
 *    - `restricted` : verrouillé ; « Déverrouiller » l'active jusqu'à la fermeture de la
 *      fenêtre (onglet) — plus strict que la lecture.
 * Priorité : restreint > lecture > écriture. Les commentaires et la validation des tâches
 * restent possibles dans tous les modes (selon le rôle).
 */
export type Area = 'docs' | 'boards' | 'calendar'
export type AreaMode = 'write' | 'read' | 'restricted'
export type LevelName = 'none' | 'read' | 'comment' | 'validate' | 'edit' | 'manage'
export interface Permissions { docs: LevelName; boards: LevelName; calendar: LevelName; invite: boolean; share: boolean; admin: boolean }
export interface MyAccess { role: string; role_id: string | null; role_name: string | null; permissions: Permissions; group_tasks: boolean }
export interface AccessPrefs { modes?: Partial<Record<Area, AreaMode>>; modesChosen?: boolean; [k: string]: unknown }

export const LEVELS: Record<LevelName, number> = { none: 0, read: 1, comment: 2, validate: 3, edit: 4, manage: 5 }
export const AREAS: Area[] = ['docs', 'boards', 'calendar']

// État partagé (tous les composants) : modifications temporaires de l'édition.
const editOn = useLocalStorage<Record<string, boolean>>('looty.editOn', {})
const unlocked = useSessionStorage<Record<string, boolean>>('looty.unlocked', {})

export const accessKey = (ws: string) => ['ws', ws, 'me']

export function useAccess(wsId: Ref<string>) {
  const query = useQuery({
    queryKey: computed(() => accessKey(wsId.value)),
    queryFn: () => api<MyAccess>(`/api/workspaces/${wsId.value}/access/me`, { silent: true }),
    enabled: computed(() => !!wsId.value),
    staleTime: 60_000,
  })
  const prefs = useMemberPrefs<AccessPrefs>(wsId)
  const perms = computed<Permissions | null>(() => query.data.value?.permissions ?? null)
  const level = (a: Area) => LEVELS[perms.value?.[a] ?? 'none']
  /** Le rôle permet-il ce niveau dans ce domaine ? */
  const allows = (a: Area, l: LevelName) => level(a) >= LEVELS[l]

  const mode = (a: Area): AreaMode => prefs.prefs.value?.modes?.[a] ?? 'read'
  const key = (a: Area) => `${wsId.value}:${a}`
  /** L'édition est-elle active (rôle + mode personnel + mode lecture global) ? */
  function editing(a: Area): boolean {
    if (readMode.value) return false
    const m = mode(a)
    if (m === 'write') return true
    if (m === 'read') return !!editOn.value[key(a)]
    return !!unlocked.value[key(a)]
  }
  /** Bascule l'édition d'un domaine selon son mode (Modifier / Déverrouiller). */
  function toggleEditing(a: Area) {
    const k = key(a)
    if (readMode.value) readMode.value = false
    if (mode(a) === 'restricted') unlocked.value = { ...unlocked.value, [k]: !unlocked.value[k] }
    else editOn.value = { ...editOn.value, [k]: !editOn.value[k] }
  }
  async function setMode(a: Area, m: AreaMode) {
    await prefs.update({ modes: { ...(prefs.prefs.value?.modes ?? {}), [a]: m } })
  }
  async function chooseAreas(workOn: Area[]) {
    const modes = Object.fromEntries(AREAS.map((a) => [a, workOn.includes(a) ? 'read' : 'restricted'])) as Record<Area, AreaMode>
    await prefs.update({ modes, modesChosen: true })
  }

  const can = computed(() => ({
    /** Documents : modifier (rôle + mode). */
    editDocs: allows('docs', 'edit') && editing('docs'),
    commentDocs: allows('docs', 'comment'),
    /** Tâches : créer / modifier. */
    editTasks: allows('boards', 'edit') && editing('boards'),
    /** Tâches : valider / terminer (possible même en mode restreint). */
    validate: allows('boards', 'validate') && !readMode.value,
    /** Tableaux, colonnes, étiquettes. */
    manageBoards: allows('boards', 'manage') && editing('boards'),
    commentTasks: allows('boards', 'comment'),
    editCalendar: allows('calendar', 'edit') && editing('calendar'),
    seeDocs: allows('docs', 'read'),
    seeBoards: allows('boards', 'read'),
    seeCalendar: allows('calendar', 'read'),
    invite: !!perms.value?.invite,
    share: !!perms.value?.share,
    admin: !!perms.value?.admin,
  }))

  return { access: query.data, perms, allows, mode, editing, toggleEditing, setMode, chooseAreas, can, prefsLoaded: prefs.loaded, modesChosen: computed(() => !!prefs.prefs.value?.modesChosen) }
}

export interface WorkspaceRole { id: string; name: string; color: string; preset: 'owner' | 'admin' | 'member' | 'viewer' | null; permissions: Permissions; position: number; member_count: number }

/** Rôles de l'espace (préréglages puis rôles créés). */
export function useRoles(wsId: Ref<string>) {
  return useQuery({
    queryKey: computed(() => ['ws', wsId.value, 'roles']),
    queryFn: () => api<WorkspaceRole[]>(`/api/workspaces/${wsId.value}/access/roles`),
    enabled: computed(() => !!wsId.value),
    staleTime: 60_000,
  })
}

/** Résumé lisible des droits d'un rôle (« Documents : modifier · Tâches : valider… »). */
export function permSummary(p: Permissions, t: (k: string) => string): string {
  return AREAS.map((a) => `${t(`access.areas.${a}`)} : ${t(`access.levels.${p[a]}`)}`).join(' · ')
}
