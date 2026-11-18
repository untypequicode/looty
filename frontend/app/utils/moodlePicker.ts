// Logique pure du groupe « Moodle » du sélecteur « @ » de l'Assistant : cours de la connexion Moodle de la
// personne, et fichiers lisibles d'un cours (parcours d'un cours, recherche dans ce cours).

import { fold } from './fold.ts'
import { isReadable } from './chatFiles.ts'
import type { PickerEntry } from './pickerSearch.ts'

/** Ce que la liste des cours de Moodle donne sur un cours (voir `MoodleCourse`). */
export interface PickerCourse { id: number; fullname: string; shortname?: string; category?: string | null; favourite?: boolean; hidden?: boolean }
/** Ce que le contenu d'un cours donne sur une activité et ses fichiers (voir `MoodleCourseContent`). */
export interface CourseSection { id: number; name: string; modules: { id: number; name: string; type: string; type_name?: string | null; files: { name: string; size: number; mime: string | null; html?: boolean; source: string }[] }[] }

const norm = (q: string) => fold(q.trim())

/** Clé d'une entrée du sélecteur pour un cours ou un fichier Moodle (même forme que `attachmentKey`). */
export const courseKey = (ws: string, id: number) => `moodle_course:${ws}:${id}`
export const fileKey = (ws: string, source: string) => `moodle_file:${ws}:${source}`

/**
 * Cours d'un espace, filtrés par la recherche (nom, nom court, catégorie). Les cours masqués ne sont proposés
 * que si la recherche les trouve. L'ordre reçu (favoris, derniers consultés) est conservé.
 */
export function courseEntries(ws: string, wsName: string, courses: readonly PickerCourse[], query: string, multiWorkspace = false): PickerEntry[] {
  const q = norm(query)
  return courses
    .filter((c) => (q ? [c.fullname, c.shortname ?? '', c.category ?? ''].some((s) => fold(s).includes(q)) : !c.hidden))
    .map((c) => ({
      key: courseKey(ws, c.id),
      kind: 'moodle_course',
      id: `${ws}:${c.id}`,
      ws,
      title: c.fullname,
      place: [multiWorkspace ? wsName : '', c.category ?? ''].filter(Boolean).join(' › '),
      excerpt: '',
      courseId: c.id,
      courseName: c.fullname,
    }))
}

/**
 * Fichiers d'un cours que l'assistant sait lire (PDF, Word, PowerPoint, texte, code), filtrés par la recherche
 * (nom du fichier, de l'activité ou de la section). Chaque entrée garde sa section, son type et sa taille pour l'aperçu.
 */
export function fileEntries(ws: string, course: { id: number; name: string }, sections: readonly CourseSection[], query: string): PickerEntry[] {
  const q = norm(query)
  const out: PickerEntry[] = []
  for (const s of sections) {
    for (const m of s.modules) {
      for (const f of m.files) {
        // Les pages HTML de la plateforme (page, livre) se lisent dans Looty, pas comme des fichiers.
        if (f.html || !isReadable(f.name, f.mime ?? '')) continue
        if (q && ![f.name, m.name, s.name].some((x) => fold(x).includes(q))) continue
        out.push({
          key: fileKey(ws, f.source),
          kind: 'moodle_file',
          id: `${ws}:${f.source}`,
          ws,
          title: f.name,
          place: [course.name, s.name].filter(Boolean).join(' › '),
          excerpt: '',
          courseId: course.id,
          courseName: course.name,
          url: f.source,
          size: f.size,
          section: s.name,
          type: m.type_name || m.type,
        })
      }
    }
  }
  return out
}
