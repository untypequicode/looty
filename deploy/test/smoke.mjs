// Vérifie une instance de Looty déjà démarrée, à travers son proxy (routage, HTTPS, WebSockets).
//   NODE_EXTRA_CA_CERTS=<racine de l'AC> node deploy/test/smoke.mjs https://localhost [délai en s]
// Attend que l'instance réponde (par défaut 180 s), puis vérifie chaque route. Code de sortie non nul au premier échec.
import assert from 'node:assert/strict'

const base = new URL(process.argv[2] ?? 'https://localhost')
const waitSeconds = Number(process.argv[3] ?? 180)

async function waitForApi() {
  const deadline = Date.now() + waitSeconds * 1000
  let last
  while (Date.now() < deadline) {
    try {
      const res = await fetch(new URL('/api/auth/providers', base))
      if (res.ok) return
      last = `HTTP ${res.status}`
    } catch (e) {
      last = e.cause?.message ?? e.message
    }
    await new Promise((r) => setTimeout(r, 2000))
  }
  throw new Error(`${base} did not answer within ${waitSeconds} s (${last})`)
}

const checks = {
  async 'API (backend)'() {
    const res = await fetch(new URL('/api/auth/providers', base))
    assert.equal(res.status, 200)
    assert.match(res.headers.get('content-type') ?? '', /application\/json/)
  },

  async 'interface (frontend)'() {
    const res = await fetch(new URL('/', base))
    assert.equal(res.status, 200)
    assert.match(res.headers.get('content-type') ?? '', /text\/html/)
  },

  async 'CalDAV discovery'() {
    const wellKnown = await fetch(new URL('/.well-known/caldav', base), { redirect: 'manual' })
    assert.equal(wellKnown.status, 301)
    assert.equal(wellKnown.headers.get('location'), '/dav/')
    const dav = await fetch(new URL('/dav/', base), { method: 'PROPFIND', headers: { Depth: '0' } })
    assert.equal(dav.status, 401)
    assert.match(dav.headers.get('www-authenticate') ?? '', /Looty CalDAV/)
  },

  async 'collaboration over /collab'() {
    const res = await fetch(new URL('/collab', base))
    assert.equal(res.status, 200)
    assert.match(await res.text(), /Hocuspocus/)
  },

  async 'WebSocket to /collab'() {
    const url = new URL('/collab', base)
    url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:'
    await new Promise((resolve, reject) => {
      const ws = new WebSocket(url)
      const timer = setTimeout(() => reject(new Error('no WebSocket handshake within 10 s')), 10_000)
      ws.onopen = () => {
        clearTimeout(timer)
        ws.close()
        resolve()
      }
      ws.onerror = (e) => {
        clearTimeout(timer)
        reject(new Error(`WebSocket error: ${e.message ?? 'handshake refused'}`))
      }
    })
  },
}

await waitForApi()
let failed = false
for (const [name, check] of Object.entries(checks)) {
  try {
    await check()
    console.log(`ok   ${name}`)
  } catch (e) {
    failed = true
    console.log(`FAIL ${name}: ${e.message}`)
  }
}
process.exit(failed ? 1 : 0)
