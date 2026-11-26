// Demande d'accès de bout en bout (ADR 0022) : lien d'une page sans droit -> « demander l'accès » -> notification chez
// les gestionnaires -> accepter ou refuser depuis la boîte de réception -> notification et accès (ou refus) chez la
// personne qui demandait.
import { B, OUT, ok } from '../lib/index.mjs'
import { start, until } from '../lib/access.mjs'

const t = await start('access request')
const { alice, ws } = t
const carol = await t.person('carol-request@x.io')
const dave = await t.person('dave-request@x.io')
const page = await t.mk('Page demandée')
const sub = await t.mk('Sous-page demandée', page.id)
const notes = (who) => (who === alice ? t.a : who.api)(`/api/notifications?workspace_id=${ws}`)
const ask = async (who, message) => {
  await who.page.goto(`${B}/w/${ws}/doc/${page.id}`, { waitUntil: 'networkidle' })
  await until(async () => who.page.url().includes('/request-access/'), 40)
  await who.page.locator('#ra-msg').fill(message)
  await who.page.getByRole('button', { name: /Request access|Demander l’accès/ }).click()
  return until(async () => /Request sent|Demande envoyée/.test(await who.page.locator('main').innerText()), 40)
}
const row = (name) => alice.locator('.n-wrap', { hasText: name })
const inbox = async () => {
  await alice.goto(`${B}/w/${ws}/inbox`, { waitUntil: 'networkidle' })
  await alice.waitForSelector('.n-wrap', { timeout: 15000 })
}

ok('sans droit: la page est introuvable', (await carol.read(page.id)).status === 404)
ok('Carol: demande envoyée depuis la page « demander l’accès »', await ask(carol, 'Je suis Carol, du groupe de lecture'))
ok('Dave: demande envoyée', await ask(dave, 'Dave ici'))
ok('une demande en attente reste sans accès', (await carol.read(page.id)).status === 404 && (await dave.read(sub.id)).status === 404)
ok('Alice est prévenue de chaque demande (notification)', await until(async () => (await notes(alice)).filter?.((n) => n.kind === 'access_request').length === 2))

// Boîte de réception d'Alice : le message, accepter Carol, refuser Dave.
await inbox()
ok('boîte de réception: la demande de Carol est là avec ses boutons', (await row('carol').getByRole('button', { name: /Approve|Accepter/ }).count()) === 1)
await alice.screenshot({ path: `${OUT}access-request-inbox.png` })
await row('carol').getByRole('button', { name: /Approve|Accepter/ }).click()
ok('Carol accepte: Carol lit la page et sa sous-page', await until(async () => (await carol.read(page.id)).id === page.id && (await carol.read(sub.id)).id === sub.id))
ok('Carol accepte: niveau lecture', (await carol.level(page.id)) === 'read')
ok('Carol accepte: elle est prévenue', await until(async () => (await notes(carol)).some?.((n) => n.kind === 'share' && n.target === `page:${page.id}`)))
await row('dave').getByRole('button', { name: /Decline|Refuser/ }).click()
ok('Dave refusé: toujours sans accès', await until(async () => (await t.a(`/api/workspaces/${ws}/pages/${page.id}/access-requests`)).length === 0) && (await dave.read(page.id)).status === 404)
ok('Alice: les demandes traitées sont marquées lues', await until(async () => (await notes(alice)).filter?.((n) => n.kind === 'access_request').every((n) => n.read_at)))

// Ce que voient les deux personnes en rouvrant le lien.
await dave.page.goto(`${B}/request-access/${page.id}`, { waitUntil: 'networkidle' })
ok('Dave: la page dit que la demande est refusée', await until(async () => /Request declined|Demande refusée/.test(await dave.page.locator('main').innerText()), 40), await dave.page.locator('main').innerText())
ok('Dave: pas de nouveau bouton pour insister', (await dave.page.getByRole('button', { name: /Request access|Demander l’accès/ }).count()) === 0)
await dave.page.screenshot({ path: `${OUT}access-request-declined.png` })
await carol.page.goto(`${B}/w/${ws}/doc/${page.id}`, { waitUntil: 'networkidle' })
ok('Carol: le lien ouvre la page, sans écran de demande', await until(async () => carol.page.url().includes(`/doc/${page.id}`) && (await carol.page.locator('main').innerText()).includes('Page demandée'), 40))
await carol.page.goto(`${B}/request-access/${page.id}`, { waitUntil: 'networkidle' })
ok('Carol: « vous avez accès » avec le lien pour ouvrir', await until(async () => (await carol.page.getByRole('link', { name: /Open the page|Ouvrir la page/ }).count()) === 1, 40))

await t.done()
