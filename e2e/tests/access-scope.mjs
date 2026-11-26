// Portée et héritage du partage d'une page avec une personne hors de l'espace (ADR 0022) : « cette page seulement » ou
// « avec les sous-pages », réglage ou refus propre à une sous-page, partage à un groupe d'instance (et sortie du groupe),
// mention d'une personne extérieure qui a accès, et « Partagé avec moi » qui se met à jour sans recharger.
import { B, OUT, ok } from '../lib/index.mjs'
import { start, until } from '../lib/access.mjs'

const t = await start('access scope')
const { alice, ws } = t
const carol = await t.person('carol-scope@x.io')
const dave = await t.person('dave-scope@x.io')

const top = await t.mk('Parent secret')
const root = await t.mk('Racine partagée', top.id)
const s1 = await t.mk('Sous-page un', root.id)
const deep = await t.mk('Sous-sous-page', s1.id)
const s2 = await t.mk('Sous-page deux', root.id)
const sees = async (who, p) => (await who.read(p.id)).id === p.id
const lvl = async (who, p) => (await who.api(`/api/pages/${p.id}/access`)).level ?? null
const seen = async (who, ...pages) => (await Promise.all(pages.map((p) => sees(who, p)))).map((v) => (v ? 1 : 0)).join('')

// 1. « Cette page seulement » puis « avec les sous-pages ».
await t.share(root.id, { user_id: carol.id, role: 'read', inherit: false })
ok('page seule: la page, rien d’autre (parent, sous-pages)', (await seen(carol, top, root, s1, deep, s2)) === '01000')
await t.share(root.id, { user_id: carol.id, role: 'read', inherit: true })
ok('avec sous-pages: page et sous-pages, pas le parent', (await seen(carol, top, root, s1, deep, s2)) === '01111')

// 2. Réglage propre à une sous-page : plus de droits sur l'une, refus sur l'autre ; le reste suit la racine.
await t.share(s1.id, { user_id: carol.id, role: 'edit', inherit: true })
await t.share(s2.id, { user_id: carol.id, role: 'deny', inherit: true })
ok('sous-page: niveau propre qui s’étend à ses sous-pages', (await lvl(carol, s1)) === 'edit' && (await lvl(carol, deep)) === 'edit' && (await lvl(carol, root)) === 'read')
ok('sous-page: refus, la racine reste lisible', (await seen(carol, root, s1, deep, s2)) === '1110')
await t.share(s2.id, { user_id: carol.id, role: 'none' })
ok('sous-page: retirer le refus rend l’héritage', (await sees(carol, s2)) && (await lvl(carol, s2)) === 'read')
// L'interface de Carol : « Partagé avec moi » n'affiche ni le parent ni le nom d'un refus.
await t.share(s2.id, { user_id: carol.id, role: 'deny', inherit: true })
await carol.page.goto(`${B}/shared`, { waitUntil: 'networkidle' })
await carol.page.waitForSelector('.sh')
const text = await carol.page.locator('.sh').innerText()
ok('partagé avec moi: racine et sous-pages, ni parent ni page refusée', text.includes('Racine partagée') && text.includes('Sous-page un') && !text.includes('Parent secret') && !text.includes('Sous-page deux'), text.replace(/\n/g, ' | '))

// 3. Groupe d'instance : le partage suit l'appartenance au groupe.
const isAdmin = (await t.a('/api/auth/me')).user.is_super_admin
ok('Alice administre l’instance', isAdmin === true)
if (isAdmin) {
  const g = await t.a('/api/admin/share/groups', 'POST', { name: `Groupe e2e ${Date.now()}` })
  await t.a(`/api/admin/share/groups/${g.id}/members`, 'POST', { user_id: dave.id })
  ok('groupe: avant le partage, aucun accès', (await seen(dave, root)) === '0')
  await t.share(root.id, { group_id: g.id, role: 'comment', inherit: true })
  ok('groupe: la personne du groupe lit la page et sa sous-page', (await seen(dave, root, s1)) === '11')
  ok('groupe: niveau donné par le groupe', (await lvl(dave, root)) === 'comment')
  await t.a(`/api/admin/share/groups/${g.id}/members/${dave.id}`, 'DELETE')
  ok('groupe: sortie du groupe, accès coupé tout de suite', await until(async () => (await seen(dave, root, s1)) === '00'))
  await t.a(`/api/admin/share/groups/${g.id}/members`, 'POST', { user_id: dave.id })
  ok('groupe: de retour dans le groupe, accès rendu', await until(async () => (await seen(dave, root)) === '1'))
  await t.a(`/api/admin/share/groups/${g.id}`, 'DELETE')
  ok('groupe supprimé: accès coupé', await until(async () => (await seen(dave, root)) === '0'))
}

// 4. Mention d'une personne extérieure qui a accès : proposée, prévenue ; sans accès : ni l'un ni l'autre.
const men = await t.a(`/api/workspaces/${ws}/pages/${s1.id}/mentionable`)
ok('mention: Carol (extérieure, avec accès) est proposée', men.some((p) => p.user_id === carol.id && p.is_member === false), JSON.stringify(men).slice(0, 200))
ok('mention: Dave (sans accès) n’est pas proposé', !men.some((p) => p.user_id === dave.id))
const mention = (id) => `<p><span data-type="mention" data-id="${id}">@x</span> regarde ceci</p>`
await t.a(`/api/workspaces/${ws}/comments`, 'POST', { kind: 'page', id: s1.id, content: mention(carol.id) })
await t.a(`/api/workspaces/${ws}/comments`, 'POST', { kind: 'page', id: s1.id, content: mention(dave.id) })
const notes = (who) => who.api(`/api/notifications?workspace_id=${ws}`)
ok('mention: Carol reçoit la notification', await until(async () => (await notes(carol)).some?.((n) => n.kind === 'mention')))
ok('mention: Dave n’en reçoit aucune', !((await notes(dave)).some?.((n) => n.kind === 'mention')))

// 5. En direct, pendant qu'elle lit une page partagée (arbre de la barre latérale, « Partagé avec moi », notification) :
//    une nouvelle page partagée apparaît, son niveau change, puis elle disparaît, sans recharger.
const fresh = await t.mk('Nouveauté en direct')
await carol.page.goto(`${B}/shared/${root.id}`, { waitUntil: 'networkidle' })
await carol.page.waitForSelector('nav.sidebar')
await carol.page.waitForTimeout(1500)
const side = async () => carol.page.locator('nav.sidebar').innerText()
const sharedList = () => carol.api('/api/shared')
const inShared = async () => (await sharedList()).some?.((p) => p.id === fresh.id)
ok('en direct: la page n’est pas encore dans l’arbre', !(await side()).includes('Nouveauté en direct') && !(await inShared()))
await t.share(fresh.id, { user_id: carol.id, role: 'read', inherit: false })
ok('en direct: la page partagée apparaît dans l’arbre sans recharger', await until(async () => (await side()).includes('Nouveauté en direct'), 80))
ok('en direct: « Partagé avec moi » la liste', await inShared())
ok('en direct: une notification prévient', await until(async () => /shared a page with you|a partagé une page avec vous/.test(await carol.page.locator('body').innerText()), 40))
await t.share(fresh.id, { user_id: carol.id, role: 'edit', inherit: false })
ok('en direct: le niveau est mis à jour', await until(async () => (await carol.api(`/api/shared`)).find?.((p) => p.id === fresh.id)?.level === 'edit', 40))
await carol.page.screenshot({ path: `${OUT}access-scope-live.png` })
await t.share(fresh.id, { user_id: carol.id, role: 'none' })
ok('en direct: la page retirée disparaît de l’arbre sans recharger', await until(async () => !(await side()).includes('Nouveauté en direct'), 80))
ok('en direct: et de « Partagé avec moi »', !(await inShared()))

await t.done()
