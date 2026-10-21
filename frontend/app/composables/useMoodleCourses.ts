import { computed, type Ref } from 'vue'
import { useQueryClient } from '@tanstack/vue-query'
import { api } from './api'
import { useMemberPrefs } from './usePrefs'
import { moodleKeys, useMoodleData, type MoodleCourse } from './useMoodle'

/**
 * Cours Moodle du membre avec ce que Looty y ajoute (GLOSSARY) : cours favoris et masqués
 * (synchronisés avec le site Moodle), dossiers de cours, personnalisation, ordre manuel.
 * Dossiers, personnalisation et ordre sont propres à chaque membre dans chaque espace
 * (préférences de membre, clé `moodleCourses`) ; les réglages d'un cours qui disparaît
 * et les dossiers vides sont conservés.
 */
export interface CourseFolder { id: string; name: string; /** Dossier parent ; absent à la racine (les préférences n'enregistrent pas les null). */ parent?: string | null }
export interface CourseCustom { short?: string; color?: string; icon?: string }
export interface MoodleCoursesPrefs {
  folders: CourseFolder[]
  /** Cours → dossier (un cours est dans un seul dossier). */
  assign: Record<number, string>
  custom: Record<number, CourseCustom>
  /** Ordre manuel des cours. */
  order: number[]
}
const EMPTY: MoodleCoursesPrefs = { folders: [], assign: {}, custom: {}, order: [] }

/** Couleurs proposées pour un cours. */
export const COURSE_COLORS = ['#e5484d', '#e0701a', '#d9a400', '#3da35d', '#1ba6a6', '#2a78d6', '#6e56cf', '#c2409a', '#6b7280']

const newId = () => Math.random().toString(36).slice(2, 10)

export function useMoodleCourses(ws: Ref<string>, enabled?: Ref<boolean>) {
  const qc = useQueryClient()
  const query = useMoodleData<MoodleCourse[]>(ws, 'courses?all=true', computed(() => !!ws.value && (enabled?.value ?? true)))
  const { prefs, update } = useMemberPrefs<{ moodleCourses?: MoodleCoursesPrefs }>(ws)
  const state = computed<MoodleCoursesPrefs>(() => ({ ...EMPTY, ...(prefs.value?.moodleCourses ?? {}) }))
  const save = (next: MoodleCoursesPrefs) => update({ moodleCourses: next })

  /** Cours visibles dans les listes (les cours masqués n'y sont pas). */
  const courses = computed(() => (query.data.value ?? []).filter((c) => !c.hidden))
  const all = computed(() => query.data.value ?? [])
  const byId = (id?: number | null) => (id ? all.value.find((c) => c.id === id) ?? null : null)

  // --- Favori, masqué : même état que sur le site Moodle ---
  function patchCourse(id: number, change: Partial<MoodleCourse>) {
    qc.setQueryData<MoodleCourse[]>(moodleKeys.data(ws.value, 'courses?all=true'), (l) => l?.map((c) => (c.id === id ? { ...c, ...change } : c)))
  }
  async function setFlag(id: number, flag: 'favourite' | 'hidden', value: boolean) {
    patchCourse(id, { [flag]: value })
    try { await api(`/api/workspaces/${ws.value}/moodle/courses/${id}/${flag}`, { method: 'PUT', body: { value } }) } catch (e) { patchCourse(id, { [flag]: !value }); throw e }
  }
  const setFavourite = (id: number, value: boolean) => setFlag(id, 'favourite', value)
  const setHidden = (id: number, value: boolean) => setFlag(id, 'hidden', value)

  // --- Dossiers ---
  const folders = computed(() => state.value.folders)
  const folderOf = (courseId: number) => state.value.assign[courseId] ?? null
  const childrenOf = (parent: string | null) => folders.value.filter((f) => (f.parent ?? null) === parent)
  /** Le dossier et tous ses sous-dossiers. */
  function treeOf(id: string): string[] {
    return [id, ...childrenOf(id).flatMap((f) => treeOf(f.id))]
  }
  /** Chemin lisible : « Licence / Semestre 3 ». */
  function pathOf(id: string): string {
    const f = folders.value.find((x) => x.id === id)
    return f ? (f.parent ? `${pathOf(f.parent)} / ${f.name}` : f.name) : ''
  }
  async function addFolder(name: string, parent: string | null = null) {
    const folder: CourseFolder = { id: newId(), name, ...(parent ? { parent } : {}) }
    await save({ ...state.value, folders: [...folders.value, folder] })
    return folder
  }
  const renameFolder = (id: string, name: string) => save({ ...state.value, folders: folders.value.map((f) => (f.id === id ? { ...f, name } : f)) })
  /** Supprime le dossier et ses sous-dossiers ; leurs cours n'ont plus de dossier. */
  function removeFolder(id: string) {
    const gone = new Set(treeOf(id))
    const assign = Object.fromEntries(Object.entries(state.value.assign).filter(([, f]) => !gone.has(f)))
    return save({ ...state.value, folders: folders.value.filter((f) => !gone.has(f.id)), assign: assign as Record<number, string> })
  }
  function moveToFolder(courseId: number, folder: string | null) {
    const assign = { ...state.value.assign }
    if (folder) assign[courseId] = folder
    else delete assign[courseId]
    return save({ ...state.value, assign })
  }

  // --- Personnalisation (nom court, couleur, icône) ---
  const customOf = (id: number): CourseCustom => state.value.custom[id] ?? {}
  function setCustom(id: number, value: CourseCustom) {
    const custom = { ...state.value.custom }
    const clean = Object.fromEntries(Object.entries(value).filter(([, v]) => v)) as CourseCustom
    if (Object.keys(clean).length) custom[id] = clean
    else delete custom[id]
    return save({ ...state.value, custom })
  }
  /** Nom affiché d'un cours : le nom court choisi, sinon le nom complet. */
  const nameOf = (c: Pick<MoodleCourse, 'id' | 'fullname'>) => customOf(c.id).short || c.fullname

  // --- Ordre manuel ---
  /** Place `id` juste avant `before` (ou à la fin) dans l'ordre manuel. */
  function reorder(id: number, before: number | null, current: number[]) {
    const list = current.filter((x) => x !== id)
    const at = before == null ? -1 : list.indexOf(before)
    if (at < 0) list.push(id)
    else list.splice(at, 0, id)
    return save({ ...state.value, order: list })
  }

  return {
    query, courses, all, byId, setFavourite, setHidden,
    state, folders, folderOf, childrenOf, treeOf, pathOf, addFolder, renameFolder, removeFolder, moveToFolder,
    customOf, setCustom, nameOf, reorder,
  }
}
