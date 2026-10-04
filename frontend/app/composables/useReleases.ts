import { computed, ref } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { useLocalStorage } from '@vueuse/core'
import { api } from './api'

export interface Release {
  platform: 'web' | 'desktop' | 'android'
  version: string
  date: string
  title: { en: string; fr: string }
  notes: { en: string[]; fr: string[] }
}
interface Latest { version: string; date: string; url: string }
export interface ReleasesInfo {
  server_version: string
  repository: string
  latest: { web?: Latest | null; desktop?: Latest | null; android?: Latest | null }
  releases: Release[]
  last_seen?: string | null
  show_whats_new: boolean
}

export const semver = (v: string): [number, number, number] => {
  const p = v.replace(/^v/, '').split(/[.-]/).map((x) => Number.parseInt(x, 10) || 0)
  return [p[0] ?? 0, p[1] ?? 0, p[2] ?? 0]
}
export const newer = (a: string, b: string) => {
  const x = semver(a), y = semver(b)
  return x[0] !== y[0] ? x[0] > y[0] : x[1] !== y[1] ? x[1] > y[1] : x[2] > y[2]
}

/** Application utilisée : web, bureau (balise posée par son proxy) ou Android (user agent). */
export function clientApp(): { platform: 'web' | 'desktop' | 'android'; version: string | null } {
  if (typeof document === 'undefined') return { platform: 'web', version: null }
  const meta = document.querySelector('meta[name="looty-desktop"]')?.getAttribute('content')
  // Anciennes versions de bureau : balise sans numéro de version.
  if (meta != null) return { platform: 'desktop', version: /\d+\.\d+/.test(meta) ? meta : '0.0.0' }
  const android = navigator.userAgent.match(/LootyAndroid\/([\d.]+)/)
  if (android) return { platform: 'android', version: android[1]! }
  return { platform: 'web', version: null }
}

/** Nouvelle version web déployée pendant que l'onglet était ouvert (cf. plugin updates). */
export const webUpdateReady = ref(false)

export function useReleases() {
  const qc = useQueryClient()
  const query = useQuery({ queryKey: ['releases'], queryFn: () => api<ReleasesInfo>('/api/releases', { silent: true }), staleTime: 60 * 60_000, refetchInterval: 6 * 60 * 60_000 })
  const client = clientApp()
  /** Version plus récente de l'application installée (bureau / Android). */
  const appUpdate = computed(() => {
    if (client.platform === 'web' || !client.version) return null
    const latest = query.data.value?.latest[client.platform]
    return latest && newer(latest.version, client.version) ? { platform: client.platform, ...latest } : null
  })
  // Notes de l'application installée (bureau / Android) : vues par appareil.
  const seenApp = useLocalStorage(`looty.releases.seen.${client.platform}`, '')
  const appNotes = computed(() => {
    const d = query.data.value
    if (!d || client.platform === 'web' || !client.version || seenApp.value === client.version) return []
    return d.releases.filter((r) => r.platform === client.platform && r.version === client.version)
  })
  /** Notes web pas encore vues (nouvelles versions du serveur depuis la dernière visite). */
  const webNotes = computed(() => {
    const d = query.data.value
    if (!d?.show_whats_new) return []
    return d.releases.filter((r) => r.platform === 'web' && !newer(r.version, d.server_version) && (!d.last_seen || newer(r.version, d.last_seen))).slice(0, 3)
  })
  const unseen = computed(() => [...webNotes.value, ...appNotes.value])
  async function markSeen() {
    const v = query.data.value?.server_version
    if (client.version) seenApp.value = client.version
    if (!v || !webNotes.value.length) return
    qc.setQueryData<ReleasesInfo>(['releases'], (d) => (d ? { ...d, last_seen: v, show_whats_new: false } : d))
    await api('/api/user/releases/seen', { method: 'PUT', body: { version: v }, silent: true }).catch(() => {})
  }
  return { data: query.data, client, appUpdate, markSeen, unseen }
}

/** Notes : texte échappé, **gras** et `code` seulement. */
export function noteHtml(s: string): string {
  const esc = s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
  return esc.replace(/\*\*(.+?)\*\*/g, '<b>$1</b>').replace(/`([^`]+)`/g, '<code>$1</code>')
}
