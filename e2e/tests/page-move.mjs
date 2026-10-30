// Déplacer une page d'un espace à l'autre (perso compris) : sous-pages, projet et
// commentaires suivent ; par le menu « Déplacer vers… » ou en glissant dans la barre latérale.
import { B, WS, OUT, ok, launch, login, api } from '../lib/index.mjs'

const N = Date.now() % 100000
const { browser, page: p, errors } = await launch(1440, 950)
await login(p)
const PID = (await api(p, '/api/workspaces/personal')).id
await api(p, `/api/workspaces/${PID}/prefs`, 'PATCH', { modes: { docs: 'edit', boards: 'edit', calendar: 'edit' }, modesChosen: true })
const pages = async (ws) => api(p, `/api/workspaces/${ws}/pages`)
const mk = (ws, title, parent_id = null) => api(p, `/api/workspaces/${ws}/pages`, 'POST', { title, parent_id })

// Une page, deux niveaux de sous-pages, un projet et un commentaire.
const a = await mk(WS, `Cours ${N}`)
const b = await mk(WS, `Chapitre ${N}`, a.id)
const c = await mk(WS, `Exercices ${N}`, b.id)
await api(p, `/api/workspaces/${WS}/pages/${b.id}/project`, 'POST', { kind: 'latex' })
await api(p, `/api/workspaces/${WS}/comments`, 'POST', { kind: 'page', id: a.id, content: 'À relire' })

// API : vers l'espace perso.
const r = await api(p, `/api/workspaces/${WS}/pages/${a.id}/move`, 'POST', { workspace_id: PID })
ok('déplacement : 3 pages', r?.moved === 3, JSON.stringify(r))
let mine = await pages(PID)
ok('arborescence gardée dans l’espace perso', mine.find((x) => x.id === a.id)?.parent_id == null && mine.find((x) => x.id === b.id)?.parent_id === a.id && mine.find((x) => x.id === c.id)?.parent_id === b.id)
ok('plus dans l’ancien espace', !(await pages(WS)).some((x) => [a.id, b.id, c.id].includes(x.id)))
ok('l’ancienne adresse ne répond plus', (await api(p, `/api/workspaces/${WS}/pages/${a.id}`))?.status === 404)
const projects = await api(p, `/api/workspaces/${PID}/projects`)
ok('le projet suit la page', JSON.stringify(projects).includes(b.id))
const comments = await api(p, `/api/workspaces/${PID}/comments?kind=page&id=${a.id}`)
ok('les commentaires suivent', JSON.stringify(comments).includes('À relire'), JSON.stringify(comments).slice(0, 200))
ok('même espace refusé', (await api(p, `/api/workspaces/${PID}/pages/${a.id}/move`, 'POST', { workspace_id: PID }))?.status === 400)
const other = await browser.newContext()
const bob = await other.newPage()
await login(bob, 'bob@x.io')
const bobSpace = (await api(bob, '/api/workspaces/personal')).id
await other.close()
ok('espace d’un autre compte refusé', (await api(p, `/api/workspaces/${PID}/pages/${a.id}/move`, 'POST', { workspace_id: bobSpace }))?.status === 404)

// API : retour sous une page de l'espace partagé.
const host = await mk(WS, `Classeur ${N}`)
await api(p, `/api/workspaces/${PID}/pages/${a.id}/move`, 'POST', { workspace_id: WS, parent_id: host.id })
ok('retour sous une page choisie', (await pages(WS)).find((x) => x.id === a.id)?.parent_id === host.id)

// Interface : menu « Déplacer vers… » depuis le document.
await p.goto(`${B}/w/${WS}/doc/${a.id}`, { waitUntil: 'networkidle' })
await p.locator('header').getByRole('button', { name: /^(Edit|Modifier)$/ }).first().click()
await p.locator('.doc-more').click()
await p.getByText(/^(Déplacer vers un autre espace…|Move to another workspace…)$/).click()
await p.locator('.mv-space', { hasText: /Espace perso|Personal space/ }).click()
await p.screenshot({ path: OUT + 'page-move-dialog.png' })
await p.getByRole('button', { name: /^(Déplacer|Move)$/ }).click()
await p.waitForURL(new RegExp(`/w/${PID}/doc/${a.id}`), { timeout: 8000 })
ok('menu : la page affichée suit son nouvel espace', p.url().includes(`/w/${PID}/doc/${a.id}`))
ok('menu : sous-pages déplacées', (await pages(PID)).filter((x) => [a.id, b.id, c.id].includes(x.id)).length === 3)

// Glisser : de l'arbre de l'espace partagé vers la section « Espace perso »…
const d = await mk(WS, `Brouillon ${N}`)
await p.goto(`${B}/w/${WS}`, { waitUntil: 'networkidle' })
const section = p.locator('.sb-personal')
await p.locator('.tree-node a', { hasText: `Brouillon ${N}` }).first().dragTo(section)
await p.waitForTimeout(1200)
ok('glisser vers l’espace perso', (await pages(PID)).some((x) => x.id === d.id))
ok('affiché dans la section perso', await section.getByText(`Brouillon ${N}`).isVisible())
// … puis de la section perso vers les documents de l'espace courant.
await section.locator('a', { hasText: `Brouillon ${N}` }).dragTo(p.locator('.sb-group.tree'), { targetPosition: { x: 100, y: 2 } })
await p.waitForTimeout(1200)
ok('glisser de l’espace perso vers l’espace courant', (await pages(WS)).some((x) => x.id === d.id))
await p.screenshot({ path: OUT + 'page-move-drag.png' })

ok('aucune erreur page', !errors.length, errors.join(' | '))
await browser.close()
