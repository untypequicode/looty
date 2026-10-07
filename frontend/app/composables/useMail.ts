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
  display_name?: string | null
  signature?: string | null
  smtp_host?: string | null
  smtp_port?: number | null
  smtp_security?: 'tls' | 'starttls' | 'none' | null
  smtp_username?: string | null
  /** Envoi configuré (SMTP). */
  can_send: boolean
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

export type FolderRole = 'inbox' | 'sent' | 'trash' | 'drafts' | 'junk' | 'archive' | 'all' | 'flagged'
export interface MailFolder {
  /** Nom brut côté serveur (à renvoyer tel quel). */
  name: string
  label: string
  parent: string | null
  role: FolderRole | null
  selectable: boolean
  total: number
  unread: number
}
export interface FolderSummary extends MailSummary { to: string[]; flagged: boolean; answered: boolean }
export interface FolderMessage extends MailMessage {
  folder: string
  reply_to: MailAddress[]
  message_id?: string | null
  references: string[]
  flagged: boolean
  attachments: { index: number; name: string; size: number; type?: string | null }[]
}
export type MailFilter = 'all' | 'unread' | 'flagged'

export const mailKeys = {
  accounts: ['mail', 'accounts'] as const,
  messages: (id: string, unread = false) => ['mail', 'messages', id, unread] as const,
  message: (id: string, uid: number) => ['mail', 'message', id, uid] as const,
  folders: (id: string) => ['mail', 'folders', id] as const,
  folder: (id: string, folder: string, filter: MailFilter, q: string) => ['mail', 'folder', id, folder, filter, q] as const,
  folderMessage: (id: string, folder: string, uid: number) => ['mail', 'message', id, folder, uid] as const,
}

export function useMailFolders(account: Ref<string | null>) {
  return useQuery({
    queryKey: computed(() => mailKeys.folders(account.value ?? '')),
    queryFn: () => api<{ delimiter: string; folders: MailFolder[] }>(`/api/mail/accounts/${account.value}/folders`, { silent: true }),
    enabled: computed(() => !!account.value),
    refetchInterval: 120_000,
    staleTime: 30_000,
  })
}

/** Messages d'un dossier, 40 par page, filtre et recherche côté serveur. */
export function useMailFolder(account: Ref<string | null>, folder: Ref<string | null>, filter: Ref<MailFilter>, q: Ref<string>) {
  return useInfiniteQuery({
    queryKey: computed(() => mailKeys.folder(account.value ?? '', folder.value ?? '', filter.value, q.value)),
    queryFn: ({ pageParam }) => {
      const params = new URLSearchParams({ name: folder.value! })
      if (pageParam) params.set('before', String(pageParam))
      if (filter.value !== 'all') params.set(filter.value, 'true')
      if (q.value) params.set('q', q.value)
      return api<{ messages: FolderSummary[]; has_more: boolean; total: number }>(`/api/mail/accounts/${account.value}/folder?${params}`, { silent: true })
    },
    initialPageParam: 0,
    getNextPageParam: (last) => (last.has_more && last.messages.length ? last.messages[last.messages.length - 1]!.uid : undefined),
    enabled: computed(() => !!account.value && !!folder.value),
    refetchInterval: 120_000,
    staleTime: 20_000,
  })
}

/** Images intégrées (servies par Looty) converties en data: pour le cadre isolé. */
async function inlineImages(html: string): Promise<string> {
  const urls = [...new Set([...html.matchAll(/src="(\/api\/mail\/accounts\/[^"]+)"/g)].map((m) => m[1]!))]
  for (const u of urls) {
    try {
      const blob = await (await fetch(u.replace(/&amp;/g, '&'), { credentials: 'include' })).blob()
      const data = await new Promise<string>((res) => { const r = new FileReader(); r.onload = () => res(String(r.result)); r.readAsDataURL(blob) })
      html = html.split(`src="${u}"`).join(`src="${data}"`)
    } catch { /* image ignorée */ }
  }
  return html
}

export function useFolderMessage(account: Ref<string | null>, folder: Ref<string | null>, uid: Ref<number | null>) {
  return useQuery({
    queryKey: computed(() => mailKeys.folderMessage(account.value ?? '', folder.value ?? '', uid.value ?? 0)),
    queryFn: async () => {
      const m = await api<FolderMessage>(`/api/mail/accounts/${account.value}/folder/message?${new URLSearchParams({ name: folder.value!, uid: String(uid.value) })}`, { silent: true })
      if (m.html) m.html = await inlineImages(m.html)
      return m
    },
    enabled: computed(() => !!account.value && !!folder.value && uid.value != null),
    staleTime: Infinity,
    retry: false,
  })
}

export const attachmentUrl = (account: string, folder: string, uid: number, index: number) =>
  `/api/mail/accounts/${account}/folder/attachment?${new URLSearchParams({ name: folder, uid: String(uid), index: String(index) })}`

export const mailActions = {
  flags: (account: string, name: string, uids: number[], body: { seen?: boolean; flagged?: boolean }) =>
    api(`/api/mail/accounts/${account}/folder/flags`, { method: 'POST', body: { name, uids, ...body } }),
  move: (account: string, name: string, uids: number[], to: string) =>
    api(`/api/mail/accounts/${account}/folder/move`, { method: 'POST', body: { name, uids, to } }),
  remove: (account: string, name: string, uids: number[]) =>
    api<{ permanent: boolean; trash: string | null }>(`/api/mail/accounts/${account}/folder/delete`, { method: 'POST', body: { name, uids } }),
  createFolder: (account: string, label: string, parent?: string | null) =>
    api<{ name: string }>(`/api/mail/accounts/${account}/folders`, { method: 'POST', body: { label, parent } }),
  renameFolder: (account: string, name: string, label: string) =>
    api<{ name: string }>(`/api/mail/accounts/${account}/folders/rename`, { method: 'POST', body: { name, label } }),
  deleteFolder: (account: string, name: string) => api(`/api/mail/accounts/${account}/folders/delete`, { method: 'POST', body: { name } }),
  emptyFolder: (account: string, name: string) => api(`/api/mail/accounts/${account}/folders/empty`, { method: 'POST', body: { name } }),
}

export interface ComposeDraft {
  account: string
  mode: 'new' | 'reply' | 'replyAll' | 'forward'
  to: string[]
  cc: string[]
  bcc: string[]
  subject: string
  html: string
  in_reply_to?: string | null
  references?: string[]
  reply?: { folder: string; uid: number }
  forward?: { folder: string; uid: number; attachments: { name: string; size: number }[] }
  /** Message cité (réponse) ou transféré, ajouté sous le texte à l'envoi. */
  quote?: string | null
}
/** Brouillon en cours (un seul à la fois, partagé entre la page et le panneau). */
export const useComposeDraft = () => useState<ComposeDraft | null>('mail-compose', () => null)

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
