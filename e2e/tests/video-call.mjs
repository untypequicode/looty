// Appel vidéo (ADR 0019, #265 et #266) : caméra simulée reçue par l'autre navigateur, partage d'écran (si le navigateur
// de test sait en simuler un), diagnostic de liaison, ICE restart sur changement de réseau et reprise après une coupure
// du WebSocket de signalisation, sans interrompre l'audio.
import { chromium } from 'playwright-core'
import { B, WS, ok, login, api, failed } from '../lib/index.mjs'

const browser = await chromium.launch({
  executablePath: process.env.CHROMIUM || undefined,
  args: [
    '--no-sandbox', '--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream', '--allow-loopback-in-peer-connection',
    '--disable-features=WebRtcHideLocalIpsWithMdns', '--auto-select-desktop-capture-source=Entire screen', '--enable-usermedia-screen-capturing',
  ],
})
const spy = () => {
  const Native = window.RTCPeerConnection
  window.__pcs = []
  window.RTCPeerConnection = function (...args) { const pc = new Native(...args); window.__pcs.push(pc); return pc }
  window.RTCPeerConnection.prototype = Native.prototype
}
const sockets = []
const open = async (email, watchSockets = false) => {
  const ctx = await browser.newContext({ viewport: { width: 1300, height: 900 }, permissions: ['microphone', 'camera'] })
  if (watchSockets) {
    // Relais transparent des WebSocket de collaboration, pour pouvoir en couper un.
    await ctx.routeWebSocket(/\/collab/, (ws) => {
      const server = ws.connectToServer()
      ws.onMessage((m) => server.send(m))
      server.onMessage((m) => ws.send(m))
      ws.onClose(() => server.close().catch(() => {}))
      server.onClose(() => ws.close().catch(() => {}))
      sockets.push({ ws, server })
    })
  }
  const p = await ctx.newPage()
  await p.addInitScript(spy)
  await login(p, email)
  return { ctx, p }
}
const rx = (en, fr) => new RegExp(`^(${en}|${fr})$`)

const a = await open('alice@x.io')
const b = await open('alice@x.io', true)
const page = await api(a.p, `/api/workspaces/${WS}/pages`, 'POST', { title: `Vidéo ${Date.now()}` })
const url = `${B}/w/${WS}/doc/${page.id}`
const bar = (p) => p.getByRole('region', { name: rx('Voice call', 'Appel vocal') })
const btn = (p, en, fr) => bar(p).getByRole('button', { name: rx(en, fr) })

await a.p.goto(url, { waitUntil: 'networkidle' })
await a.p.getByRole('button', { name: rx('Start call', 'Lancer un appel') }).click()
await bar(a.p).waitFor({ timeout: 15000 })
await b.p.goto(url, { waitUntil: 'networkidle' })
await b.p.getByRole('button', { name: rx('Join call', 'Rejoindre l’appel') }).click()
await bar(b.p).waitFor({ timeout: 15000 })

const stat = (p, kind, dir = 'inbound-rtp') => p.evaluate(async ([kind, dir]) => {
  let best = 0
  for (const pc of window.__pcs) {
    if (pc.connectionState !== 'connected') continue
    for (const s of (await pc.getStats()).values()) if (s.type === dir && s.kind === kind) best = Math.max(best, s.bytesReceived ?? s.bytesSent ?? 0)
  }
  return best
}, [kind, dir])
const grows = async (p, kind, min = 2000) => {
  for (let i = 0; i < 60; i++) { if ((await stat(p, kind)) > min) return true; await p.waitForTimeout(500) }
  return false
}
ok('audio A -> B avant la vidéo', await grows(b.p, 'audio'))

// --- Caméra ---
await btn(a.p, 'Turn camera on', 'Activer la caméra').click()
ok('A voit son aperçu', await a.p.waitForFunction(() => [...document.querySelectorAll('.stage video')].some((v) => v.videoWidth > 0), null, { timeout: 15000 }).then(() => true, () => false))
ok('B reçoit la vidéo de A (octets RTP)', await grows(b.p, 'video'))
ok('B affiche la vignette de A', await b.p.waitForFunction(() => [...document.querySelectorAll('.stage video')].some((v) => v.videoWidth > 0), null, { timeout: 15000 }).then(() => true, () => false))
const cam = await a.p.evaluate(() => {
  for (const pc of window.__pcs) for (const s of pc.getSenders()) if (s.track?.kind === 'video') return { cap: s.getParameters().encodings?.[0]?.maxBitrate ?? null, hint: s.track.contentHint, h: s.track.getSettings().height }
  return null
})
ok('caméra plafonnée et modeste', cam != null && (cam.cap == null || cam.cap <= 600_000) && (cam.h == null || cam.h <= 480), JSON.stringify(cam))
ok('l’audio continue pendant la vidéo', await grows(b.p, 'audio', 20000))
await btn(a.p, 'Turn camera off', 'Couper la caméra').click()
ok('B ne voit plus la vidéo de A', await b.p.waitForFunction(() => document.querySelectorAll('.stage video').length === 0, null, { timeout: 10000 }).then(() => true, () => false))

// --- Partage d'écran (selon le navigateur de test) ---
await btn(a.p, 'Share your screen, a window or a tab', 'Partager l’écran, une fenêtre ou un onglet').click()
const shared = await b.p.waitForFunction(() => [...document.querySelectorAll('.stage-share video')].some((v) => v.videoWidth > 0), null, { timeout: 15000 }).then(() => true, () => false)
if (shared) {
  ok('B reçoit le partage d’écran de A', true)
  ok('B voit qui partage', (await bar(b.p).getByText(/Sharing:|Partage en cours/).count()) > 0)
  const hint = await a.p.evaluate(() => { for (const pc of window.__pcs) for (const s of pc.getSenders()) if (s.track?.kind === 'video') return s.track.contentHint; return null })
  ok('contentHint « detail » pour le texte', hint === 'detail', String(hint))
  await bar(a.p).getByRole('button', { name: rx('Stop sharing', 'Arrêter le partage') }).first().click()
  ok('le partage s’arrête chez B', await b.p.waitForFunction(() => document.querySelectorAll('.stage-share').length === 0, null, { timeout: 10000 }).then(() => true, () => false))
} else console.log('INFO screen capture is not simulated by this browser build: share checks skipped')

// --- Diagnostic de liaison ---
await btn(b.p, 'Call settings', 'Réglages de l’appel').click()
await b.p.getByRole('button', { name: rx('Connection details', 'Détails de la connexion') }).click()
ok('diagnostic : latence mesurée', await b.p.waitForFunction(() => /\d+ ms/.test(document.querySelector('.cb-diag tbody')?.textContent ?? ''), null, { timeout: 15000 }).then(() => true, () => false))
ok('indicateur de qualité affiché', (await bar(b.p).getByRole('img', { name: /Connection quality|Qualité de connexion/ }).count()) === 1)

// --- ICE restart sur changement de réseau ---
const ufrag = (p) => p.evaluate(() => { for (const pc of window.__pcs) { const m = pc.localDescription?.sdp.match(/a=ice-ufrag:(\S+)/); if (m) return m[1] } return null })
const before = await ufrag(a.p)
await a.p.evaluate(() => window.dispatchEvent(new Event('online')))
let after = before
for (let i = 0; i < 40 && after === before; i++) { await a.p.waitForTimeout(500); after = await ufrag(a.p) }
ok('ICE restart après changement de réseau', before != null && after !== before, `${before} -> ${after}`)
const audioBefore = await stat(b.p, 'audio')
await a.p.waitForTimeout(2000)
ok('connexion rétablie, audio toujours reçu', (await a.p.evaluate(() => window.__pcs.some((pc) => pc.connectionState === 'connected'))) && (await stat(b.p, 'audio')) > audioBefore)

// --- Coupure du WebSocket de signalisation : l'audio continue et l'appel reprend ---
const audioAtDrop = await stat(b.p, 'audio')
for (const s of sockets.splice(0)) { await s.ws.close({ code: 1006 }).catch(() => {}); await s.server.close().catch(() => {}) }
await b.p.waitForTimeout(3000)
ok('audio inchangé pendant la coupure de signalisation', (await stat(b.p, 'audio')) > audioAtDrop)
ok('B toujours dans l’appel', await bar(b.p).isVisible())
// Une fois reconnecté, les messages repassent : B coupe son micro, A le voit.
await b.p.waitForFunction(() => !document.body.textContent.includes('Connection to the server lost') && !document.body.textContent.includes('Connexion au serveur perdue'), null, { timeout: 30000 }).catch(() => {})
await btn(b.p, 'Mute', 'Couper le micro').click()
await btn(a.p, 'Call settings', 'Réglages de l’appel').click()
ok('la signalisation a repris après la coupure', await a.p.locator('.cb-peer .cb-off').first().waitFor({ timeout: 30000 }).then(() => true, () => false))
ok('A et B toujours deux dans l’appel', (await a.p.locator('.cb-peer').count()) === 2)

// --- Fin d'appel : plus de piste ni de connexion ---
await bar(b.p).getByRole('button', { name: rx('Leave', 'Quitter') }).click()
ok('B quitte : ses connexions sont fermées', await b.p.evaluate(() => window.__pcs.every((pc) => pc.connectionState === 'closed')))
await bar(a.p).getByRole('button', { name: rx('Leave', 'Quitter') }).click()
ok('A quitte : ses connexions sont fermées', await a.p.evaluate(() => window.__pcs.every((pc) => pc.connectionState === 'closed')))

await browser.close()
process.exit(failed() ? 1 : 0)
