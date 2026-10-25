// Projets dans les documents : une page porte un projet LaTeX (fichiers, éditeur, PDF) avec
// les droits de la page ; une page existante devient un projet ; l'accueil des projets les relie.
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'

const { browser, page: p, errors } = await launch(1440, 950)
await login(p)
const api = (path, method, body) => call(p, path, method, body)

// Page créée avec un projet LaTeX.
const pg = await api(`/api/workspaces/${WS}/pages`, 'POST', { title: 'Mémoire', project: { kind: 'latex', template: 'article' } })
ok('page créée avec un projet', !!pg.project_id && pg.project_kind === 'latex', JSON.stringify({ project_id: pg.project_id, kind: pg.project_kind }))
const proj = await api(`/api/projects/${pg.project_id}`)
ok('projet lié à la page', proj.page_id === pg.id && proj.files.some((f) => f.path === 'main.tex'))
const list = await api(`/api/workspaces/${WS}/pages`)
ok('liste des pages : type du projet', list.find((x) => x.id === pg.id)?.project_kind === 'latex')

// La page s'ouvre sur l'espace du projet ; « Notes » montre l'éditeur de la page.
await p.goto(`${B}/w/${WS}/doc/${pg.id}`, { waitUntil: 'networkidle' })
await p.waitForTimeout(1200)
ok('page : arborescence du projet', await p.locator('.doc-project .pj-tree').isVisible())
ok('page : main.tex dans les fichiers', (await p.locator('.doc-project .pj-tree').textContent()).includes('main.tex'))
ok('page : bouton Compiler', await p.locator('.doc-project .pj-compile').isVisible())
await p.screenshot({ path: OUT + 'page-project.png' })
await p.locator('.doc-project-head').getByText(/^(Notes)$/).click()
await p.waitForTimeout(800)
ok('notes : éditeur de la page', await p.locator('.ProseMirror').first().isVisible())
ok('notes : adresse ?view=notes', new URL(p.url()).searchParams.get('view') === 'notes')

// Une page ordinaire devient un projet (site).
const plain = await api(`/api/workspaces/${WS}/pages`, 'POST', { title: 'Vitrine' })
const conv = await api(`/api/workspaces/${WS}/pages/${plain.id}/project`, 'POST', { kind: 'site', template: 'starter' })
ok('page transformée en site', conv.project_kind === 'site' && !!conv.project_id)
const again = await api(`/api/workspaces/${WS}/pages/${plain.id}/project`, 'POST', { kind: 'latex' })
ok('une seule fois', again.status === 409)

// Projet sans page, rangé ensuite dans les documents.
const solo = await api(`/api/workspaces/${WS}/projects`, 'POST', { kind: 'notebook', name: 'Analyse', template: 'data' })
const attached = await api(`/api/projects/${solo.id}/page`, 'POST')
ok('projet rangé dans les documents', !!attached.page_id)
const projects = await api(`/api/workspaces/${WS}/projects`)
ok('accueil : projets avec leur page', ['Mémoire', 'Vitrine', 'Analyse'].every((n) => projects.find((x) => x.name === n)?.page_id), JSON.stringify(projects.map((x) => [x.name, !!x.page_id])))
ok('accueil : aperçu du fichier principal', (projects.find((x) => x.name === 'Mémoire')?.snippet ?? '').includes('\\documentclass'))

// Droits de la page : une personne bloquée sur la page ne voit pas le projet.
const bob = await launch()
await login(bob.page, 'bob-projects@x.io')
const bobId = (await call(bob.page, '/api/auth/me')).user.id
await api(`/api/workspaces/${WS}/access/members/add`, 'POST', { user_id: bobId })
ok('membre : projet visible', (await call(bob.page, `/api/projects/${pg.project_id}`)).project?.id === pg.project_id)
await api(`/api/workspaces/${WS}/pages/${pg.id}/permissions`, 'PUT', { user_id: bobId, role: 'blocked' })
ok('bloqué sur la page : projet introuvable', (await call(bob.page, `/api/projects/${pg.project_id}`)).status === 404)
ok('bloqué : absent de l’accueil des projets', !(await call(bob.page, `/api/workspaces/${WS}/projects`)).some((x) => x.id === pg.project_id))
await api(`/api/workspaces/${WS}/pages/${pg.id}/permissions`, 'PUT', { user_id: bobId, role: 'viewer' })
const ro = await call(bob.page, `/api/projects/${pg.project_id}`)
ok('lecteur de la page : projet en lecture seule', ro.project && ro.can_write === false)
await bob.browser.close()

// Accueil des projets (interface).
await p.goto(`${B}/w/${WS}/projects`, { waitUntil: 'networkidle' })
await p.waitForTimeout(600)
ok('accueil : galerie de modèles', (await p.locator('.pl-tpl').count()) >= 4)
ok('accueil : cartes', (await p.locator('.pl-card').count()) >= 3)
await p.screenshot({ path: OUT + 'projects-home.png' })
await p.locator('.pl-card', { hasText: 'Analyse' }).locator('.pl-card-link').click()
await p.waitForTimeout(800)
ok('carte : ouvre la page du projet', p.url().includes(`/doc/${attached.page_id}`), p.url())

// Suppression définitive de la page : le projet part avec elle.
await api(`/api/workspaces/${WS}/trash/settings`, 'PUT', { retention_days: 0 })
await api(`/api/workspaces/${WS}/pages/${plain.id}`, 'DELETE')
ok('page supprimée : projet supprimé', (await api(`/api/projects/${conv.project_id}`)).status === 404)
await api(`/api/workspaces/${WS}/trash/settings`, 'PUT', { retention_days: 30 })

ok('aucune erreur page', !errors.length, errors.join(' | '))
await browser.close()
