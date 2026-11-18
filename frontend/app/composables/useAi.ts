import { ref } from 'vue'
import { api, errorMessage } from './api'
import { links } from './useWorkspace'
import type { EndedGrant } from '~/utils/grantState'
import { attachmentPayload } from '~/utils/assistantDraft'

export type AiProvider = 'openai' | 'anthropic' | 'gemini' | 'perplexity' | 'openai_compatible'
export type AiMode = 'suggest' | 'append' | 'replace'

export interface AiConnection {
  id: string
  provider: AiProvider
  label: string
  model: string
  base_url: string | null
  has_key: boolean
  key_hint: string | null
  /** La personne a déjà vu l'avis « le contenu d'un fichier part chez ce fournisseur ». */
  files_notice_seen?: boolean
}

export interface AiConversation {
  id: string
  connection_id: string | null
  context_kind: 'page' | 'project' | 'workspace' | 'none'
  context_id: string | null
  title: string
  updated_at: string
}

/** Type d'élément joignable à un message (la même liste que les résultats de recherche utiles). */
export type AiAttachKind = 'page' | 'study' | 'task' | 'board' | 'project' | 'event' | 'file' | 'moodle_course' | 'moodle_file'
/** Élément joint : le serveur le relit avec vos droits à l'envoi (rien du contenu ne part du navigateur). */
export interface AiAttachment {
  kind: AiAttachKind; id: string; title: string; ws?: string
  /** Fichier : échéance dans le Drive (au-delà, l'assistant ne le lit plus). */
  expiresAt?: string | null; pages?: number
  /** Moodle : cours (et son nom) et, pour un fichier, son adresse d'origine sur la plateforme. `ws` est l'espace de la connexion Moodle. */
  courseId?: number; courseName?: string; url?: string; size?: number
}

export interface AiMessage { id: string; role: 'user' | 'assistant' | 'tool'; content: string; data?: any; created_at?: string }
export interface AiGrant { id: string; page_id: string; page_title: string; workspace_id: string; mode: AiMode; expires_at: string }
export interface AiStudyGrant { id: string; kind: 'create' | 'update'; workspace_id: string | null; workspace_name: string | null; personal: boolean; set_id: string | null; set_title: string | null; expires_at: string }
export interface AiDiffLine { op: ' ' | '+' | '-'; text: string }
export interface AiSuggestion { id: string; page_id: string; diff: AiDiffLine[] }
/** Autorisation que l'assistant a demandée sans l'avoir (carte dans la discussion). */
export type AiGrantNeed =
  | { kind: 'page'; page_id: string; workspace_id: string; title: string; operation: 'append' | 'replace' }
  | { kind: 'study_create'; workspace_id: string | null; personal: boolean; title: string }
  | { kind: 'study_update'; set_id: string; title: string }

/** Éléments à joindre au prochain message de la page Assistant (« Demander à l'IA » dans un menu). */
export const aiInbox = ref<AiAttachment[]>([])

/** Ouvre l'Assistant avec cet élément joint. */
export function askAi(ws: string, item: AiAttachment) {
  aiInbox.value = [{ ...item, ws }]
  return navigateTo(links.assistant(ws))
}


/**
 * Erreur du fournisseur (« provider_http_503: {...} » dans le flux) → phrase courte et lisible.
 * `t` est la fonction de traduction ; les autres codes passent par les messages d'erreur communs.
 */
export function aiErrorMessage(raw: string, t: (k: string) => string): string {
  const m = /^provider_http_(\d{3})/.exec(raw)
  if (m) {
    const status = Number(m[1])
    if (status === 429 || /quota|RESOURCE_EXHAUSTED|rate.?limit/i.test(raw)) return t('ai.chat.err.quota')
    if (status === 401 || status === 403) return t('ai.chat.err.auth')
    if (status === 404) return t('ai.chat.err.model')
    if (status === 503 || status === 502 || status === 504 || /overloaded|UNAVAILABLE/i.test(raw)) return t('ai.chat.err.busy')
    return status >= 500 ? t('ai.chat.err.server') : t('ai.chat.err.request')
  }
  if (raw.startsWith('provider_unreachable')) return t('ai.chat.err.unreachable')
  return errorMessage({ data: { error: raw } })
}

export type AiStreamEvent =
  | { type: 'text'; delta: string }
  | { type: 'tool'; name: string; ok: boolean; page_id?: string; study_set_id?: string | null }
  | { type: 'suggestion'; suggestion: AiSuggestion }
  | { type: 'page_changed'; page_id: string }
  | { type: 'grant_needed'; grant: AiGrantNeed }
  | { type: 'title'; title: string }
  | { type: 'error'; message: string }
  | { type: 'done' }

/** Envoie un message et lit la réponse en flux (SSE par POST : pas d'`EventSource`). */
export async function streamMessage(conversationId: string, content: string, attachments: AiAttachment[], onEvent: (e: AiStreamEvent) => void, signal?: AbortSignal, workspaceId?: string) {
  const res = await fetch(`/api/ai/conversations/${conversationId}/messages`, {
    method: 'POST',
    credentials: 'include',
    headers: { 'content-type': 'application/json', accept: 'text/event-stream' },
    body: JSON.stringify({ content, workspace_id: workspaceId || undefined, attachments: attachments.map(attachmentPayload) }),
    signal,
  })
  if (!res.ok || !res.body) {
    const body = await res.json().catch(() => ({}))
    throw Object.assign(new Error('ai_http'), { status: res.status, data: body })
  }
  const reader = res.body.getReader()
  const decoder = new TextDecoder()
  let buf = ''
  for (;;) {
    const { done, value } = await reader.read()
    if (done) break
    buf += decoder.decode(value, { stream: true })
    let i: number
    while ((i = buf.indexOf('\n\n')) >= 0) {
      const frame = buf.slice(0, i)
      buf = buf.slice(i + 2)
      const data = frame.split('\n').filter((l) => l.startsWith('data:')).map((l) => l.slice(5).trim()).join('')
      if (data) {
        try { onEvent(JSON.parse(data)) } catch { /* ligne de maintien de connexion */ }
      }
    }
  }
}

export const aiApi = {
  connections: () => api<AiConnection[]>('/api/ai/connections'),
  ackFilesNotice: (id: string) => api(`/api/ai/connections/${id}/files-notice`, { method: 'POST', silent: true }),
  conversations: (q?: string) => api<AiConversation[]>('/api/ai/conversations', { query: { q: q || undefined }, silent: true }),
  conversation: (id: string) => api<{ conversation: AiConversation; messages: AiMessage[]; grants: AiGrant[]; study_grants: AiStudyGrant[]; ended_grants?: EndedGrant[]; suggestions: AiSuggestion[]; restored_revisions?: string[]; default_workspace_id?: string | null }>(`/api/ai/conversations/${id}`),
}

