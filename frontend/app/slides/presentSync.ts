/**
 * Synchronisation du mode présentation (issue #303).
 *  - Entre fenêtres du même navigateur (affichage + vue présentateur) : BroadcastChannel, avec repli
 *    sur l'événement `storage` du localStorage quand l'API n'existe pas.
 *  - Entre personnes : champ d'awareness `present` du document Yjs (« suivez-moi »), sans serveur de plus.
 */

export type Msg =
  | { t: 'hello' }
  | { t: 'pos'; slide: string; step: number; screen: '' | 'black' | 'white'; from: string }
  | { t: 'timer'; startedAt: number | null; pausedAt: number | null; from: string }
  | { t: 'bye'; from: string }

export interface Channel {
  post(m: Msg): void
  onMessage(fn: (m: Msg) => void): void
  close(): void
}

const isMsg = (v: unknown): v is Msg => !!v && typeof v === 'object' && typeof (v as { t?: unknown }).t === 'string'

export function openChannel(name: string): Channel {
  const handlers: ((m: Msg) => void)[] = []
  const emit = (m: unknown) => { if (isMsg(m)) handlers.forEach((h) => h(m)) }
  if (typeof BroadcastChannel === 'function') {
    const bc = new BroadcastChannel(name)
    bc.onmessage = (e) => emit(e.data)
    return { post: (m) => bc.postMessage(m), onMessage: (fn) => { handlers.push(fn) }, close: () => bc.close() }
  }
  const key = `${name}:msg`
  const on = (e: StorageEvent) => {
    if (e.key !== key || !e.newValue) return
    try { emit(JSON.parse(e.newValue).m) } catch { /* message illisible : ignoré */ }
  }
  window.addEventListener('storage', on)
  let n = 0
  return {
    // Une valeur différente à chaque envoi, sinon l'événement `storage` ne se déclenche pas.
    post: (m) => { try { localStorage.setItem(key, JSON.stringify({ n: ++n + Date.now(), m })) } catch { /* stockage plein ou refusé */ } },
    onMessage: (fn) => { handlers.push(fn) },
    close: () => window.removeEventListener('storage', on),
  }
}

export const channelName = (pageId: string) => `looty-present:${pageId}`

/** État publié par la personne qui présente (champ d'awareness `present`). */
export interface PresentState { on: boolean; slide: string; step: number; ts: number }
export interface Presenter extends PresentState { clientId: number; name: string; color: string }

/** La personne à suivre : la plus récente parmi celles qui partagent leur présentation (pas soi-même). */
export function pickPresenter(states: Iterable<[number, any]>, selfId: number): Presenter | null {
  let best: Presenter | null = null
  for (const [id, s] of states) {
    if (id === selfId || !s?.user) continue
    const p = s.present
    if (!p || p.on !== true || typeof p.slide !== 'string' || !p.slide) continue
    const ts = typeof p.ts === 'number' ? p.ts : 0
    if (!best || ts > best.ts) best = { on: true, slide: p.slide, step: Number.isFinite(p.step) ? Math.max(0, Math.trunc(p.step)) : 0, ts, clientId: id, name: String(s.user.name ?? ''), color: String(s.user.color ?? '') }
  }
  return best
}

/** Durée écoulée du minuteur (ms) : démarré à `startedAt`, figé à `pausedAt` s'il est en pause. */
export function elapsed(startedAt: number | null, pausedAt: number | null, now: number): number {
  if (startedAt === null) return 0
  return Math.max(0, (pausedAt ?? now) - startedAt)
}

export function formatClock(ms: number): string {
  const s = Math.floor(ms / 1000)
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const pad = (n: number) => String(n).padStart(2, '0')
  return h ? `${h}:${pad(m)}:${pad(s % 60)}` : `${pad(m)}:${pad(s % 60)}`
}
