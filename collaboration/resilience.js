/** Parties pures de la résilience du serveur de collaboration (testées dans resilience.test.js). */

/** Délais entre les tentatives d'écriture en base (4 tentatives au total). */
const RETRY_DELAYS_MS = [250, 1000, 3000]

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

/**
 * Exécute `fn` ; en cas d'échec, la relance après chaque délai de `delays`, puis relance la
 * dernière erreur. `onRetry(error, attempt)` est appelé avant chaque nouvelle tentative.
 */
async function withRetry(fn, { delays = RETRY_DELAYS_MS, onRetry = () => {}, wait = sleep } = {}) {
  for (let attempt = 0; ; attempt++) {
    try {
      return await fn()
    } catch (e) {
      if (attempt >= delays.length) throw e
      onRetry(e, attempt + 1)
      await wait(delays[attempt])
    }
  }
}

/**
 * Raison d'échec d'authentification selon la réponse du backend : 401/403/404 sont des refus
 * définitifs (« forbidden »), le reste (5xx, 429, 408, redémarrage derrière un proxy…) est
 * transitoire (« auth-unavailable ») : le client réessaiera. Null si la réponse est un succès.
 */
function authErrorForStatus(status) {
  if (status >= 200 && status < 300) return null
  // 401 : session non reconnue (expirée, ou lecture momentanément impossible) : le client réessaie
  // quelques fois avant de conclure ; 403/404 : droits retirés ou page supprimée, refus définitif.
  if (status === 401) return 'unauthorized'
  if (status === 403 || status === 404) return 'forbidden'
  return 'auth-unavailable'
}

/**
 * Ligne de journal d'un refus de connexion : motif, statut renvoyé par le backend, document et
 * type de client, pour retrouver d'où vient une reconnexion en boucle. Jamais de cookie, de jeton
 * ni d'adresse : le navigateur et le document suffisent.
 */
function describeRefusal({ reason, status, documentName, hasToken, hasCookie, userAgent, cookieCount, cookieHash, origin, forwardedFor, client }) {
  const ua = String(userAgent || '-').replace(/\s+/g, ' ').slice(0, 80)
  const doc = String(documentName || '-').slice(0, 64)
  const via = hasToken ? 'token' : hasCookie ? 'cookie' : 'none'
  const clean = (v, n) => String(v || '-').replace(/[\s"]+/g, ' ').slice(0, n)
  let line = `refused reason=${reason}${status ? ` status=${status}` : ''} doc=${doc} via=${via} ua="${ua}"`
  if (cookieCount !== undefined) line += ` session_cookies=${cookieCount} session_sha=${cookieHash || '-'} origin="${clean(origin, 80)}" xff="${clean(forwardedFor, 80)}"`
  if (client) line += ` client=${client}`
  return line
}

/** Cookies `looty_session` d'un en-tête Cookie : nombre et préfixe sha256 (jamais la valeur) du premier. */
function sessionCookieInfo(cookieHeader, sha256) {
  const values = String(cookieHeader || '').split(';').map((p) => p.trim()).filter((p) => p.startsWith('looty_session='))
  return { count: values.length, hash: values.length ? sha256(values[0].slice('looty_session='.length)).slice(0, 8) : '' }
}

/**
 * Freine les clients refusés en boucle (session périmée sur un autre appareil…).
 * `note(key)` compte un refus ; `check(key)` renvoie { delayMs, giveUp } selon les refus récents
 * (fenêtre glissante) : d'abord un délai croissant avant de questionner le backend, puis, au-delà
 * de `giveUpAfter`, une réponse immédiate « session-expired » que le client traite comme définitive.
 * `success(key)` oublie le client.
 */
function createRefusalLimiter({ windowMs = 120_000, freeAttempts = 3, giveUpAfter = 8, maxDelayMs = 10_000, now = Date.now, maxClients = 5000 } = {}) {
  const clients = new Map()
  const recent = (key) => (clients.get(key) || []).filter((t) => now() - t < windowMs)
  return {
    note(key) {
      const list = recent(key)
      list.push(now())
      clients.delete(key)
      clients.set(key, list)
      if (clients.size > maxClients) clients.delete(clients.keys().next().value)
    },
    check(key) {
      const n = recent(key).length
      if (n >= giveUpAfter) return { delayMs: 0, giveUp: true }
      if (n < freeAttempts) return { delayMs: 0, giveUp: false }
      return { delayMs: Math.min(maxDelayMs, 500 * 2 ** (n - freeAttempts)), giveUp: false }
    },
    success(key) { clients.delete(key) },
  }
}

/**
 * Revérification périodique des droits : un refus (401/403/404) n'est suivi d'effet qu'à la deuxième
 * réponse consécutive ; toute autre réponse (succès, 5xx, 429…) remet le compteur à zéro sans fermer.
 * Renvoie { action: 'ok' | 'keep' | 'recheck' | 'forbid', failures }.
 */
function nextRevalidation(failures, status) {
  if (status >= 200 && status < 300) return { action: 'ok', failures: 0 }
  if (status === 401 || status === 403 || status === 404) {
    return failures + 1 >= 2 ? { action: 'forbid', failures: 0 } : { action: 'recheck', failures: failures + 1 }
  }
  return { action: 'keep', failures: 0 }
}

module.exports = { RETRY_DELAYS_MS, withRetry, authErrorForStatus, describeRefusal, nextRevalidation, sessionCookieInfo, createRefusalLimiter }

/**
 * Relance l'enregistrement des documents dont l'enregistrement a échoué et qui sont encore
 * chargés ; oublie ceux qui ne le sont plus .
 * `failed` : Map nom -> contexte ; `loaded` : Map nom -> document.
 * Renvoie les noms relancés.
 */
function retryFailedStores(failed, loaded, trigger) {
  const retried = []
  for (const [name, ctx] of [...failed]) {
    const doc = loaded.get(name)
    if (!doc) { failed.delete(name); continue }
    trigger(doc, ctx)
    retried.push(name)
  }
  return retried
}

module.exports.retryFailedStores = retryFailedStores
