// Déplacer une page change qui la voit (ADR 0022) : l'aperçu annonce qui gagne ou perd l'accès avant de confirmer
// (dans l'espace, par glisser dans l'arbre) ; vers un autre espace, les partages individuels sont retirés, et on le dit.
import { B, OUT, ok } from '../lib/index.mjs'
import { start, until } from '../lib/access.mjs'

const t = await start('access move', 1500, 1000)
const { alice, ws } = t
const carol = await t.person('carol-move@x.io')
const shelf = await t.mk('Étagère partagée')
const loose = await t.mk('Page rangée plus tard')
const kid = await t.mk('Enfant de la rangée', loose.id)
await t.share(shelf.id, { user_id: carol.id, role: 'edit', inherit: true })
const sees = async (p) => (await carol.read(p.id)).id === p.id
const dialog = () => alice.locator('[role=dialog], [role=alertdialog]').last()

// 1. Aperçu du serveur : qui gagne l'accès si la page passe sous l'étagère, qui le perd si elle en sort.
const prev = await t.a(`/api/workspaces/${ws}/pages/${loose.id}/move-preview?parent_id=${shelf.id}`)
const gain = prev.changes.find((c) => c.name === 'carol-move')
ok('aperçu: Carol gagnerait l’accès (modifier)', !!gain && gain.before === 'none' && gain.after === 'edit', JSON.stringify(prev))
ok('aperçu: rien n’a bougé', !(await sees(loose)))
const none = await t.a(`/api/workspaces/${ws}/pages/${shelf.id}/move-preview`)
ok('aperçu: ne pas changer de parent ne change personne', none.changes.length === 0, JSON.stringify(none))

// 2. Interface : glisser la page sous l'étagère -> fenêtre d'avertissement -> annuler, puis confirmer.
await alice.goto(`${B}/w/${ws}`, { waitUntil: 'networkidle' })
await alice.waitForSelector('.tree-node a')
const drop = async (srcTitle, dstTitle) => {
  const dt = await alice.evaluateHandle(() => new DataTransfer())
  const src = alice.locator('.sb-section:not(.sb-personal) .tree-node a', { hasText: srcTitle }).first()
  const dst = alice.locator('.sb-section:not(.sb-personal) .tree-node a', { hasText: dstTitle }).first()
  await src.dispatchEvent('dragstart', { dataTransfer: dt })
  const box = await dst.evaluate((el) => { const r = el.getBoundingClientRect(); return { x: r.left + 20, y: r.top + r.height / 2 } })
  await dst.dispatchEvent('dragover', { dataTransfer: dt, clientX: box.x, clientY: box.y })
  await dst.dispatchEvent('drop', { dataTransfer: dt, clientX: box.x, clientY: box.y })
  await src.dispatchEvent('dragend', { dataTransfer: dt })
}
await drop('Page rangée plus tard', 'Étagère partagée')
await dialog().waitFor({ timeout: 8000 })
const msg = await dialog().innerText()
ok('glisser: la fenêtre annonce le changement d’accès', /Access will change|L’accès va changer/.test(msg) && /carol-move/.test(msg), msg.replace(/\n/g, ' | '))
await alice.screenshot({ path: `${OUT}access-move-preview.png` })
await alice.keyboard.press('Escape')
await alice.waitForTimeout(800)
ok('annuler: la page n’a pas bougé et Carol n’y accède pas', (await t.a(`/api/workspaces/${ws}/pages/${loose.id}`)).parent_id == null && !(await sees(loose)))
await drop('Page rangée plus tard', 'Étagère partagée')
await dialog().waitFor({ timeout: 8000 })
await dialog().getByRole('button', { name: /Move anyway|Déplacer quand même/ }).click()
ok('confirmer: la page est sous l’étagère', await until(async () => (await t.a(`/api/workspaces/${ws}/pages/${loose.id}`)).parent_id === shelf.id))
ok('confirmer: Carol accède à la page et à son enfant', await until(async () => (await sees(loose)) && (await sees(kid))))
ok('confirmer: au niveau de l’étagère', (await carol.level(loose.id)) === 'edit')

// Retour à la racine : Carol perd l'accès (l'aperçu le dit aussi).
const out = await t.a(`/api/workspaces/${ws}/pages/${loose.id}/move-preview`)
ok('aperçu: Carol perdrait l’accès', out.changes.some((c) => c.name === 'carol-move' && c.after === 'none'), JSON.stringify(out))
await t.a(`/api/workspaces/${ws}/pages/${loose.id}`, 'PUT', { parent_id: null })
ok('sortie de l’étagère: accès coupé pour la page et son enfant', await until(async () => !(await sees(loose)) && !(await sees(kid))))

// 3. Vers un autre espace : le partage individuel est retiré (et annoncé), Carol perd tout.
const other = (await t.a('/api/workspaces', 'POST', { name: `access move cible ${Date.now()}` })).workspace
await t.a(`/api/workspaces/${other.id}/prefs`, 'PATCH', { modes: { docs: 'write', boards: 'write', calendar: 'write' }, modesChosen: true })
ok('avant: Carol lit l’étagère', await sees(shelf))
await alice.goto(`${B}/w/${ws}/doc/${shelf.id}`, { waitUntil: 'networkidle' })
await alice.locator('.doc-more').click()
await alice.getByText(/^(Déplacer vers un autre espace…|Move to another workspace…)$/).click()
await alice.locator('.mv-space', { hasText: other.name }).click()
ok('autre espace: la fenêtre de déplacement dit que les partages sont retirés', /Individual shares are removed|partages individuels sont retirés/.test(await alice.locator('body').innerText()))
await alice.getByRole('button', { name: /^(Déplacer|Move)$/ }).click()
const confirm = alice.locator('[role=dialog], [role=alertdialog]', { hasText: /Sharing will be removed|Le partage sera retiré/ })
await confirm.waitFor({ timeout: 8000 })
ok('autre espace: une confirmation avertit avant de retirer les partages', true, (await confirm.innerText()).replace(/\n/g, ' | '))
await alice.screenshot({ path: `${OUT}access-move-space.png` })
await confirm.getByRole('button', { name: /Move anyway|Déplacer quand même/ }).click()
await alice.waitForURL(new RegExp(`/w/${other.id}/doc/${shelf.id}`), { timeout: 10000 })
ok('autre espace: la page a changé d’espace', (await t.a(`/api/workspaces/${other.id}/pages/${shelf.id}`)).id === shelf.id)
ok('autre espace: Carol n’a plus rien (ancienne et nouvelle adresse)', !(await sees(shelf)) && (await carol.api(`/api/workspaces/${other.id}/pages/${shelf.id}`)).status === 404)
ok('autre espace: ni « Partagé avec moi »', !((await carol.api('/api/shared')).some?.((p) => p.id === shelf.id)))

await t.done()
