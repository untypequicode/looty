// Accès aux pages en direct (ADR 0022) : changer le niveau d'une personne qui a la page ouverte (modifier -> lire) rend sa
// session de collaboration en lecture seule sans la couper, un refus la coupe, et retirer l'accès pendant qu'elle regarde
// ferme la page, sa sous-page et son image.
import { B, OUT, ok } from '../lib/index.mjs'
import { start, until } from '../lib/access.mjs'

const t = await start('access live')
const { alice, ws } = t
const carol = await t.person('carol-live@x.io')
const dave = await t.person('dave-live@x.io')

// 1. Session de collaboration ouverte : modifier -> lire (sans coupure) -> refus (coupure).
const doc = await t.mk('Live page')
const sub = await t.mk('Live sub', doc.id)
await t.share(doc.id, { user_id: carol.id, role: 'edit', inherit: true })
const sockets = []
carol.page.on('websocket', (s) => { const rec = { url: s.url(), closed: false }; s.on('close', () => { rec.closed = true }); sockets.push(rec) })
await t.open(alice, doc.id)
await t.open(carol.page, doc.id)
const pm = (p) => p.locator('.ProseMirror').first()
await pm(carol.page).waitFor({ timeout: 20000 })
await pm(alice).waitFor({ timeout: 20000 })
ok('invitée en modification: éditeur actif', await until(async () => (await pm(carol.page).getAttribute('contenteditable')) === 'true'))
await pm(carol.page).click()
await carol.page.keyboard.type(' bonjour-de-carol')
ok('invitée en modification: son texte arrive en direct chez Alice', await until(async () => (await pm(alice).innerText()).includes('bonjour-de-carol')))
const collab = () => sockets.filter((s) => /\/collab/.test(s.url))
ok('session de collaboration ouverte', collab().length >= 1, JSON.stringify(sockets))
await t.share(doc.id, { user_id: carol.id, role: 'read', inherit: true })
ok('modifier -> lire: l’éditeur passe en lecture seule sans recharger', await until(async () => (await pm(carol.page).getAttribute('contenteditable')) === 'false', 80))
ok('modifier -> lire: la page reste affichée', carol.page.url().includes(`/doc/${doc.id}`) && (await pm(carol.page).innerText()).includes('bonjour-de-carol'))
ok('modifier -> lire: la connexion n’est pas coupée', collab().length >= 1 && collab().every((s) => !s.closed))
ok('modifier -> lire: le serveur de collaboration le confirme', (await carol.level(doc.id)) === 'read')
await carol.page.screenshot({ path: `${OUT}access-live-read.png` })
await t.share(doc.id, { user_id: carol.id, role: 'deny', inherit: true })
ok('refus: la connexion en direct est fermée', await until(async () => collab().some((s) => s.closed), 80))
ok('refus: la page est fermée', (await carol.read(doc.id)).status === 404 && (await carol.read(sub.id)).status === 404)

// 2. Accès retiré pendant que la personne regarde : page, sous-page et image coupées.
const shown = await t.mk('Regardée')
const shownSub = await t.mk('Regardée sous-page', shown.id)
const img = await t.upload(shown.id)
await t.a(`/api/workspaces/${ws}/pages/${shown.id}`, 'PUT', { content: `<p>visible-par-dave</p><p><img src="${img}"></p>` })
await t.share(shown.id, { user_id: dave.id, role: 'read', inherit: true })
await t.open(dave.page, shown.id)
ok('lecture seule: la page s’affiche avec son image', await until(async () => (await dave.page.locator('main').innerText()).includes('visible-par-dave')) && (await dave.imageLoads(img)))
ok('lecture seule: éditeur non modifiable', (await pm(dave.page).getAttribute('contenteditable')) === 'false')
ok('lecture seule: sous-page lisible', (await dave.read(shownSub.id)).id === shownSub.id)
await t.share(shown.id, { user_id: dave.id, role: 'none' })
ok('retrait: la page affichée se ferme (demande d’accès)', await until(async () => dave.page.url().includes('/request-access/') || !(await dave.page.locator('main').innerText()).includes('visible-par-dave'), 80))
ok('retrait: page refusée', (await dave.read(shown.id)).status === 404)
ok('retrait: sous-page refusée', (await dave.read(shownSub.id)).status === 404)
ok('retrait: image refusée', (await dave.status(img)) === 404 && !(await dave.imageLoads(img)))
await dave.page.screenshot({ path: `${OUT}access-live-revoked.png` })

await t.done()
