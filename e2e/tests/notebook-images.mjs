// Images des notebooks (ADR 0006) : une image collée dans une cellule Markdown devient un
// fichier du projet (images/), s'affiche, et se retrouve sur le lien public de la page.
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'

const { browser, page: p, errors } = await launch(1400, 950)
await login(p)
const api = (path, method, body) => call(p, path, method, body)

// Page portant un notebook, avec une cellule Markdown en tête.
const pg = await api(`/api/workspaces/${WS}/pages`, 'POST', { title: 'Analyse illustrée', project: { kind: 'notebook', template: 'blank' } })
ok('page with a notebook', !!pg.project_id && pg.project_kind === 'notebook', JSON.stringify(pg))
const pid = pg.project_id

await p.goto(`${B}/w/${WS}/projects/${pid}`, { waitUntil: 'networkidle' })
await p.locator('.nb .cell').first().waitFor({ timeout: 15000 })
await p.locator('.nb-bar').getByRole('button', { name: /^(Text|Texte)$/ }).click()
const md = p.locator('.nb .cell.markdown').first()
await md.locator('.cm-content').waitFor({ timeout: 5000 })
await p.keyboard.type('# Figure\n\n')

// --- 1. Coller une image (PNG 2×2 rouge) ---
const PNG = 'iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEklEQVR4nGP4z8DAwMDAxMDAAAAyDgMBhBHeRwAAAABJRU5ErkJggg=='
await md.locator('.cm-content').evaluate((el, b64) => {
  const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0))
  const dt = new DataTransfer()
  dt.items.add(new File([bytes], 'image.png', { type: 'image/png' }))
  el.dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true }))
}, PNG)
await md.locator('.cm-content').getByText(/!\[collage-\d{8}-\d{6}\]\(images\/collage-\d{8}-\d{6}\.png\)/).waitFor({ timeout: 10000 })
ok('link inserted at the cursor', true)

// --- 2. Fichier dans images/, image affichée ---
const proj = await api(`/api/projects/${pid}`)
const img = proj.files.find((f) => /^images\/collage-\d{8}-\d{6}\.png$/.test(f.path))
ok('image stored in images/', !!img, JSON.stringify(proj.files.map((f) => f.path)))
await p.locator('.nb-scroll').click({ position: { x: 5, y: 5 } })
await p.keyboard.press('Escape')
const shown = md.locator('.cell-md img')
await shown.waitFor({ timeout: 5000 })
await p.waitForFunction(() => { const i = document.querySelector('.nb .cell.markdown .cell-md img'); return i && i.complete && i.naturalWidth === 2 }, null, { timeout: 10000 })
ok('image rendered in the cell', (await shown.getAttribute('src')).includes(`/files/${img?.id}/raw`))
// Deuxième collage : nom distinct (-2) ou autre seconde, jamais d'écrasement.
await md.locator('.cell-md').dblclick()
await md.locator('.cm-content').waitFor({ timeout: 5000 })
await md.locator('.cm-content').evaluate((el, b64) => {
  const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0))
  const dt = new DataTransfer()
  dt.items.add(new File([bytes], 'figure.png', { type: 'image/png' }))
  // Sous la dernière ligne : inséré à la fin du texte.
  const r = el.getBoundingClientRect()
  el.dispatchEvent(new DragEvent('drop', { dataTransfer: dt, bubbles: true, cancelable: true, clientX: r.right - 4, clientY: r.bottom - 2 }))
}, PNG)
await md.locator('.cm-content').getByText('![figure](images/figure.png)').waitFor({ timeout: 10000 })
ok('dropped image keeps its name', true)
const after = await api(`/api/projects/${pid}`)
ok('dropped image in images/, nothing at the root', after.files.some((f) => f.path === 'images/figure.png') && !after.files.some((f) => f.path === 'figure.png'), JSON.stringify(after.files.map((f) => f.path)))
await p.screenshot({ path: OUT + 'notebook-images.png' })
await p.keyboard.press('Escape')
// Enregistrement du .ipynb par le serveur de collaboration.
await p.waitForTimeout(3500)

// --- 3. Publier la page ---
const shared = await api(`/api/workspaces/${WS}/pages/${pg.id}/share`, 'POST')
ok('page shared', !!shared.share_token && shared.is_public === true)

// --- 4. Lien public, navigation anonyme ---
const anon = await browser.newContext({ viewport: { width: 1300, height: 900 } })
const v = await anon.newPage()
v.on('pageerror', (e) => errors.push(`public: ${e.message}`))
const pub = await (await fetch(`${B}/api/public/pages/${shared.share_token}`)).json()
ok('public page exposes the notebook', pub.project?.kind === 'notebook' && pub.project?.main === 'notebook.ipynb' && pub.project?.notebooks?.includes('notebook.ipynb'), JSON.stringify(pub.project))
await v.goto(`${B}/public/${shared.share_token}`, { waitUntil: 'networkidle' })
await v.locator('.nbv .nbv-cell').first().waitFor({ timeout: 15000 })
ok('notebook tab open by default (empty page: no tabs)', (await v.locator('.rd-tabs').count()) === 0)
ok('title still shown', (await v.locator('.rd-title').textContent()).includes('Analyse illustrée'))
await v.waitForFunction(() => [...document.querySelectorAll('.nbv img')].some((i) => i.complete && i.naturalWidth === 2), null, { timeout: 10000 })
ok('image shown to an anonymous visitor', true)
ok('no run button on the public link', (await v.locator('.nbv .cell-run, .nbv [data-testid=nb-kernel]').count()) === 0)
await v.screenshot({ path: OUT + 'notebook-public.png' })

// Contrôles d'accès des routes publiques.
const get = (u) => fetch(`${B}${u}`).then((r) => r.status)
const t = shared.share_token
ok('public notebook route', (await get(`/api/public/pages/${t}/notebook?path=notebook.ipynb`)) === 200)
ok('public image route', (await get(`/api/public/pages/${t}/files/${img.path}`)) === 200)
ok('other files refused', (await get(`/api/public/pages/${t}/files/notebook.ipynb`)) === 404)
ok('wrong token refused', (await get(`/api/public/pages/00000000-0000-0000-0000-000000000000/files/${img.path}`)) === 404)
ok('no PDF for a notebook', (await get(`/api/public/pages/${t}/output.pdf`)) === 404)
await api(`/api/workspaces/${WS}/pages/${pg.id}/share`, 'DELETE')
ok('revoked link: image refused', (await get(`/api/public/pages/${t}/files/${img.path}`)) === 404)

// --- Page avec du texte : onglets « Page » / « Notebook », choix dans l'adresse ---
await api(`/api/workspaces/${WS}/pages/${pg.id}`, 'PUT', { content: '<p>Introduction du TP.</p>' })
const again = await api(`/api/workspaces/${WS}/pages/${pg.id}/share`, 'POST')
await v.goto(`${B}/public/${again.share_token}`, { waitUntil: 'networkidle' })
await v.locator('[data-testid=rd-tabs]').waitFor({ timeout: 10000 })
ok('tabs shown when the page has text', /Notebook/.test(await v.locator('[data-testid=rd-tabs]').textContent()))
ok('notebook tab open by default', (await v.locator('[data-testid=nb-viewer]').count()) === 1)
await v.locator('[data-testid=rd-tabs]').getByRole('tab', { name: 'Page' }).click()
await v.getByText('Introduction du TP.').waitFor({ timeout: 5000 })
ok('page tab in the hash', new URL(v.url()).hash === '#page')
await v.reload({ waitUntil: 'networkidle' })
await v.getByText('Introduction du TP.').waitFor({ timeout: 10000 })
ok('hash reopens the page tab', (await v.locator('[data-testid=nb-viewer]').count()) === 0)
const dl = await v.locator('[data-testid=rd-tabs]').getByRole('tab', { name: 'Notebook' }).click().then(() => v.locator('[data-testid=pv-download]').getAttribute('href'))
ok('download link serves the .ipynb as is', dl?.includes('download=1') && (await fetch(`${B}${dl}`).then((r) => r.headers.get('content-disposition') ?? '')).startsWith('attachment'), dl)

// --- Lecture des membres : même composant, routes authentifiées ---
await p.goto(`${B}/w/${WS}/read/${pg.id}`, { waitUntil: 'networkidle' })
await p.locator('[data-testid=nb-viewer]').waitFor({ timeout: 10000 })
await p.waitForFunction(() => [...document.querySelectorAll('[data-testid=nb-viewer] img')].some((i) => i.complete && i.naturalWidth === 2), null, { timeout: 10000 })
ok('member reader shows the notebook and its images', true)

// --- Projet LaTeX : onglet PDF, message sans PDF, jamais de compilation ---
const tex = await api(`/api/workspaces/${WS}/pages`, 'POST', { title: 'Rapport', project: { kind: 'latex', template: 'article' } })
const texShare = await api(`/api/workspaces/${WS}/pages/${tex.id}/share`, 'POST')
const texPub = await (await fetch(`${B}/api/public/pages/${texShare.share_token}`)).json()
ok('LaTeX page: no notebook exposed', texPub.project?.kind === 'latex' && texPub.project.notebooks.length === 0 && texPub.project.has_pdf === false, JSON.stringify(texPub.project))
const noPdf = await fetch(`${B}/api/public/pages/${texShare.share_token}/output.pdf`)
ok('no PDF yet: clear 404', noPdf.status === 404 && (await noPdf.text()).includes('latex_no_pdf'))
ok('no compile route on the public link', (await fetch(`${B}/api/public/pages/${texShare.share_token}/compile`, { method: 'POST' })).status >= 400)
ok('LaTeX sources not served', (await fetch(`${B}/api/public/pages/${texShare.share_token}/notebook?path=main.tex`)).status === 404)
await v.goto(`${B}/public/${texShare.share_token}`, { waitUntil: 'networkidle' })
await v.getByText(/No PDF yet|Pas encore de PDF/).waitFor({ timeout: 10000 })
ok('empty state without a PDF', true)
await v.screenshot({ path: OUT + 'latex-public.png' })

await anon.close()
ok('no page errors', errors.length === 0, errors.join(' | '))
await browser.close()
