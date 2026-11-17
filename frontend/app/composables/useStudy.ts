import { computed, type Ref } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from './api'
import { confirmDialog } from './useConfirm'
import { trashedToast } from './useTrash'
import { translate } from './useI18n'

/**
 * Entraînements (GLOSSARY, ADR 0011) : éléments de l'espace comme les pages. Format des éléments :
 * docs/study-items.md. Les tentatives et l'état des fiches sont propres à la personne connectée.
 */
export type StudyKind = 'qcm' | 'questionnaire' | 'flashcards' | 'summary'
export type StudyItemType = 'single' | 'multi' | 'text' | 'flashcard'

export interface StudyChoice { id: string; text: string }
export interface StudyItem {
  id?: string
  type: StudyItemType
  prompt: string
  choices: StudyChoice[] | null
  /** Absente (null) pour qui ne peut que s'entraîner, sauf verso des fiches. */
  answer: any
  explanation: string
}
export interface StudySummary {
  id: string; owner_id: string; workspace_id: string; parent_id: string | null; private_to: string | null; position: number
  title: string; kind: StudyKind; created_at: string; updated_at: string; item_count: number; last_score: number | null; last_max: number | null; can_edit: boolean
}
export interface StudySet {
  id: string; owner_id: string; workspace_id: string; parent_id: string | null; private_to: string | null; position: number
  title: string; kind: StudyKind; can_edit: boolean; is_owner: boolean; items: StudyItem[]
}
export interface StudyAttempt { id: string; set_id: string; started_at: string; finished_at: string | null; score: number; max_score: number; answers: Record<string, { given: any; correct: boolean | null }> }
export interface StudyCorrection { item_id: string; correct: boolean | null; answer: any; explanation: string }
export interface CardState { item_id: string; ease: number; interval_days: number; due_at: string }
export type Rating = 'again' | 'hard' | 'good' | 'easy'
/** Éléments de l'espace auxquels on lie un entraînement. */
export type StudyLinkKind = 'page' | 'task' | 'event' | 'board' | 'project'
export interface StudyLink { id: string; target_kind: StudyLinkKind; target_id: string; title: string }

/** Types d'éléments permis par type d'entraînement (miroir du serveur). */
export const ITEM_TYPES: Record<StudyKind, StudyItemType[]> = {
  qcm: ['single', 'multi'], questionnaire: ['single', 'multi', 'text'], flashcards: ['flashcard'], summary: ['flashcard'],
}
export const STUDY_KINDS: StudyKind[] = ['qcm', 'questionnaire', 'flashcards', 'summary']
export const isCardKind = (k: StudyKind) => k === 'flashcards' || k === 'summary'

export const studyKeys = {
  /** Sous `['ws', ws]` : les invalidations d'espace (personnel, déplacement…) la rechargent. */
  list: (ws: string) => ['ws', ws, 'study'] as const,
  set: (id: string) => ['study', 'set', id] as const,
  attempts: (id: string) => ['study', 'attempts', id] as const,
  cards: (id: string) => ['study', 'cards', id] as const,
  links: (id: string) => ['study', 'links', id] as const,
  linked: (kind: string, id: string) => ['study', 'linked', kind, id] as const,
}

/** Les entraînements de l'espace que la personne peut ouvrir. */
export function useStudySets(ws: Ref<string>) {
  return useQuery({
    queryKey: computed(() => studyKeys.list(ws.value)),
    queryFn: () => api<StudySummary[]>(`/api/study/sets?workspace_id=${ws.value}`),
    enabled: computed(() => !!ws.value),
  })
}

export function useStudySet(id: Ref<string>) {
  return useQuery({
    queryKey: computed(() => studyKeys.set(id.value)),
    queryFn: () => api<StudySet>(`/api/study/sets/${id.value}`, { silent: true }),
    retry: false,
  })
}

export function useStudyAttempts(id: Ref<string>) {
  return useQuery({
    queryKey: computed(() => studyKeys.attempts(id.value)),
    queryFn: () => api<StudyAttempt[]>(`/api/study/sets/${id.value}/attempts`),
  })
}

export function useStudyCards(id: Ref<string>) {
  return useQuery({
    queryKey: computed(() => studyKeys.cards(id.value)),
    queryFn: () => api<CardState[]>(`/api/study/sets/${id.value}/cards`),
  })
}

export interface StudyRevision { id: string; created_at: string; source: 'ai' | 'restore'; title: string; item_count: number }

/** Versions gardées avant chaque modification par l'IA ou restauration (éditeurs seulement). */
export function useStudyRevisions(id: Ref<string>) {
  return useQuery({
    queryKey: computed(() => ['study', 'revisions', id.value]),
    queryFn: () => api<StudyRevision[]>(`/api/study/sets/${id.value}/revisions`, { silent: true }),
    retry: false,
  })
}

/** Liens de l'entraînement (sans ceux dont la cible échappe à la personne). */
export function useStudyLinks(id: Ref<string>) {
  return useQuery({ queryKey: computed(() => studyKeys.links(id.value)), queryFn: () => api<StudyLink[]>(`/api/study/sets/${id.value}/links`) })
}

/** « Entraînements liés » d'un élément (page, tâche, événement, tableau, projet). */
export function useLinkedStudy(kind: Ref<StudyLinkKind>, id: Ref<string | undefined>) {
  return useQuery({
    queryKey: computed(() => studyKeys.linked(kind.value, id.value ?? '')),
    queryFn: () => api<StudySummary[]>(`/api/study/linked?target_kind=${kind.value}&target_id=${id.value}`, { silent: true }),
    enabled: computed(() => !!id.value),
  })
}

export function useStudyActions() {
  const qc = useQueryClient()
  /** Listes d'espace, arbre, liens et ensembles ouverts rechargés. */
  const refreshList = async () => {
    await Promise.all([
      qc.invalidateQueries({ queryKey: ['ws'], predicate: (q) => q.queryKey[2] === 'study' }),
      qc.invalidateQueries({ queryKey: ['study', 'linked'] }),
    ])
  }
  return {
    /** Remet une version ; renvoie la version qui annule cette restauration. */
    async restore(setId: string, revisionId: string) {
      const r = await api<{ set_id: string; undo_revision_id: string }>(`/api/ai/study-revisions/${revisionId}/restore`, { method: 'POST' })
      await qc.invalidateQueries({ queryKey: studyKeys.set(setId) })
      await qc.invalidateQueries({ queryKey: ['study', 'revisions', setId] })
      await refreshList()
      return r.undo_revision_id
    },
    async create(body: { title: string; kind: StudyKind; workspace_id: string; parent_id?: string | null; personal?: boolean }) {
      const set = await api<{ id: string }>('/api/study/sets', { method: 'POST', body })
      await refreshList()
      return set.id
    },
    async save(id: string, body: { title: string; kind: StudyKind; items: unknown[] }) {
      await api(`/api/study/sets/${id}`, { method: 'PUT', body })
      await qc.invalidateQueries({ queryKey: ['study', 'set', id] })
      await qc.invalidateQueries({ queryKey: ['study', 'revisions', id] })
      await refreshList()
    },
    /** Renommer, ranger sous une page (`parent_id`) ou changer de place. */
    async patch(id: string, body: { title?: string; parent_id?: string | null; position?: number }) {
      await api(`/api/study/sets/${id}`, { method: 'PUT', body })
      await qc.invalidateQueries({ queryKey: studyKeys.set(id) })
      await refreshList()
    },
    async duplicate(id: string, title: string) {
      const copy = await api<{ id: string; workspace_id: string }>(`/api/study/sets/${id}/duplicate`, { method: 'POST', body: { title } })
      await refreshList()
      return copy
    },
    /** Après confirmation, à la corbeille avec « Annuler ». Renvoie faux si abandonné. */
    async remove(set: { id: string; title: string }) {
      const ok = await confirmDialog({
        title: translate('study.deleteSet'),
        message: translate('study.deleteConfirm', { title: set.title || translate('study.untitled') }),
        confirmLabel: translate('common.delete'),
        danger: true,
      })
      if (!ok) return false
      await api(`/api/study/sets/${set.id}`, { method: 'DELETE' })
      await refreshList()
      trashedToast('study', set.id, () => { void refreshList() })
      return true
    },
    submitAttempt(id: string, answers: Record<string, unknown>, started_at: string) {
      return api<{ attempt: StudyAttempt; correction: StudyCorrection[] }>(`/api/study/sets/${id}/attempts`, { method: 'POST', body: { answers, started_at } })
        .then(async (r) => { await qc.invalidateQueries({ queryKey: studyKeys.attempts(id) }); await refreshList(); return r })
    },
    async review(setId: string, itemId: string, rating: Rating) {
      const st = await api<CardState>(`/api/study/items/${itemId}/review`, { method: 'POST', body: { rating } })
      await qc.invalidateQueries({ queryKey: studyKeys.cards(setId) })
      return st
    },
    async link(setId: string, target_kind: StudyLinkKind, target_id: string) {
      await api(`/api/study/sets/${setId}/links`, { method: 'POST', body: { target_kind, target_id } })
      await qc.invalidateQueries({ queryKey: studyKeys.links(setId) })
      await qc.invalidateQueries({ queryKey: ['study', 'linked'] })
    },
    /** Retire le lien d'un entraînement vers un élément (depuis la fiche de l'élément). */
    async unlinkTarget(setId: string, kind: StudyLinkKind, targetId: string) {
      const ls = await api<StudyLink[]>(`/api/study/sets/${setId}/links`)
      const l = ls.find((x) => x.target_kind === kind && x.target_id === targetId)
      if (l) await api(`/api/study/sets/${setId}/links/${l.id}`, { method: 'DELETE' })
      await qc.invalidateQueries({ queryKey: studyKeys.links(setId) })
      await qc.invalidateQueries({ queryKey: ['study', 'linked'] })
    },
    async unlink(setId: string, linkId: string) {
      await api(`/api/study/sets/${setId}/links/${linkId}`, { method: 'DELETE' })
      await qc.invalidateQueries({ queryKey: studyKeys.links(setId) })
      await qc.invalidateQueries({ queryKey: ['study', 'linked'] })
    },
  }
}
