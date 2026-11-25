// Appel vocal (ADR 0019) : deux navigateurs avec faux micro rejoignent l'appel d'une page, chacun reçoit
// de l'audio de l'autre (octets RTP entrants) ; la latence aller-retour du chemin média est reportée.
// Droits : une personne sans accès à la page est refusée par la route d'accès de la salle.
import { chromium } from 'playwright-core'
import { B, WS, ok, login, api, failed } from '../lib/index.mjs'

const browser = await chromium.launch({
  executablePath: process.env.CHROMIUM || undefined,
  args: ['--no-sandbox', '--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream', '--allow-loopback-in-peer-connection', '--disable-features=WebRtcHideLocalIpsWithMdns'],
})
const spy = () => {
  // Garde les connexions WebRTC de la page pour lire leurs statistiques.
  const Native = window.RTCPeerConnection
  window.__pcs = []
  window.RTCPeerConnection = function (...args) { const pc = new Native(...args); window.__pcs.push(pc); return pc }
  window.RTCPeerConnection.prototype = Native.prototype
}
const open = async (email) => {
  const ctx = await browser.newContext({ viewport: { width: 1300, height: 900 }, permissions: ['microphone'] })
  const p = await ctx.newPage()
  await p.addInitScript(spy)
  await login(p, email)
  return { ctx, p }
}

const a = await open('alice@x.io')
const page = await api(a.p, `/api/workspaces/${WS}/pages`, 'POST', { title: `Appel ${Date.now()}` })

// Droits : la route d'accès de la salle suit ceux de la page.
ok('accès à la salle : membre', (await api(a.p, `/api/calls/page/${page.id}/access`))?.user_id != null)
ok('accès à la salle : page inconnue', (await api(a.p, '/api/calls/page/00000000-0000-4000-8000-000000000000/access'))?.status === 404)
ok('accès à la salle : type inconnu', (await api(a.p, `/api/calls/other/${page.id}/access`))?.status === 404)
const cfg = await api(a.p, '/api/calls/config')
ok('configuration : STUN et limite', cfg?.ice_servers?.[0]?.urls?.[0]?.startsWith('stun:') && cfg.max_participants >= 2, JSON.stringify(cfg))
// Une page d'un espace où Alice n'est pas membre : sa salle lui est refusée.
const bob = await open('bob@x.io')
const bobWs = (await api(bob.p, '/api/workspaces', 'POST', { name: `appel ${Date.now()}` }))?.workspace
const bobPage = await api(bob.p, `/api/workspaces/${bobWs?.id}/pages`, 'POST', { title: 'Privé' })
ok('accès à la salle : propriétaire', (await api(bob.p, `/api/calls/page/${bobPage?.id}/access`))?.user_id != null)
const denied = await api(a.p, `/api/calls/page/${bobPage?.id}/access`)
ok('accès à la salle : personne sans accès refusée', [403, 404].includes(denied?.status), JSON.stringify(denied))
await bob.ctx.close()

const b = await open('alice@x.io')
const url = `${B}/w/${WS}/doc/${page.id}`
const start = (p) => p.getByRole('button', { name: /^(Start call|Lancer un appel|Join call|Rejoindre l’appel)$/ })
const bar = (p) => p.getByRole('region', { name: /^(Voice call|Appel vocal)$/ })

await a.p.goto(url, { waitUntil: 'networkidle' })
await start(a.p).click()
await bar(a.p).waitFor({ timeout: 15000 })
ok('barre d’appel affichée', true)

await b.p.goto(url, { waitUntil: 'networkidle' })
// Pastille : B voit qu'un appel est en cours avant de le rejoindre.
await b.p.getByRole('button', { name: /^(Join call|Rejoindre l’appel)$/ }).waitFor({ timeout: 15000 })
ok('présence visible avant de rejoindre', true)
await start(b.p).click()
await bar(b.p).waitFor({ timeout: 15000 })

// Audio reçu par chacun (octets RTP entrants, connexion établie). Interrogation manuelle : waitForFunction
// n'attend pas les fonctions asynchrones.
const audioBytes = (p) => p.evaluate(async () => {
  let best = 0
  for (const pc of window.__pcs) {
    if (pc.connectionState !== 'connected') continue
    for (const s of (await pc.getStats()).values()) if (s.type === 'inbound-rtp' && s.kind === 'audio') best = Math.max(best, s.bytesReceived)
  }
  return best
})
const received = async (p) => {
  for (let i = 0; i < 60; i++) { if ((await audioBytes(p)) > 2000) return true; await p.waitForTimeout(500) }
  return false
}
ok('A reçoit l’audio de B', await received(a.p))
ok('B reçoit l’audio de A', await received(b.p))

// Latence du chemin média (RTT de la paire de candidats retenue, sinon celui des rapports RTCP), reportée ici et dans la doc.
const readRtt = () => a.p.evaluate(async () => {
  for (const pc of window.__pcs) for (const s of (await pc.getStats()).values()) {
    if (s.type === 'candidate-pair' && s.state === 'succeeded' && typeof s.currentRoundTripTime === 'number') return s.currentRoundTripTime * 1000
    if (s.type === 'remote-inbound-rtp' && typeof s.roundTripTime === 'number') return s.roundTripTime * 1000
  }
  return null
})
let rtt = null
for (let i = 0; i < 40 && rtt == null; i++) { rtt = await readRtt(); if (rtt == null) await a.p.waitForTimeout(500) }
console.log(`INFO round-trip time on the media path: ${rtt == null ? 'n/a' : rtt.toFixed(1) + ' ms'}`)
ok('latence aller-retour sous 150 ms', rtt != null && rtt < 150, String(rtt))

// Chaque barre voit l'autre participant ; la sourdine est partagée.
await a.p.getByRole('button', { name: /^(Mute|Couper le micro)$/ }).click()
ok('micro coupé (état local)', (await a.p.getByRole('button', { name: /^(Unmute|Réactiver le micro)$/ }).getAttribute('aria-pressed')) === 'true')
await b.p.getByRole('button', { name: /^(Call settings|Réglages de l’appel)$/ }).click()
await b.p.locator('.cb-peer .cb-off').first().waitFor({ timeout: 10000 }).then(() => ok('B voit A en sourdine', true), () => ok('B voit A en sourdine', false))

// L'appel survit à un changement de page.
const before = await b.p.evaluate(() => performance.timeOrigin)
await b.p.locator(`a[href="/w/${WS}/docs"]`).first().click()
await b.p.waitForURL(/\/docs$/)
ok('navigation sans rechargement', (await b.p.evaluate(() => performance.timeOrigin)) === before)
ok('la barre reste pendant la navigation', await bar(b.p).isVisible())

// Quitter : A reste seul.
await bar(b.p).getByRole('button', { name: /^(Leave|Quitter)$/ }).click()
await a.p.getByText(/Waiting for others|En attente d’autres/).waitFor({ timeout: 15000 }).then(() => ok('B quitte : A est seul', true), () => ok('B quitte : A est seul', false))
await bar(a.p).getByRole('button', { name: /^(Leave|Quitter)$/ }).click()
ok('appel terminé', await bar(a.p).isHidden())

await browser.close()
process.exit(failed() ? 1 : 0)
