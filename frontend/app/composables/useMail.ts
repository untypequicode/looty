import { computed, type Ref } from 'vue'
import { useQuery, useInfiniteQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from './api'

export interface MailAccount {
  id: string
  workspace_id: string
  label: string
  email: string
  imap_host: string
  imap_port: number
  security: 'tls' | 'starttls'
  username: string
  webmail_url?: string | null
  notify: boolean
  last_check_at?: string | null
  last_error?: string | null
  unread: number
}

export interface MailSummary {
  uid: number
  from_name: string
  from_addr: string
  subject: string
  snippet: string
  sent_at?: string | null
  seen: boolean
  has_attachments: boolean
}

export interface MailAddress { name?: string | null; address?: string | null }
export interface MailMessage {
  uid: number
  subject?: string | null
  from: MailAddress[]
  to: MailAddress[]
  cc: MailAddress[]
  date?: string | null
  html?: string | null
  text?: string | null
  attachments: { name: string; size: number; type?: string | null }[]
}

export const mailKeys = {
  accounts: ['mail', 'accounts'] as const,
  messages: (id: string, unread = false) => ['mail', 'messages', id, unread] as const,
  message: (id: string, uid: number) => ['mail', 'message', id, uid] as const,
}

export function useMailAccounts() {
  return useQuery({ queryKey: mailKeys.accounts, queryFn: () => api<MailAccount[]>('/api/mail/accounts'), refetchInterval: 120_000 })
}

/** Boîte de réception paginée (« Charger plus » va chercher les plus anciens sur le serveur). */
export function useMailMessages(account: Ref<string | null>, unread: Ref<boolean>) {
  return useInfiniteQuery({
    queryKey: computed(() => mailKeys.messages(account.value ?? '', unread.value)),
    queryFn: ({ pageParam }) => api<{ messages: MailSummary[]; has_more: boolean }>(
      `/api/mail/accounts/${account.value}/messages?${unread.value ? 'unread=true&' : ''}${pageParam ? `before=${pageParam}` : ''}`,
    ),
    initialPageParam: 0,
    getNextPageParam: (last) => (last.has_more && last.messages.length ? last.messages[last.messages.length - 1]!.uid : undefined),
    enabled: computed(() => !!account.value),
    refetchInterval: 120_000,
  })
}

export function useMailMessage(account: Ref<string | null>, uid: Ref<number | null>) {
  return useQuery({
    queryKey: computed(() => mailKeys.message(account.value ?? '', uid.value ?? 0)),
    queryFn: () => api<MailMessage>(`/api/mail/accounts/${account.value}/messages/${uid.value}`, { silent: true }),
    enabled: computed(() => !!account.value && uid.value != null),
    staleTime: Infinity,
    retry: false,
  })
}

export function useMailInvalidate() {
  const qc = useQueryClient()
  return () => qc.invalidateQueries({ queryKey: ['mail'], predicate: (q) => q.queryKey[1] !== 'message' })
}

export const mailUnread = (accounts?: MailAccount[]) => (accounts ?? []).reduce((n, a) => n + a.unread, 0)

/** Document affiché dans le cadre isolé : aucune ressource distante sauf si `images`. */
export function mailDoc(m: MailMessage, images: boolean, dark: boolean): string {
  const csp = `default-src 'none'; style-src 'unsafe-inline'; img-src data: cid:${images ? ' https: http:' : ''}; font-src data:`
  const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
  const linkify = (s: string) => esc(s).replace(/https?:\/\/[^\s<>"']+/g, (u) => `<a href="${u}" target="_blank" rel="noopener noreferrer">${u}</a>`)
  const body = m.html ?? `<pre class="plain">${linkify(m.text ?? '')}</pre>`
  // Messages en texte brut : couleurs du thème. Messages HTML : fond blanc (ils sont faits pour).
  const plain = !m.html
  return `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${csp}"><base target="_blank"><style>
html,body{margin:0;padding:0}body{padding:16px 20px;font:14px/1.5 -apple-system,system-ui,sans-serif;word-wrap:break-word;${plain ? (dark ? 'color:#e6e6e6;background:transparent' : 'color:#1f2023;background:transparent') : 'color:#1f2023;background:#fff'}}
a{color:${plain && dark ? '#8f9bf5' : '#4c5bd4'}}img{max-width:100%;height:auto}.plain{white-space:pre-wrap;font:inherit;margin:0}table{max-width:100%}blockquote{margin:0 0 0 8px;padding-left:10px;border-left:2px solid #ccc}
</style></head><body>${body}</body></html>`
}
