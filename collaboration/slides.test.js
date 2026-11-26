const test = require('node:test')
const assert = require('node:assert/strict')
const Y = require('yjs')
const { SLIDES_SCHEMA, slidesId, slidesSummary, initialSlidesState } = require('./slides')

const ID = '0f8b6c1e-3f0a-4a53-9a6a-1f2b3c4d5e6f'

test('only slides:<uuid> names are presentations', () => {
  assert.equal(slidesId(`slides:${ID}`), ID)
  assert.equal(slidesId(`canvas:${ID}`), null)
  assert.equal(slidesId('slides:nope'), null)
  assert.equal(slidesId(ID), null)
  assert.equal(SLIDES_SCHEMA, 1)
})

function deck() {
  const doc = new Y.Doc()
  const slides = doc.getMap('slides')
  const mk = (id, pos, texts, notes) => {
    const s = new Y.Map()
    slides.set(id, s)
    s.set('pos', pos)
    const els = new Y.Map()
    s.set('elements', els)
    texts.forEach((t, i) => {
      const e = new Y.Map()
      els.set(`${id}-${i}`, e)
      e.set('z', i)
      const f = new Y.XmlFragment()
      e.set('text', f)
      const p = new Y.XmlElement('paragraph')
      f.push([p])
      const x = new Y.XmlText()
      p.push([x])
      x.insert(0, t)
    })
    const n = new Y.Text()
    s.set('notes', n)
    if (notes) n.insert(0, notes)
  }
  mk('b', 2, ['Second <b>'], 'say hi')
  mk('a', 1, ['First', 'Plans & more'])
  return Y.encodeStateAsUpdate(doc)
}

test('summary counts slides and outlines their text in slide order', () => {
  const { count, text } = slidesSummary(deck())
  assert.equal(count, 2)
  assert.equal(text, '## 1\nFirst\nPlans & more\n\n## 2\nSecond <b>\nsay hi')
})

test('an empty document has no slides', () => {
  const { count, text } = slidesSummary(Y.encodeStateAsUpdate(new Y.Doc()))
  assert.deepEqual([count, text], [0, ''])
})

test('the server starts a new presentation with one deterministic slide, so simultaneous opens cannot create two', () => {
  const { INITIAL_SLIDE_ID } = require('./slides')
  const state = initialSlidesState()
  assert.deepEqual(slidesSummary(state), { count: 1, text: '## 1\n' })
  const a = new Y.Doc()
  const b = new Y.Doc()
  Y.applyUpdate(a, state)
  Y.applyUpdate(b, state)
  assert.equal(a.getMap('meta').get('schema'), SLIDES_SCHEMA)
  assert.deepEqual([a.getMap('meta').get('width'), a.getMap('meta').get('height')], [1920, 1080])
  assert.deepEqual([...a.getMap('slides').keys()], [INITIAL_SLIDE_ID])
  // Une deuxième initialisation par un client (même clé) fusionne en une seule diapositive.
  for (const d of [a, b]) {
    const s = d.getMap('slides')
    if (s.size === 0) s.set(INITIAL_SLIDE_ID, new Y.Map())
  }
  Y.applyUpdate(a, Y.encodeStateAsUpdate(b))
  assert.equal(a.getMap('slides').size, 1)
  // Même identifiant que le client (frontend/app/slides/model.ts).
  const model = require('fs').readFileSync(require('path').join(__dirname, '..', 'frontend', 'app', 'slides', 'model.ts'), 'utf8')
  assert.ok(model.includes(`INITIAL_SLIDE_ID = '${INITIAL_SLIDE_ID}'`))
  assert.ok(model.includes(`SLIDES_SCHEMA = ${SLIDES_SCHEMA}`))
})
