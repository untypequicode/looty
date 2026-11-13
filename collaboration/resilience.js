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
  if (status === 401 || status === 403 || status === 404) return 'forbidden'
  return 'auth-unavailable'
}

module.exports = { RETRY_DELAYS_MS, withRetry, authErrorForStatus }

/**
 * Relance l'enregistrement des documents dont l'enregistrement a échoué et qui sont encore
 * chargés ; oublie ceux qui ne le sont plus. `failed` : Map nom -> contexte ; `loaded` : Map nom -> document.
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
