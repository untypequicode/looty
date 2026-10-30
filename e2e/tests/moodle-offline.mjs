// Moodle hors ligne : ce qui a déjà été ouvert (liste des cours, cours, page, fichier,
// échéances) reste consultable quand la plateforme ne répond plus ; le reste est indisponible.
import { B, WS, OUT, ok, launch, login } from '../lib/index.mjs'

const MOCK = 'http://127.0.0.1:18300'
const setDown = (on) => fetch(`${MOCK}/__mock/down?on=${on ? 1 : 0}`)
const { browser, page: p, errors } = await launch(1440, 950)
await login(p)
// Requête avec l'en-tête « hors ligne » de la réponse.
const get = (path) => p.evaluate(async (path) => {
  const r = await fetch(path, { cache: 'no-store' })
  return { status: r.status, offline: r.headers.get('x-looty-offline'), type: r.headers.get('content-type'), body: r.headers.get('content-type')?.includes('json') ? await r.json() : null }
}, path)
const base = `/api/workspaces/${WS}/moodle`
const pageUrl = encodeURIComponent('http://127.0.0.1:18300/webservice/pluginfile.php/21/mod_page/content/3/index.html?forcedownload=1')
const fileUrl = encodeURIComponent('http://127.0.0.1:18300/webservice/pluginfile.php/20/mod_resource/content/1/plan.pdf?forcedownload=1')

try {
  // Consultation en ligne : chaque réponse est gardée.
  for (const path of [`${base}/courses`, `${base}/courses/1`, `${base}/html?url=${pageUrl}`, `${base}/file?url=${fileUrl}`, `${base}/deadlines`]) {
    const r = await get(path)
    ok(`en ligne : ${path.split('/moodle')[1].split('?')[0]}`, r.status === 200 && !r.offline, String(r.status))
  }
  const ov = await get(base)
  ok('copie : éléments comptés', ov.body.offline_items >= 5, String(ov.body.offline_items))

  // Panne : la copie est servie, marquée hors ligne.
  await setDown(true)
  const courses = await get(`${base}/courses`)
  ok('panne : liste des cours depuis la copie', courses.status === 200 && !!courses.offline && courses.body.length >= 1)
  const course = await get(`${base}/courses/1`)
  ok('panne : cours déjà ouvert', course.status === 200 && !!course.offline && course.body.sections.length > 0)
  const page = await get(`${base}/html?url=${pageUrl}`)
  ok('panne : page déjà lue', page.status === 200 && !!page.offline)
  const file = await get(`${base}/file?url=${fileUrl}`)
  ok('panne : fichier déjà ouvert', file.status === 200 && !!file.offline, `${file.status} ${file.type}`)
  const unseen = await get(`${base}/courses/2`)
  ok('panne : cours jamais ouvert indisponible', unseen.status >= 400)

  // Interface : bandeau et cours consultable.
  await p.goto(`${B}/w/${WS}/moodle?course=1`, { waitUntil: 'networkidle' })
  await p.waitForTimeout(800)
  ok('bandeau hors ligne', await p.locator('.md-banner.warn').isVisible())
  await p.screenshot({ path: OUT + 'moodle-offline.png' })

  // Retour de la plateforme : plus de bandeau.
  await setDown(false)
  await p.locator('.md-banner.warn').getByRole('button').click()
  await p.waitForTimeout(1200)
  ok('plateforme revenue : bandeau retiré', !(await p.locator('.md-banner.warn').count()))
} finally {
  await setDown(false)
}
ok('aucune erreur page', !errors.length, errors.join(' | '))
await browser.close()
