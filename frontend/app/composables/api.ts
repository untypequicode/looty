import { toast } from './useToast'
import { translate } from './useI18n'

export interface ApiError {
  status: number
  code: string
}

const DETAILED = new Set(['mail_invalid_address', 'mail_smtp_error', 'mail_imap_error'])

/** Message lisible pour un code d'erreur renvoyé par le backend (`{ error: "code" }`). */
export function errorMessage(e: unknown): string {
  const err = e as any
  // Les erreurs détaillées ont la forme « code: détail »
  const raw = String(err?.data?.error || err?.code || '')
  const code: string = (raw.split(':')[0] ?? '').trim()
  const key = `errors.${code}`
  const msg = translate(key)
  // Détail utile à l'utilisateur (adresse refusée, réponse du serveur de messagerie).
  const detail = raw.includes(':') ? raw.slice(raw.indexOf(':') + 1).trim() : ''
  if (msg !== key) return detail && DETAILED.has(code) ? `${msg} (${detail})` : msg
  if (err?.status === 403 || err?.statusCode === 403) return translate('errors.forbidden')
  if (err?.status === 404 || err?.statusCode === 404) return translate('errors.not_found')
  return translate('errors.generic')
}

/**
 * Client HTTP de l'application. En cas de 401 on renvoie vers la connexion
 * en conservant la page courante pour y revenir ensuite.
 */
export async function api<T = any>(url: string, opts: Parameters<typeof $fetch>[1] & { silent?: boolean } = {}): Promise<T> {
  const { silent, ...rest } = opts as any
  try {
    return (await $fetch<T>(url, { credentials: 'include', ...rest })) as T
  } catch (e: any) {
    const status = e?.status ?? e?.statusCode
    if (status === 401 && typeof window !== 'undefined' && !url.startsWith('/api/auth/')) {
      const here = window.location.pathname + window.location.search + window.location.hash
      if (!window.location.pathname.startsWith('/login')) {
        await navigateTo({ path: '/login', query: { redirect: here } })
      }
    } else if (!silent) {
      toast.error(errorMessage(e))
    }
    throw e
  }
}

/** Upload d'un fichier vers /api/upload (ou une autre route multipart). */
export async function uploadFile(file: File, url = '/api/upload'): Promise<{ url: string; name: string; mime: string; size: number }> {
  const form = new FormData()
  form.append('file', file)
  return api(url, { method: 'POST', body: form })
}
