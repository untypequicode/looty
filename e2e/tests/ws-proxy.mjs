// Les WebSockets /api/ws (temps réel) et /collab (édition collaborative) traversent l'interface seule, sans reverse proxy.
import { B, WS, ok, launch, login } from '../lib/index.mjs'
const { browser, page: p } = await launch()
await login(p)
const probe = (path, { waitMessage }) => p.evaluate(([url, waitMessage]) => new Promise((resolve) => {
  const ws = new WebSocket(url)
  let opened = false
  const done = (r) => { try { ws.close() } catch {} resolve(r) }
  ws.onopen = () => { opened = true; if (!waitMessage) setTimeout(() => done({ opened, stayed: true }), 1500) }
  ws.onmessage = (e) => done({ opened, message: String(e.data).slice(0, 60) })
  ws.onclose = () => done({ opened, stayed: false })
  setTimeout(() => done({ opened, timeout: true }), 8000)
}), [`${B.replace(/^http/, 'ws')}${path}`, waitMessage])

const rt = await probe(`/api/ws?workspace_id=${WS}`, { waitMessage: true })
ok('/api/ws relays the real-time feed', rt.opened && /presence/.test(rt.message || ''), JSON.stringify(rt))
if (process.env.E2E_COLLAB !== '0') {
  const co = await probe('/collab', { waitMessage: false })
  ok('/collab stays open through the relay', co.opened && co.stayed, JSON.stringify(co))
}
await browser.close()
