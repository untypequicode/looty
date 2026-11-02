// Notes Moodle : panneau « Notes » du tableau de bord et carnet de notes d'un cours.
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'
const { browser, page: p, errors } = await launch(1400, 950)
await login(p)
const api = (path, method, body) => call(p, path, method, body)

const grades = await api(`/api/workspaces/${WS}/moodle/grades`)
ok('grades api: overall grade per course', grades.find((g) => g.course_id === 1)?.grade === '14,50', JSON.stringify(grades))
const book = await api(`/api/workspaces/${WS}/moodle/courses/1/grades`)
ok('grade book: items and total', book.items.length === 2 && book.total?.grade === '14,50', JSON.stringify(book).slice(0, 200))

// Panneau « Notes » : une ligne par cours noté, clic → le cours.
await api(`/api/workspaces/${WS}/prefs`, 'PATCH', { moodleBoard: { panels: [{ i: 'g', kind: 'grades', x: 0, y: 0, w: 6, h: 10 }] } })
await p.goto(`${B}/w/${WS}/moodle`, { waitUntil: 'networkidle' }); await p.waitForTimeout(1500)
ok('grades panel lists graded courses only', (await p.locator('.gr').count()) === 1 && /14,50/.test(await p.locator('.gr').first().innerText()))
await p.screenshot({ path: OUT + 'grades-panel.png' })
await p.locator('.gr').first().click(); await p.waitForTimeout(1500)

// Carnet du cours : replié, puis déplié à la demande.
const head = p.locator('.cg-head')
ok('course shows a grades section', await head.count() === 1)
await head.click(); await p.waitForTimeout(1200)
ok('grade items listed', (await p.locator('.cg-item').count()) === 2)
ok('feedback shown', /Très bon travail/.test(await p.locator('.cg-body').innerText()))
ok('total shown in the header', /14,50 \/ 20/.test(await head.innerText()), await head.innerText())
await p.screenshot({ path: OUT + 'grades-course.png' })

await api(`/api/workspaces/${WS}/prefs`, 'PATCH', { moodleBoard: null })
ok('no page errors', errors.length === 0, errors.join(' | '))
await browser.close()
