// Faux agent `looty-bridge` pour les tests e2e : parle le protocole de docs/tunnel-protocol.md (version 1) et joue un
// serveur Jupyter minimal (`/api`, `/api/kernelspecs`). Il ne lance aucun noyau.
import WebSocket from 'ws'

const SPECS = {
  default: 'python3',
  kernelspecs: {
    python3: { name: 'python3', spec: { display_name: 'Fake Python 3', language: 'python' } },
    ir: { name: 'ir', spec: { display_name: 'Fake R', language: 'R' } },
  },
}

/** Trame de données : id u32 big endian, drapeaux, charge utile. */
const frame = (id, payload, flags = 0) => {
  const b = Buffer.alloc(5 + payload.length)
  b.writeUInt32BE(id, 0)
  b[4] = flags
  payload.copy(b, 5)
  return b
}

/**
 * @param {{ backend: string, token: string, name?: string }} o `backend` : adresse du backend (http://127.0.0.1:8080).
 * @returns {Promise<{ close(): void, closed: Promise<number>, seen: string[] }>} résolu une fois `welcome` reçu.
 */
export function startFakeAgent({ backend, token, name = 'e2e-pc' }) {
  const url = backend.replace(/^http/, 'ws') + '/api/tunnels/agent'
  const ws = new WebSocket(url, 'looty-tunnel.v1', { headers: { Authorization: `Bearer ${token}` } })
  const seen = []
  const pending = new Map()
  const closed = new Promise((resolve) => ws.on('close', (code) => resolve(code)))
  const send = (o) => ws.send(JSON.stringify(o))
  ws.on('message', (data, isBinary) => {
    if (isBinary) return // corps de requête : rien à lire pour des GET
    const m = JSON.parse(data.toString())
    if (m.t === 'ping') return send({ t: 'pong', n: m.n })
    if (m.t === 'open') {
      seen.push(`${m.method} ${m.path}`)
      const path = m.path.split('?')[0]
      if (m.service !== 'jupyter' || m.proto !== 'http') return send({ t: 'error', id: m.id, code: 'unknown_service' })
      const body = path === '/api/kernelspecs' ? SPECS : path === '/api' ? { version: `fake-${name}` } : null
      pending.set(m.id, true)
      if (!body) {
        send({ t: 'head', id: m.id, status: 404, headers: [['content-type', 'application/json']] })
        return send({ t: 'end', id: m.id })
      }
      const payload = Buffer.from(JSON.stringify(body))
      send({ t: 'head', id: m.id, status: 200, headers: [['content-type', 'application/json']] })
      ws.send(frame(m.id, payload))
      send({ t: 'end', id: m.id })
    }
  })
  return new Promise((resolve, reject) => {
    ws.on('error', reject)
    ws.on('unexpected-response', (_req, res) => reject(new Error(`agent refused: HTTP ${res.statusCode}`)))
    ws.on('open', () => send({ t: 'hello', v: 1, agent: `looty-bridge/0.0.0-fake ${name}`, services: [{ name: 'jupyter', kind: 'jupyter' }] }))
    ws.on('message', (data, isBinary) => {
      if (!isBinary && JSON.parse(data.toString()).t === 'welcome') resolve({ close: () => ws.close(1000), closed, seen })
    })
  })
}
