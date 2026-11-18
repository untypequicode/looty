import { ref } from 'vue'

/** Demande de création d'un entraînement (fenêtre « Nouvel entraînement »). */
export const studyCreateRequest = ref<{ ws: string; parentId: string | null; personal: boolean } | null>(null)

/** Ouvre la fenêtre de création : dans l'espace `ws`, sous la page `parentId`, ou en personnel. */
export function openStudyCreate(ws: string, opts: { parentId?: string | null; personal?: boolean } = {}) {
  studyCreateRequest.value = { ws, parentId: opts.parentId ?? null, personal: opts.personal ?? false }
}
