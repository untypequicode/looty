// Partage d'une page : dialogue (personne hors de l'espace, sous-pages, niveau, retrait), « Partagé avec moi »,
// page « demander l'accès » avec approbation depuis le dialogue.
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'
const { browser, page: alice, errors } = await launch(1400, 950)
await login(alice)
const a = (path, method, body) => call(alice, path, method, body)
await a(`/api/workspaces/${WS}/prefs`, 'PATCH', { modes: { docs: 'write', boards: 'write', calendar: 'write' }, modesChosen: true })

const person = async (email) => {
  const ctx = await browser.newContext({ viewport: { width: 1200, height: 900 } })
  const page = await ctx.newPage()
  await login(page, email)
  return { page, id: (await call(page, '/api/auth/me')).user.id, api: (p, m, b) => call(page, p, m, b) }
}
const carol = await person('carol-share@x.io')
const dave = await person('dave-share@x.io')

const top = await a(`/api/workspaces/${WS}/pages`, 'POST', { title: 'Parent secret partage', content: '<p>x</p>' })
const root = await a(`/api/workspaces/${WS}/pages`, 'POST', { title: 'Page partagée e2e', parent_id: top.id, content: '<p>x</p>' })
const child = await a(`/api/workspaces/${WS}/pages`, 'POST', { title: 'Sous-page e2e', parent_id: root.id, content: '<p>x</p>' })

// 1. Le dialogue : recherche par e-mail d'une personne hors de l'espace, ajout.
await alice.goto(`${B}/w/${WS}/doc/${root.id}`, { waitUntil: 'networkidle' })
await alice.waitForTimeout(1200)
await alice.getByRole('button', { name: /^(Share|Partager)$/ }).first().click()
const dlg = alice.locator('[role=dialog]')
await dlg.getByRole('combobox').fill('carol-share@x.io')
await alice.waitForTimeout(900)
const hit = dlg.getByRole('option').filter({ hasText: 'carol-share' }).first()
ok('dialogue: la personne est trouvée et marquée hors de l’espace', (await hit.count()) === 1 && /Outside the space|Hors de l’espace/.test(await hit.innerText()))
await hit.click()
await alice.waitForTimeout(900)
ok('dialogue: résumé « hors de l’espace »', /outside the space|hors de l’espace/.test(await dlg.locator('.summary').innerText()))
await alice.screenshot({ path: `${OUT}share-dialog.png` })

// 2. Elle voit la page et sa sous-page, pas le parent ; « Partagé avec moi » ne nomme pas le parent.
ok('invitée: page', (await carol.api(`/api/workspaces/${WS}/pages/${root.id}`)).id === root.id)
ok('invitée: sous-page', (await carol.api(`/api/workspaces/${WS}/pages/${child.id}`)).id === child.id)
ok('invitée: parent refusé', (await carol.api(`/api/workspaces/${WS}/pages/${top.id}`)).status === 404)
await carol.page.goto(`${B}/shared`, { waitUntil: 'networkidle' })
await carol.page.waitForTimeout(800)
const sharedText = await carol.page.locator('.sh').innerText()
ok('partagé avec moi: pages listées, parent absent', sharedText.includes('Page partagée e2e') && sharedText.includes('Sous-page e2e') && !sharedText.includes('Parent secret'))
ok('partagé avec moi: niveau visible', /Can comment|Peut commenter/.test(sharedText))

// 3. Portée « cette page seulement », changement de niveau, retrait (immédiats).
await a(`/api/workspaces/${WS}/pages/${root.id}/permissions`, 'PUT', { user_id: carol.id, role: 'comment', inherit: false })
ok('portée page seule: sous-page fermée', (await carol.api(`/api/workspaces/${WS}/pages/${child.id}`)).status === 404)
await a(`/api/workspaces/${WS}/pages/${root.id}/permissions`, 'PUT', { user_id: carol.id, role: 'edit', inherit: true })
ok('niveau modifier: la sous-page se rouvre', (await carol.api(`/api/workspaces/${WS}/pages/${child.id}`)).can_edit === true)
await carol.page.reload({ waitUntil: 'networkidle' })
await carol.page.waitForTimeout(600)
ok('partagé avec moi: niveau mis à jour', /Can edit|Peut modifier/.test(await carol.page.locator('.sh').innerText()))
await a(`/api/workspaces/${WS}/pages/${root.id}/permissions`, 'PUT', { user_id: carol.id, role: 'none' })
ok('retrait: page fermée', (await carol.api(`/api/workspaces/${WS}/pages/${root.id}`)).status === 404)

// 4. Lien sans droit : « demander l'accès », approbation depuis le dialogue.
await dave.page.goto(`${B}/w/${WS}/doc/${root.id}`, { waitUntil: 'networkidle' })
await dave.page.waitForTimeout(1500)
ok('lien sans droit: page « demander l’accès »', dave.page.url().includes(`/request-access/${root.id}`), dave.page.url())
await dave.page.getByRole('button', { name: /Request access|Demander l’accès/ }).click()
await dave.page.waitForTimeout(900)
ok('demande envoyée', /Request sent|Demande envoyée/.test(await dave.page.locator('main').innerText()))
await alice.reload({ waitUntil: 'networkidle' })
await alice.waitForTimeout(1000)
await alice.getByRole('button', { name: /^(Share|Partager)$/ }).first().click()
await alice.waitForTimeout(800)
await alice.locator('[role=dialog]').getByRole('button', { name: /Approve|Accepter/ }).first().click()
await alice.waitForTimeout(900)
ok('demande approuvée: la personne lit la page', (await dave.api(`/api/workspaces/${WS}/pages/${root.id}`)).id === root.id)

ok('aucune erreur JS', errors.length === 0, errors.join(' | '))
await browser.close()
