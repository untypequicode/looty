// Brouillon partagé de la zone de saisie de l'Assistant : le texte déjà tapé et les puces jointes. Il est gardé
// dans le navigateur (et partagé entre onglets) pour qu'un élément envoyé à l'IA depuis une autre page (cours
// Moodle…) s'ajoute au brouillon en cours, sans perdre ce qui est déjà écrit. Logique pure, sans Vue.

import type { AiAttachment } from '~/composables/useAi'

/** Un message ne porte pas plus de pièces jointes (le serveur en ignore au-delà). */
export const MAX_ATTACHMENTS = 10

export interface AssistantDraft { text: string; attachments: AiAttachment[] }

export const emptyDraft = (): AssistantDraft => ({ text: '', attachments: [] })

export const isMoodleKind = (kind: string): kind is 'moodle_course' | 'moodle_file' => kind === 'moodle_course' || kind === 'moodle_file'

/** Identité d'une puce : deux puces de même type et même élément sont les mêmes. */
export const attachmentKey = (a: Pick<AiAttachment, 'kind' | 'id'>) => `${a.kind}:${a.id}`

const KINDS = ['page', 'study', 'task', 'board', 'project', 'event', 'file', 'moodle_course', 'moodle_file']

/** Lecture tolérante du stockage : tout ce qui n'a pas la bonne forme est écarté. */
export function parseDraft(raw: string | null | undefined): AssistantDraft {
  try {
    const v = JSON.parse(raw ?? 'null')
    if (!v || typeof v !== 'object') return emptyDraft()
    const attachments = Array.isArray(v.attachments)
      ? v.attachments.filter((a: any) => a && typeof a.id === 'string' && typeof a.title === 'string' && KINDS.includes(a.kind)).slice(0, MAX_ATTACHMENTS)
      : []
    return { text: typeof v.text === 'string' ? v.text : '', attachments }
  } catch { return emptyDraft() }
}

export type AddResult = 'added' | 'duplicate' | 'full'

/** Ajoute une puce au brouillon (sans toucher au texte) : ni doublon, ni plus que `MAX_ATTACHMENTS`. */
export function withAttachment(d: AssistantDraft, a: AiAttachment): { draft: AssistantDraft; result: AddResult } {
  if (d.attachments.some((x) => attachmentKey(x) === attachmentKey(a))) return { draft: d, result: 'duplicate' }
  if (d.attachments.length >= MAX_ATTACHMENTS) return { draft: d, result: 'full' }
  return { draft: { ...d, attachments: [...d.attachments, a] }, result: 'added' }
}

export interface DraftStorage { getItem(key: string): string | null; setItem(key: string, value: string): void }

export const DRAFT_KEY = 'looty.ai.draft'

/**
 * Le brouillon et son stockage. `storage` est celui du navigateur (partagé entre onglets) ; `reload()` relit
 * le stockage après un changement fait par un autre onglet. Les abonnés sont prévenus à chaque changement.
 */
export function createDraftStore(storage: DraftStorage | null, key = DRAFT_KEY) {
  let state = parseDraft(storage?.getItem(key))
  const listeners = new Set<(d: AssistantDraft) => void>()
  const emit = () => listeners.forEach((l) => l(state))
  function set(next: AssistantDraft) {
    state = next
    try { storage?.setItem(key, JSON.stringify(state)) } catch { /* stockage plein ou indisponible : le brouillon reste en mémoire */ }
    emit()
  }
  return {
    get: () => state,
    setText: (text: string) => { if (text !== state.text) set({ ...state, text }) },
    setAttachments: (attachments: AiAttachment[]) => set({ ...state, attachments: attachments.slice(0, MAX_ATTACHMENTS) }),
    add(a: AiAttachment): AddResult {
      const r = withAttachment(state, a)
      if (r.result === 'added') set(r.draft)
      return r.result
    },
    remove: (a: Pick<AiAttachment, 'kind' | 'id'>) => set({ ...state, attachments: state.attachments.filter((x) => attachmentKey(x) !== attachmentKey(a)) }),
    clear: () => set(emptyDraft()),
    reload() {
      const next = parseDraft(storage?.getItem(key))
      if (JSON.stringify(next) !== JSON.stringify(state)) { state = next; emit() }
    },
    subscribe(fn: (d: AssistantDraft) => void) { listeners.add(fn); return () => { listeners.delete(fn) } },
  }
}

export type DraftStore = ReturnType<typeof createDraftStore>

// --- Éléments Moodle -----------------------------------------------------------------------------------

/** Cours Moodle joint : le serveur le relit avec le jeton de la personne et ne donne à l'IA que sa table des matières. */
export function moodleCourseAttachment(ws: string, course: { id: number; name: string }): AiAttachment {
  return { kind: 'moodle_course', id: `${ws}:${course.id}`, title: course.name, ws, courseId: course.id, courseName: course.name }
}

/** Fichier d'un cours Moodle (`url` : adresse d'origine du fichier sur la plateforme, `source` dans l'API des cours). */
export function moodleFileAttachment(ws: string, course: { id: number; name: string }, file: { name: string; source: string; size?: number }): AiAttachment {
  return { kind: 'moodle_file', id: `${ws}:${file.source}`, title: file.name, ws, courseId: course.id, courseName: course.name, url: file.source, size: file.size }
}

/** Corps envoyé au serveur pour une pièce jointe : les éléments Moodle ne portent pas d'identifiant (calculé côté serveur). */
export function attachmentPayload(a: AiAttachment): Record<string, unknown> {
  if (!isMoodleKind(a.kind)) return { kind: a.kind, id: a.id, title: a.title }
  return { kind: a.kind, title: a.title, workspace_id: a.ws, course_id: a.courseId, url: a.kind === 'moodle_file' ? a.url : undefined, course_title: a.kind === 'moodle_file' ? a.courseName : undefined }
}
