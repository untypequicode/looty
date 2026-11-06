// Parcours complet avec le Pocket ID inclus, sur une instance déjà démarrée (deploy/test/caddy-stack.sh pocket-id) :
// création du premier compte Pocket ID (ce que fait la page /setup), puis connexion à Looty par OIDC.
//   NODE_EXTRA_CA_CERTS=<racine de l'AC> node deploy/test/pocket-id.mjs https://localhost https://auth.localhost <OIDC_PROVIDER_ID>
// Vérifie que le client créé par pocket-id-init est accepté (URL de redirection exacte, secret, sans écran de
// consentement) et que ce premier compte devient administrateur de Looty. Code de sortie non nul au premier échec.
import assert from 'node:assert/strict'

const looty = new URL(process.argv[2] ?? 'https://localhost')
const auth = new URL(process.argv[3] ?? 'https://auth.localhost')
const providerId = process.argv[4]
assert.ok(providerId, 'usage: pocket-id.mjs <looty URL> <Pocket ID URL> <OIDC_PROVIDER_ID>')

/** Petit bocal à cookies, un par site. */
class Jar {
  cookies = new Map()
  take(res) {
    for (const line of res.headers.getSetCookie()) {
      const [pair] = line.split(';')
      const i = pair.indexOf('=')
      this.cookies.set(pair.slice(0, i).trim(), pair.slice(i + 1).trim())
    }
  }
  header() {
    return [...this.cookies].map(([k, v]) => `${k}=${v}`).join('; ')
  }
}
const lootyJar = new Jar()
const authJar = new Jar()

async function get(url, jar, init = {}) {
  const res = await fetch(url, { redirect: 'manual', ...init, headers: { ...init.headers, cookie: jar.header() } })
  jar.take(res)
  return res
}

const steps = {
  async 'Pocket ID: /setup page is served'() {
    const res = await fetch(new URL('/setup', auth))
    assert.equal(res.status, 200)
    assert.match(res.headers.get('content-type') ?? '', /text\/html/)
  },

  async 'Pocket ID: first account (admin)'() {
    const res = await get(new URL('/api/signup/setup', auth), authJar, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ username: 'admin', email: 'admin@example.com', firstName: 'Ada', lastName: 'Admin' }),
    })
    const text = await res.text()
    assert.equal(res.status, 200, text)
    assert.equal(JSON.parse(text).isAdmin, true)
    // « __Host-access_token » en HTTPS, « access_token » sinon.
    assert.ok([...authJar.cookies.keys()].some((k) => k.endsWith('access_token')), 'no Pocket ID session cookie')
  },

  async 'Looty: the bundled provider is listed with its fixed id'() {
    const res = await fetch(new URL('/api/auth/providers', looty))
    assert.equal(res.status, 200)
    assert.deepEqual((await res.json()).map((p) => p.id), [providerId])
  },

  async 'Looty: OIDC sign-in through Pocket ID'() {
    const start = await get(new URL(`/api/auth/login/${providerId}?redirect=/`, looty), lootyJar)
    assert.equal(start.status, 200, await start.clone().text())
    const authorize = new URL((await start.json()).url)
    assert.equal(authorize.origin, auth.origin)
    assert.equal(authorize.searchParams.get('redirect_uri'), new URL(`/api/auth/callback/${providerId}`, looty).href)

    // Session Pocket ID ouverte et client sans consentement : Pocket ID renvoie tout de suite vers Looty.
    const granted = await get(authorize, authJar)
    assert.ok([302, 303].includes(granted.status), `authorize: HTTP ${granted.status}`)
    const callback = new URL(granted.headers.get('location'))
    assert.equal(callback.searchParams.get('error'), null, `Pocket ID refused: ${callback.searchParams.get('error_description')}`)
    assert.equal(callback.origin + callback.pathname, new URL(`/api/auth/callback/${providerId}`, looty).href)
    assert.ok(callback.searchParams.get('code'))

    const done = await get(callback, lootyJar)
    assert.ok([302, 303, 307].includes(done.status), `callback: HTTP ${done.status}`)
    assert.equal(done.headers.get('location'), '/', `Looty refused the sign-in: ${done.headers.get('location')}`)
  },

  async 'Looty: the first Pocket ID account is Looty administrator'() {
    const res = await get(new URL('/api/auth/me', looty), lootyJar)
    assert.equal(res.status, 200)
    const me = await res.json()
    assert.equal(me.authenticated, true)
    assert.equal(me.user.email, 'admin@example.com')
    assert.equal(me.user.is_super_admin, true)
  },
}

for (const [name, step] of Object.entries(steps)) {
  try {
    await step()
    console.log(`ok   ${name}`)
  } catch (e) {
    console.log(`FAIL ${name}: ${e.cause?.message ?? e.message}`)
    process.exit(1)
  }
}
