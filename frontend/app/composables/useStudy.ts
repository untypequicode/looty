import { computed, type Ref } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from './api'

/**
 * Jeux de révision (GLOSSARY). Format des éléments : docs/study-items.md.
 * Les tentatives et l'état des fiches sont propres à la personne connectée.
 */
export type StudyKind = 'qcm' | 'questionnaire' | 'flashcards' | 'summary'
export type StudyItemType = 'single' | 'multi' | 'text' | 'flashcard'
export type StudyVisibility = 'private' | 'workspace' | 'link'

export interface StudyChoice { id: string; text: string }
export interface StudyItem {
  id?: string
  type: StudyItemType
  prompt: string
  choices: StudyChoice[] | null
  /** Absente (null) pour qui ne peut que passer le jeu, sauf verso des fiches. */
  answer: any
  explanation: string
}
export interface StudySummary {
  id: string; owner_id: string; workspace_id: string | null; title: string; kind: StudyKind; visibility: StudyVisibility
  updated_at: string; item_count: number; last_score: number | null; last_max: number | null; can_edit: boolean
}
export interface StudySet {
  id: string; owner_id: string; workspace_id: string | null; title: string; kind: StudyKind; visibility: StudyVisibility
  share_token: string | null; can_edit: boolean; is_owner: boolean; items: StudyItem[]
}
export interface StudyAttempt { id: string; set_id: string; started_at: string; finished_at: string | null; score: number; max_score: number; answers: Record<string, { given: any; correct: boolean | null }> }
export interface StudyCorrection { item_id: string; correct: boolean | null; answer: any; explanation: string }
export interface CardState { item_id: string; ease: number; interval_days: number; due_at: string }
export type Rating = 'again' | 'hard' | 'good' | 'easy'

/** Types d'éléments permis par type de jeu (miroir du serveur). */
export const ITEM_TYPES: Record<StudyKind, StudyItemType[]> = {
  qcm: ['single', 'multi'], questionnaire: ['single', 'multi', 'text'], flashcards: ['flashcard'], summary: ['flashcard'],
}
export const isCardKind = (k: StudyKind) => k === 'flashcards' || k === 'summary'

export const studyKeys = { list: ['study', 'sets'] as const, set: (id: string) => ['study', 'set', id] as const, attempts: (id: string) => ['study', 'attempts', id] as const, cards: (id: string) => ['study', 'cards', id] as const }

const tokenQ = (token?: string | null) => (token ? `?token=${encodeURIComponent(token)}` : '')

export function useStudySets() {
  return useQuery({ queryKey: studyKeys.list, queryFn: () => api<StudySummary[]>('/api/study/sets') })
}

export function useStudySet(id: Ref<string>, token: Ref<string | null>) {
  return useQuery({
    queryKey: computed(() => [...studyKeys.set(id.value), token.value]),
    queryFn: () => api<StudySet>(`/api/study/sets/${id.value}${tokenQ(token.value)}`, { silent: true }),
    retry: false,
  })
}

export function useStudyAttempts(id: Ref<string>, token: Ref<string | null>) {
  return useQuery({
    queryKey: computed(() => studyKeys.attempts(id.value)),
    queryFn: () => api<StudyAttempt[]>(`/api/study/sets/${id.value}/attempts${tokenQ(token.value)}`),
  })
}

export function useStudyCards(id: Ref<string>, token: Ref<string | null>) {
  return useQuery({
    queryKey: computed(() => studyKeys.cards(id.value)),
    queryFn: () => api<CardState[]>(`/api/study/sets/${id.value}/cards${tokenQ(token.value)}`),
  })
}

export function useStudyActions() {
  const qc = useQueryClient()
  const refreshList = () => qc.invalidateQueries({ queryKey: studyKeys.list })
  return {
    async create(body: { title: string; kind: StudyKind; workspace_id: string | null }) {
      const set = await api<{ id: string }>('/api/study/sets', { method: 'POST', body })
      await refreshList()
      return set.id
    },
    async save(id: string, body: { title: string; kind: StudyKind; items: unknown[] }) {
      await api(`/api/study/sets/${id}`, { method: 'PUT', body })
      await qc.invalidateQueries({ queryKey: ['study', 'set', id] })
      await refreshList()
    },
    async remove(id: string) {
      await api(`/api/study/sets/${id}`, { method: 'DELETE' })
      await refreshList()
    },
    async setVisibility(id: string, visibility: StudyVisibility) {
      const row = await api<{ share_token: string | null }>(`/api/study/sets/${id}/visibility`, { method: 'PUT', body: { visibility } })
      await qc.invalidateQueries({ queryKey: ['study', 'set', id] })
      await refreshList()
      return row
    },
    submitAttempt(id: string, token: string | null, answers: Record<string, unknown>, started_at: string) {
      return api<{ attempt: StudyAttempt; correction: StudyCorrection[] }>(`/api/study/sets/${id}/attempts${tokenQ(token)}`, { method: 'POST', body: { answers, started_at } })
        .then(async (r) => { await qc.invalidateQueries({ queryKey: studyKeys.attempts(id) }); await refreshList(); return r })
    },
    async review(setId: string, itemId: string, token: string | null, rating: Rating) {
      const st = await api<CardState>(`/api/study/items/${itemId}/review${tokenQ(token)}`, { method: 'POST', body: { rating } })
      await qc.invalidateQueries({ queryKey: studyKeys.cards(setId) })
      return st
    },
  }
}
