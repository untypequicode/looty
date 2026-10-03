import { ref, computed } from 'vue'
import frCore from '~/i18n/fr'
import enCore from '~/i18n/en'
import frBoard from '~/i18n/fr-board'
import enBoard from '~/i18n/en-board'

export type Locale = 'fr' | 'en'
type Messages = Record<string, any>

/** Fusion profonde des dictionnaires (un fichier par domaine fonctionnel). */
function merge(...parts: Messages[]): Messages {
  const out: Messages = {}
  for (const p of parts) for (const [k, v] of Object.entries(p)) {
    out[k] = v && typeof v === 'object' && !Array.isArray(v) && out[k] && typeof out[k] === 'object' ? merge(out[k], v) : v
  }
  return out
}

const dictionaries: Record<Locale, Messages> = { fr: merge(frCore, frBoard), en: merge(enCore, enBoard) }
export const LOCALES: { code: Locale; label: string }[] = [
  { code: 'fr', label: 'Français' },
  { code: 'en', label: 'English' },
]

function detect(): Locale {
  if (typeof window === 'undefined') return 'fr'
  const saved = localStorage.getItem('looty.locale')
  if (saved === 'fr' || saved === 'en') return saved
  return navigator.language?.toLowerCase().startsWith('fr') ? 'fr' : 'en'
}

const locale = ref<Locale>(detect())

function lookup(dict: Messages, key: string): any {
  return key.split('.').reduce<any>((acc, k) => (acc == null ? undefined : acc[k]), dict)
}

/**
 * Traduit une clé. Paramètres interpolés via `{name}`.
 * Pluriel : si la valeur est un objet `{ one, other }` (et `zero` optionnel), `params.count` choisit la forme.
 */
export function translate(key: string, params?: Record<string, any>): string {
  let msg = lookup(dictionaries[locale.value], key) ?? lookup(dictionaries.fr, key)
  if (msg && typeof msg === 'object' && params && 'count' in params) {
    const n = Number(params.count)
    msg = (n === 0 && msg.zero) || (n === 1 ? msg.one : msg.other) || msg.other
  }
  if (typeof msg !== 'string') return key
  if (!params) return msg
  return msg.replace(/\{(\w+)\}/g, (_, k) => (params[k] ?? `{${k}}`).toString())
}

export function setLocale(l: Locale) {
  locale.value = l
  if (typeof window !== 'undefined') {
    localStorage.setItem('looty.locale', l)
    document.documentElement.lang = l
  }
}

export function useI18n() {
  const dateLocale = computed(() => (locale.value === 'fr' ? 'fr-FR' : 'en-US'))

  /** « il y a 5 min », « hier », « 12 mars »... */
  function relativeTime(input: string | number | Date | null | undefined): string {
    if (!input) return ''
    const d = new Date(input)
    const diff = (Date.now() - d.getTime()) / 1000
    const rtf = new Intl.RelativeTimeFormat(dateLocale.value, { numeric: 'auto' })
    const abs = Math.abs(diff)
    const sign = diff >= 0 ? -1 : 1
    if (abs < 45) return translate('time.justNow')
    if (abs < 3600) return rtf.format(sign * Math.round(abs / 60), 'minute')
    if (abs < 86400) return rtf.format(sign * Math.round(abs / 3600), 'hour')
    if (abs < 86400 * 7) return rtf.format(sign * Math.round(abs / 86400), 'day')
    return formatDate(d)
  }

  function formatDate(input: string | number | Date | null | undefined, opts?: Intl.DateTimeFormatOptions): string {
    if (!input) return ''
    const d = new Date(input)
    const sameYear = d.getFullYear() === new Date().getFullYear()
    return d.toLocaleDateString(dateLocale.value, opts ?? { day: 'numeric', month: 'short', ...(sameYear ? {} : { year: 'numeric' }) })
  }

  function formatDateTime(input: string | number | Date | null | undefined): string {
    if (!input) return ''
    return new Date(input).toLocaleString(dateLocale.value, { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' })
  }

  return { t: translate, locale, setLocale, dateLocale, relativeTime, formatDate, formatDateTime }
}
