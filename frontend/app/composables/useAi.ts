import { ref } from 'vue'
import { api } from './api'

/** Panneau de l'agent IA (reste ouvert d'une page à l'autre). */
export const aiOpen = ref(false)

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
}

export interface AiConversation {
  id: string
  connection_id: string | null
  context_kind: 'page' | 'project' | 'workspace' | 'none'
  context_id: string | null
  title: string
  updated_at: string
}

export interface AiMessage { id: string; role: 'user' | 'assistant' | 'tool'; content: string; data?: any }
export interface AiGrant { id: string; page_id: string; page_title: string; workspace_id: string; mode: AiMode; expires_at: string }
export interface AiStudyGrant { id: string; kind: 'create' | 'update'; workspace_id: string | null; workspace_name: string | null; personal: boolean; set_id: string | null; set_title: string | null; expires_at: string }
export interface AiStudyGrant { id: string; kind: 'create' | 'update'; workspace_id: string | null; workspace_name: string | null; personal: boolean; set_id: string | null; set_title: string | null; expires_at: string }
export interface AiDiffLine { op: ' ' | '+' | '-'; text: string }
export interface AiSuggestion { id: string; page_id: string; diff: AiDiffLine[] }

/** Valeurs proposées à la création d'une connexion. */
export const AI_PROVIDERS: { value: AiProvider; label: string; model: string; baseUrl?: string }[] = [
  { value: 'openai', label: 'OpenAI', model: 'gpt-4.1' },
  { value: 'anthropic', label: 'Anthropic (Claude)', model: 'claude-sonnet-4-5' },
  { value: 'gemini', label: 'Google Gemini', model: 'gemini-2.5-flash' },
  { value: 'perplexity', label: 'Perplexity', model: 'sonar' },
  { value: 'openai_compatible', label: 'OpenAI-compatible (Ollama…)', model: 'llama3.1', baseUrl: 'http://localhost:11434/v1' },
]

export type AiStreamEvent =
  | { type: 'text'; delta: string }
  | { type: 'tool'; name: string; ok: boolean; page_id?: string; study_set_id?: string | null }
  | { type: 'suggestion'; suggestion: AiSuggestion }
  | { type: 'page_changed'; page_id: string }
  | { type: 'error'; message: string }
  | { type: 'done' }

/** Envoie un message et lit la réponse en flux (SSE par POST : pas d'`EventSource`). */
export async function streamMessage(conversationId: string, content: string, onEvent: (e: AiStreamEvent) => void, signal?: AbortSignal) {
  const res = await fetch(`/api/ai/conversations/${conversationId}/messages`, {
    method: 'POST',
    credentials: 'include',
    headers: { 'content-type': 'application/json', accept: 'text/event-stream' },
    body: JSON.stringify({ content }),
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
  conversations: (kind?: string, id?: string) => api<AiConversation[]>('/api/ai/conversations', { query: { context_kind: kind, context_id: id }, silent: true }),
  conversation: (id: string) => api<{ conversation: AiConversation; messages: AiMessage[]; grants: AiGrant[]; study_grants: AiStudyGrant[]; suggestions: AiSuggestion[] }>(`/api/ai/conversations/${id}`),
}

