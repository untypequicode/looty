// Présentation partagée avec une personne extérieure et pages en lien public (ADR 0022, 0023) : lecture, modification
// et refus d'une présentation, images qui se chargent ou non, retrait en direct, lien public d'une présentation et
// d'une page avec leurs images (jeton du lien), et rien pour une personne sans lien ni droit.
import { B, OUT, ok } from '../lib/index.mjs'
import { start, until } from '../lib/access.mjs'

const t = await start('access slides', 1500, 950)
const { alice, ws, browser } = t
const carol = await t.person('carol-slides@x.io')
const dave = await t.person('dave-slides@x.io')

// Une présentation avec une image envoyée par l'interface (elle est citée dans l'état collaboratif de la présentation).
const deck = await t.mk('Présentation partagée', null, { default_view: 'slides', content: undefined })
await t.open(alice, deck.id)
await alice.waitForSelector('.deck .rail-item', { timeout: 20000 })
await alice.locator('input[type=file]').first().setInputFiles({ name: 'p.png', mimeType: 'image/png', buffer: Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==', 'base64') })
await alice.waitForSelector('.frame .sv-el[data-type="image"] img', { timeout: 15000 })
const imgUrl = await alice.locator('.frame .sv-el[data-type="image"] img').first().getAttribute('src')
const imgPath = new URL(imgUrl, B).pathname
ok('image insérée dans la présentation', imgPath.startsWith('/uploads/'), imgUrl)
await alice.waitForTimeout(4000)

// 1. Sans rien : ni la page ni l'image.
ok('sans partage: page refusée et image refusée', (await carol.read(deck.id)).status === 404 && (await carol.status(imgPath)) === 404)

// 2. Lecture : la présentation s'affiche avec son image, sans outils d'édition.
await t.share(deck.id, { user_id: carol.id, role: 'read', inherit: true })
await carol.page.goto(`${B}/shared/${deck.id}`, { waitUntil: 'networkidle' })
await carol.page.waitForSelector('.deck .rail-item', { timeout: 20000 })
const shownImage = async () => carol.page.evaluate(() => { const i = document.querySelector('.frame .sv-el[data-type="image"] img'); return !!i && i.complete && i.naturalWidth > 0 })
ok('lecture: la présentation affiche l’image', await until(shownImage, 60))
ok('lecture: pas d’outils d’édition', (await carol.page.locator('.bar .tb[title]').filter({ hasText: /Text|Texte/ }).count()) === 0)
await carol.page.screenshot({ path: `${OUT}access-slides-read.png` })

// 3. Modification en direct : les outils apparaissent sans recharger, puis disparaissent au retour en lecture.
await t.share(deck.id, { user_id: carol.id, role: 'edit', inherit: true })
ok('modifier: les outils d’édition apparaissent sans recharger', await until(async () => (await carol.page.locator('.bar .tb').filter({ hasText: /Text|Texte/ }).count()) >= 1, 80))
await t.share(deck.id, { user_id: carol.id, role: 'read', inherit: true })
ok('retour en lecture: les outils disparaissent sans recharger', await until(async () => (await carol.page.locator('.bar .tb').filter({ hasText: /Text|Texte/ }).count()) === 0, 80))

// 4. Refus : la présentation et son image sont coupées, pendant qu'elle est ouverte.
await t.share(deck.id, { user_id: carol.id, role: 'deny', inherit: true })
ok('refus: la présentation se ferme', await until(async () => carol.page.url().includes('/request-access/') || (await carol.page.locator('.deck').count()) === 0, 80))
ok('refus: page et image refusées', (await carol.read(deck.id)).status === 404 && (await carol.status(imgPath)) === 404 && !(await carol.imageLoads(imgPath)))

// 5. Lien public d'une présentation : visible sans compte, image chargée avec le jeton du lien ; sans le jeton, refusée.
const shared = await t.a(`/api/workspaces/${ws}/pages/${deck.id}/share`, 'POST')
const anon = await browser.newContext({ viewport: { width: 1300, height: 900 } })
const visitor = await anon.newPage()
const visitorStatus = (u) => visitor.evaluate(async (x) => (await fetch(x, { credentials: 'include' })).status, u)
await visitor.goto(`${B}/public/${shared.share_token}`, { waitUntil: 'networkidle' })
await visitor.waitForSelector('.sv-el[data-type="image"] img', { timeout: 20000 })
ok('lien public: l’image de la présentation se charge sans compte', await until(async () => visitor.evaluate(() => { const i = document.querySelector('.sv-el[data-type="image"] img'); return !!i && i.complete && i.naturalWidth > 0 }), 60))
ok('lien public: sans le jeton, l’image reste privée', (await visitorStatus(imgPath)) === 401)
await visitor.screenshot({ path: `${OUT}access-slides-public.png` })

// 6. Lien public d'une page : l'image du texte passe avec le jeton du lien ; couper le lien la coupe aussi.
const note = await t.mk('Page publique')
const noteImg = await t.upload(note.id)
await t.a(`/api/workspaces/${ws}/pages/${note.id}`, 'PUT', { content: `<p>texte-public</p><p><img src="${noteImg}"></p>` })
const pub = await t.a(`/api/workspaces/${ws}/pages/${note.id}/share`, 'POST')
await visitor.goto(`${B}/public/${pub.share_token}`, { waitUntil: 'networkidle' })
ok('page publique: le texte s’affiche sans compte', await until(async () => (await visitor.locator('body').innerText()).includes('texte-public'), 40))
ok('page publique: l’image se charge avec le jeton du lien', await until(async () => visitor.evaluate(() => { const i = document.querySelector('img[src*="/uploads/"]'); return !!i && i.complete && i.naturalWidth > 0 }), 60))
ok('page publique: l’adresse de l’image porte le jeton', (await visitor.locator('img[src*="/uploads/"]').first().getAttribute('src')).includes(`share=${pub.share_token}`))
ok('page publique: sans le jeton, l’image reste privée', (await visitorStatus(noteImg)) === 401 && (await dave.status(noteImg)) === 404)
ok('page publique: un jeton d’une autre page ne donne pas cette image', (await visitorStatus(`${noteImg}?share=${shared.share_token}`)) === 401)
await t.a(`/api/workspaces/${ws}/pages/${note.id}/share`, 'DELETE')
ok('lien coupé: la page et l’image ne se chargent plus', await until(async () => (await visitorStatus(`${noteImg}?share=${pub.share_token}`)) === 401 && (await visitorStatus(`/api/public/pages/${pub.share_token}`)) === 404, 40))
await anon.close()

await t.done()
