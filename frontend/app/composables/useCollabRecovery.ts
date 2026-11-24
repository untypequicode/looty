import type { HocuspocusProvider } from '@hocuspocus/provider'

/**
 * Reprise des éditeurs collaboratifs (page, fichier de code, notebook) après une coupure ou un
 * refus passager. Le fournisseur Hocuspocus se reconnecte seul quand le WebSocket tombe, mais :
 * - une fermeture au niveau du document (« close ») ne relance rien ;
 * - un refus d'authentification passager (session non reconnue, serveur qui redémarre) laisse la
 *   connexion ouverte et non authentifiée.
 * Ici : délais croissants, nouvelle authentification + synchronisation tant que rien n'a abouti, et
 * distinction entre refus passager (on réessaie) et refus définitif (accès retiré).
 */

export const COLLAB_RETRY_DELAYS = [2000, 5000, 10000, 15000]
/** Raisons d'échec toujours passagères (le serveur ne conclut pas à un retrait de droits). */
const TRANSIENT_REASONS = ['auth-unavailable', 'restoring']
/** Refus qui ne deviennent définitifs qu'après ce nombre de nouvelles tentatives sans succès. */
const RETRIES_BEFORE_DENIED: Record<string, number> = { unauthorized: 4, forbidden: 1 }

export type AuthVerdict = 'transient' | 'denied'

export function useCollabRecovery(provider: HocuspocusProvider, opts: { onLost?: () => void } = {}) {
  let timer: ReturnType<typeof setTimeout> | null = null
  let attempts = 0
  let synced = false
  let halted = false
  let stopped = false
  const failures: Record<string, number> = {}

  function clear() { if (timer) clearTimeout(timer); timer = null }

  function schedule() {
    if (timer || stopped || halted || synced) return
    const delay = COLLAB_RETRY_DELAYS[Math.min(attempts, COLLAB_RETRY_DELAYS.length - 1)]!
    attempts++
    timer = setTimeout(async () => {
      timer = null
      if (stopped || halted || synced) return
      // Socket coupé : le fournisseur se reconnecte seul et s'authentifie à l'ouverture.
      const ws = (provider as any).configuration?.websocketProvider
      if (!ws || ws.status === 'connected') {
        try {
          await provider.sendToken()
          provider.startSync()
        } catch { /* nouvelle tentative ci-dessous */ }
      }
      schedule()
    }, delay)
  }

  /** Verdict d'un échec d'authentification : 'transient' (une reprise est planifiée) ou 'denied'. */
  function classify(reason?: string): AuthVerdict {
    const limit = reason ? RETRIES_BEFORE_DENIED[reason] : undefined
    let transient = !!reason && TRANSIENT_REASONS.includes(reason)
    if (!transient && reason && limit !== undefined) {
      failures[reason] = (failures[reason] ?? 0) + 1
      transient = failures[reason]! <= limit
    }
    if (transient) { schedule(); return 'transient' }
    halted = true
    clear()
    return 'denied'
  }

  function lost() {
    if (stopped) return
    synced = false
    opts.onLost?.()
    schedule()
  }

  provider.on('synced', () => {
    synced = true
    halted = false
    attempts = 0
    for (const k of Object.keys(failures)) delete failures[k]
    clear()
  })
  provider.on('close', lost)
  provider.on('disconnect', lost)

  function stop() { stopped = true; clear() }
  return { classify, stop }
}
