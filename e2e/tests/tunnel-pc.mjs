// Tunnel PC (ADR 0020) : parcours complet avec un faux agent (lib/fake-agent.mjs). Demande LOOTY_TUNNELS=on côté backend
// et l'adresse directe du backend pour l'agent (E2E_BACKEND, défaut http://127.0.0.1:8080). Sans tunnels activés : ignoré.
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'
import { startFakeAgent } from '../lib/fake-agent.mjs'

const BACKEND = process.env.E2E_BACKEND || 'http://127.0.0.1:8080'
const { browser, page: p } = await launch(1400, 950)
await login(p)
const api = (path, method, body) => call(p, path, method, body)

const list = await api('/api/tunnels')
if (!list?.enabled) {
  console.log('SKIP tunnel-pc (LOOTY_TUNNELS is not on)')
  await browser.close()
  process.exit(0)
}

const nb = await api(`/api/workspaces/${WS}/projects`, 'POST', { kind: 'notebook', name: 'Calcul sur mon PC', template: 'blank' })
await p.goto(`${B}/w/${WS}/projects/${nb.id}`, { waitUntil: 'networkidle' })
await p.locator('.nb .cell').first().waitFor({ timeout: 15000 })
await p.locator('[data-testid=nb-engine]').click()
const dialog = p.locator('.dialog')
await dialog.getByText(/^(Jupyter server|Serveur Jupyter)$/).first().click()

// --- Assistant : avertissement et confirmation explicite ---
await dialog.locator('[data-testid=tunnel-use-pc]').click()
const create = dialog.locator('[data-testid=tunnel-create]')
ok('create is disabled until the warning is confirmed', await create.isDisabled())
ok('the warning says members can run code', /run code/i.test(await dialog.locator('.tb-warn').textContent()) || /exécuter du code/i.test(await dialog.locator('.tb-warn').textContent()))
await dialog.locator('[data-testid=tunnel-confirm]').check()
ok('create is enabled once confirmed', await create.isEnabled())
await create.click()

// --- Jeton montré une fois, lignes prêtes à copier ---
const tokenBox = dialog.locator('[data-testid=tunnel-token]')
await tokenBox.waitFor({ timeout: 10000 })
const token = (await tokenBox.textContent()).trim()
ok('token has the tunnel prefix', token.startsWith('lty_tun_'), token.slice(0, 12))
const nix = (await dialog.locator('[data-testid=tunnel-line]').textContent()).trim()
ok('nix line has the instance and the token filled in', nix.startsWith('nix run github:untypequicode/looty#bridge -- --instance ') && nix.includes(token) && nix.includes(new URL(B).origin) && nix.includes('--service jupyter=http://127.0.0.1:8888'), nix)
await dialog.getByRole('tab', { name: 'Docker Compose' }).click()
const compose = (await dialog.locator('[data-testid=tunnel-line]').textContent()).trim()
ok('compose line uses the environment variables', compose.includes('LOOTY_INSTANCE=') && compose.includes(`LOOTY_TOKEN=${token}`) && compose.includes('docker compose -f docker-compose.bridge.yml up'), compose)
ok('status is waiting for the agent', (await dialog.locator('[data-testid=tunnel-status]').getAttribute('data-phase')) === 'waiting')
await p.screenshot({ path: OUT + 'tunnel-launch.png' })

// --- Le faux agent se connecte : statut en direct, noyaux de la machine ---
const agent = await startFakeAgent({ backend: BACKEND, token, name: 'e2e-pc' })
await dialog.locator('[data-testid=tunnel-status][data-phase=connected]').waitFor({ timeout: 20000 })
ok('status shows connected with the machine name', (await dialog.locator('[data-testid=tunnel-status]').textContent()).includes('e2e-pc'))
await dialog.locator('[data-testid=tunnel-done]').click()
ok('token hidden after Done', (await dialog.locator('[data-testid=tunnel-token]').count()) === 0)
await dialog.locator('select').filter({ hasText: 'Fake Python 3' }).waitFor({ timeout: 15000 })
ok('kernels announced by the PC are listed', (await dialog.locator('select').filter({ hasText: 'Fake R' }).count()) > 0)
ok('the instance asked the agent for the kernelspecs', agent.seen.some((s) => s === 'GET /api/kernelspecs'), agent.seen.join(', '))

// --- Jamais le jeton ailleurs ---
const cfg = JSON.stringify(await api(`/api/projects/${nb.id}/jupyter`))
const mine = JSON.stringify(await api('/api/tunnels'))
ok('token never returned again', !cfg.includes(token) && !mine.includes(token))
ok('project config exposes the tunnel', JSON.parse(cfg).tunnel?.connected === true && JSON.parse(cfg).tunnel?.service === 'jupyter', cfg)

// --- Le tunnel tombe : message clair et repli ---
agent.close()
await dialog.locator('[data-testid=tunnel-status][data-phase=disconnected]').waitFor({ timeout: 25000 })
ok('dropped tunnel shows a relaunch hint', (await dialog.locator('[data-testid=tunnel-dropped]').count()) === 1)
const down = await p.evaluate(async (id) => { const r = await fetch(`/api/projects/${id}/jupyter/test`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: '{}' }); return { s: r.status, t: await r.text() } }, nb.id)
ok('backend answers jupyter_tunnel_offline', down.s === 400 && down.t.includes('jupyter_tunnel_offline'), down.t)
await p.screenshot({ path: OUT + 'tunnel-dropped.png' })

// --- Compte : liste des tunnels et révocation ---
await p.keyboard.press('Escape')
await p.goto(`${B}/w/${WS}/settings/tunnels`, { waitUntil: 'networkidle' })
await p.locator('[data-testid=tunnel-row]').first().waitFor({ timeout: 10000 })
ok('account settings list the tunnel', (await p.locator('[data-testid=tunnel-row]').count()) >= 1)
await p.locator('[data-testid=tunnel-revoke]').first().click()
await p.getByRole('button', { name: /^(Revoke|Révoquer)$/ }).last().click()
await p.waitForTimeout(800)
const after = await api('/api/tunnels')
ok('revoked tunnel leaves the list', !after.tunnels.some((t) => t.id === JSON.parse(cfg).tunnel.id))
const refused = await startFakeAgent({ backend: BACKEND, token }).then(() => false, () => true)
ok('a revoked token is refused', refused)

await browser.close()
