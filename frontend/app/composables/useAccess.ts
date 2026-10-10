import { computed, watch, type Ref } from 'vue'
import { useQuery } from '@tanstack/vue-query'
import { useLocalStorage, useSessionStorage } from '@vueuse/core'
import { api } from './api'
import { useMemberPrefs } from './usePrefs'

/**
 * Droits effectifs dans l'espace :
 * 1. le **rôle** (préréglage ou rôle créé) fixe ce qui est permis par domaine ;
 * 2. un **seul état d'édition par domaine** (documents, tâches, calendrier), dont la valeur
 *    par défaut vient du mode personnel :
 *    - `write` : en édition ;
 *    - `read` : en lecture, « Modifier » l'active (mémorisé sur l'appareil) ;
 *    - `restricted` : verrouillé ; « Déverrouiller » l'active jusqu'à la fermeture de la
 *      fenêtre (onglet) — plus strict que la lecture.
 *    Le bouton de l'en-tête change le domaine affiché ; le mode lecture (barre latérale,
 *    Ctrl+Maj+E) change tous les domaines d'un coup : c'est le même état.
 * Les commentaires et la validation des tâches restent possibles en lecture (selon le rôle).
 */
export type Area = 'docs' | 'boards' | 'calendar'
export type AreaMode = 'write' | 'read' | 'restricted'
export type LevelName = 'none' | 'read' | 'comment' | 'validate' | 'edit' | 'manage'
export interface Permissions { docs: LevelName; boards: LevelName; calendar: LevelName; invite: boolean; share: boolean; admin: boolean }
export interface MyAccess { role: string; role_id: string | null; role_name: string | null; permissions: Permissions; group_tasks: boolean }
export interface AccessPrefs { modes?: Partial<Record<Area, AreaMode>>; modesChosen?: boolean; [k: string]: unknown }

export const LEVELS: Record<LevelName, number> = { none: 0, read: 1, comment: 2, validate: 3, edit: 4, manage: 5 }
export const AREAS: Area[] = ['docs', 'boards', 'calendar']

// État partagé (tous les composants) : édition choisie par domaine, en plus du mode par défaut.
const editOn = useLocalStorage<Record<string, boolean>>('looty.editOn', {})
const unlocked = useSessionStorage<Record<string, boolean>>('looty.unlocked', {})
// Ancien mode lecture global (avant la fusion) : appliqué une fois à chaque espace.
const legacyRead = useLocalStorage('looty.readMode', false)
const legacyApplied = useLocalStorage<string[]>('looty.readModeApplied', [])

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

  const isGuest = computed(() => query.data.value?.role === 'guest')
  // Invités : pas de préférences ; ils modifient selon le partage de chaque page.
  // Documents partagés page par page (invités, rôles qui ne modifient pas les documents) :
  // le partage décide, page par page ; pas de mode de lecture propre.
  const sharedOnly = (a: Area) => a === 'docs' && (isGuest.value || !allows('docs', 'edit'))
  const mode = (a: Area): AreaMode => (sharedOnly(a) ? 'write' : prefs.prefs.value?.modes?.[a] ?? 'read')
  const key = (a: Area) => `${wsId.value}:${a}`
  /** L'édition de ce domaine est-elle active (état choisi, sinon mode par défaut) ? */
  function editing(a: Area): boolean {
    const k = key(a)
    const m = mode(a)
    if (m === 'restricted') return !!unlocked.value[k]
    const chosen = editOn.value[k]
    return chosen ?? m === 'write'
  }
  function setEditing(a: Area, on: boolean) {
    const k = key(a)
    if (mode(a) === 'restricted') unlocked.value = { ...unlocked.value, [k]: on }
    else editOn.value = { ...editOn.value, [k]: on }
  }
  /** Bascule l'édition d'un domaine (Modifier / Terminer, Déverrouiller / Verrouiller). */
  function toggleEditing(a: Area) {
    setEditing(a, !editing(a))
  }
  /** Domaines que le rôle permet de modifier (les invités : documents, selon chaque page). */
  const editableAreas = computed(() => AREAS.filter((a) => allows(a, 'edit') || (isGuest.value && a === 'docs')))
  /** Mode lecture : aucun domaine en édition (la barre latérale et Ctrl+Maj+E le basculent). */
  const reading = computed(() => editableAreas.value.length > 0 && editableAreas.value.every((a) => !editing(a)))
  function setReading(on: boolean) {
    for (const a of editableAreas.value) setEditing(a, !on)
  }
  // Ancien mode lecture global : cet espace commence en lecture, une seule fois.
  watch([() => wsId.value, () => query.data.value], ([ws, me]) => {
    if (!ws || !me || !legacyRead.value || legacyApplied.value.includes(ws)) return
    legacyApplied.value = [...legacyApplied.value, ws]
    setReading(true)
  }, { immediate: true })
  async function setMode(a: Area, m: AreaMode) {
    // Nouveau mode : on repart de sa valeur par défaut.
    const k = key(a)
    const { [k]: _e, ...restOn } = editOn.value
    const { [k]: _u, ...restUnlocked } = unlocked.value
    editOn.value = restOn
    unlocked.value = restUnlocked
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
    validate: allows('boards', 'validate'),
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

  return { access: query.data, perms, allows, mode, editing, setEditing, toggleEditing, reading, setReading, editableAreas, setMode, chooseAreas, can, prefsLoaded: prefs.loaded, modesChosen: computed(() => !!prefs.prefs.value?.modesChosen) }
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
