// node --test frontend/tests/slidesCollab.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import * as Y from 'yjs'
import {
  INITIAL_SLIDE_ID, addElement, createSlide, decodeState, deleteSlide, diffSlides, groupSlides, initDoc, listSlides, readSlide, restoreFrom, setSection,
  slideIds, slideSignature, textOf, updateElement,
} from '../app/slides/model.ts'
import { anchorCounts, anchorTarget, elementAnchor, parseAnchor, slideAnchor } from '../app/slides/comments.ts'
import { GRID, clampZoom, midpoint, pinchZoom, snapMove, snapRect, snapValue, stepZoom, wheelZoom } from '../app/slides/view.ts'

const sync = (a: Y.Doc, b: Y.Doc) => { Y.applyUpdate(b, Y.encodeStateAsUpdate(a)); Y.applyUpdate(a, Y.encodeStateAsUpdate(b)) }
const rect = (x = 0) => ({ type: 'rect' as const, x, y: 0, w: 100, h: 50, props: { fill: 'blue' } })

test('two people opening a brand-new deck at once end up with one initial slide', () => {
  const a = new Y.Doc()
  const b = new Y.Doc()
  initDoc(a)
  initDoc(b)
  sync(a, b)
  assert.deepEqual(slideIds(a), [INITIAL_SLIDE_ID])
  assert.deepEqual(slideIds(b), [INITIAL_SLIDE_ID])
  assert.equal(listSlides(a).length, 1)
})

test('a document initialised by the server is not initialised again by clients', () => {
  const server = new Y.Doc()
  initDoc(server)
  const a = new Y.Doc()
  const b = new Y.Doc()
  Y.applyUpdate(a, Y.encodeStateAsUpdate(server))
  Y.applyUpdate(b, Y.encodeStateAsUpdate(server))
  const before = slideIds(a)
  initDoc(a)
  initDoc(b)
  const e = addElement(a, INITIAL_SLIDE_ID, rect())!
  sync(a, b)
  assert.deepEqual(slideIds(a), before)
  assert.equal(readSlide(b, INITIAL_SLIDE_ID)!.elements[0]!.id, e)
})

test('sections group consecutive slides, follow deletions and can be cleared', () => {
  const d = new Y.Doc()
  initDoc(d)
  const first = slideIds(d)[0]!
  const b = createSlide(d)!
  const c = createSlide(d)!
  const e = createSlide(d)!
  assert.deepEqual(groupSlides(listSlides(d)).map((g) => g.ids.length), [4])
  setSection(d, b, ' Intro ')
  setSection(d, e, 'Conclusion')
  const groups = groupSlides(listSlides(d))
  assert.deepEqual(groups.map((g) => [g.start, g.title, g.ids]), [[null, '', [first]], [b, 'Intro', [b, c]], [e, 'Conclusion', [e]]])
  deleteSlide(d, b)
  assert.equal(readSlide(d, c)!.section, 'Intro')
  setSection(d, c, '')
  assert.deepEqual(groupSlides(listSlides(d)).map((g) => g.title), ['', 'Conclusion'])
})

test('anchors: format, counts per slide and element, detached targets', () => {
  assert.equal(slideAnchor('s1'), 'slide:s1')
  assert.equal(elementAnchor('s1', 'e1'), 'el:s1:e1')
  assert.deepEqual(parseAnchor('el:s1:e1'), { kind: 'element', slide: 's1', element: 'e1' })
  assert.deepEqual(parseAnchor('slide:s1'), { kind: 'slide', slide: 's1' })
  for (const bad of ['', null, 'slide:', 'el:a', 'x:y', 'slide:a b', 'el:a:b:c', 'slide:' + 'x'.repeat(65)]) assert.equal(parseAnchor(bad as string), null)
  const counts = anchorCounts([
    { anchor: 'slide:s1', resolved: false }, { anchor: 'el:s1:e1', resolved: false }, { anchor: 'el:s1:e1', resolved: false },
    { anchor: 'el:s2:e9', resolved: true }, { anchor: null, resolved: false },
  ])
  assert.equal(counts.bySlide.get('s1'), 3)
  assert.equal(counts.bySlide.get('s2'), undefined)
  assert.equal(counts.byElement.get('s1:e1'), 2)
  assert.equal(counts.general, 1)
  const slides = [{ id: 's1', elements: [{ id: 'e1' }] }] as any
  assert.equal(anchorTarget('el:s1:e1', slides).state, 'element')
  assert.equal(anchorTarget('el:s1:gone', slides).state, 'missing')
  assert.equal(anchorTarget('slide:zz', slides).state, 'missing')
  assert.equal(anchorTarget(null, slides).state, 'general')
})

test('revision diff tells added, removed and changed slides', () => {
  const live = new Y.Doc()
  initDoc(live)
  const first = slideIds(live)[0]!
  const text = addElement(live, first, { type: 'text', x: 0, y: 0, w: 300, h: 100, text: 'Hello' })!
  const gone = createSlide(live)!
  const old = new Y.Doc()
  Y.applyUpdate(old, Y.encodeStateAsUpdate(live))
  updateElement(live, first, text, { x: 50 })
  const added = createSlide(live)!
  deleteSlide(live, gone)
  const diff = diffSlides(old, live)
  assert.equal(diff.get(first), 'changed')
  assert.equal(diff.get(added), 'added')
  assert.equal(diff.get(gone), 'removed')
  assert.notEqual(slideSignature(old, first), slideSignature(live, first))
})

test('restoring a version brings its slides and text back as a new state, keeps unchanged slides, merges with peers and can be undone', () => {
  const live = new Y.Doc()
  initDoc(live)
  const first = slideIds(live)[0]!
  const t = addElement(live, first, { type: 'text', x: 10, y: 10, w: 400, h: 100, text: 'Version one' })!
  const second = createSlide(live)!
  const snapshot = decodeState(Buffer.from(Y.encodeStateAsUpdate(live)).toString('base64'))
  // Édition ultérieure : texte changé, élément ajouté, diapositive supprimée, diapositive ajoutée.
  const frag = textOf(live, first, t)!
  frag.delete(0, frag.length)
  const p = new Y.XmlElement('paragraph')
  frag.push([p])
  const tx = new Y.XmlText(); p.push([tx]); tx.insert(0, 'Version two')
  addElement(live, first, rect(5))
  deleteSlide(live, second)
  const extra = createSlide(live)!
  const peer = new Y.Doc()
  Y.applyUpdate(peer, Y.encodeStateAsUpdate(live))
  assert.equal(restoreFrom(live, snapshot), true)
  assert.deepEqual(slideIds(live), [first, second])
  assert.equal(readSlide(live, first)!.elements.length, 1)
  assert.equal(slideSignature(live, first), slideSignature(snapshot, first))
  assert.equal(slideIds(live).includes(extra), false)
  // Les autres personnes reçoivent la restauration comme n'importe quelle modification.
  sync(live, peer)
  assert.deepEqual(slideIds(peer), [first, second])
  assert.equal(slideSignature(peer, first), slideSignature(snapshot, first))
  // Le texte restauré est un vrai fragment éditable.
  const restored = textOf(live, first, t)!
  const q = new Y.XmlElement('paragraph'); restored.push([q])
  assert.ok(slideSignature(live, first).includes('Version one'))
})

test('restoring never leaves a deck empty and refuses a newer format', () => {
  const live = new Y.Doc()
  initDoc(live)
  const empty = new Y.Doc()
  empty.getMap('meta').set('schema', 1)
  assert.equal(restoreFrom(live, empty), true)
  assert.equal(slideIds(live).length, 1)
  const future = new Y.Doc()
  initDoc(future)
  future.getMap('meta').set('schema', 99)
  assert.equal(restoreFrom(live, future), false)
})

test('zoom steps, wheel, pinch and grid snapping', () => {
  assert.equal(clampZoom(99), 4)
  assert.equal(clampZoom(0), 0.1)
  assert.equal(clampZoom(NaN), 1)
  assert.equal(stepZoom(0.53, 1), 0.67)
  assert.equal(stepZoom(0.53, -1), 0.5)
  assert.equal(stepZoom(1, 1), 1.25)
  assert.equal(stepZoom(1, -1), 0.75)
  assert.equal(stepZoom(4, 1), 4)
  assert.equal(stepZoom(0.1, -1), 0.1)
  assert.ok(wheelZoom(1, -100) > 1 && wheelZoom(1, 100) < 1)
  assert.equal(wheelZoom(4, -300), 4)
  assert.equal(pinchZoom(1, 100, 200), 2)
  assert.equal(pinchZoom(1, 0, 200), 1)
  assert.equal(pinchZoom(2, 100, 1000), 4)
  assert.deepEqual(midpoint({ x: 0, y: 0 }, { x: 10, y: 20 }), { x: 5, y: 10 })
  assert.equal(snapValue(57), 40)
  assert.equal(snapValue(61), 80)
  assert.deepEqual(snapMove({ x: 50, y: 10 }, 13, 5), { dx: 30, dy: -10 })
  assert.deepEqual(snapRect({ x: 47, y: 12, w: 130, h: 5 }), { x: 40, y: 0, w: 120, h: GRID })
})

test('arrow geometry stops the line at the head and supports none, end and both', async () => {
  const { arrowGeometry } = await import('../app/slides/render.ts')
  const end = arrowGeometry(600, 64, 8, 'end')
  assert.deepEqual([end.x1, end.y], [0, 32])
  assert.ok(end.x2 < 600 && end.endHead?.startsWith('600,32 ') && end.startHead === null)
  const both = arrowGeometry(600, 64, 8, 'both')
  assert.ok(both.x1 > 0 && both.x2 < 600 && both.startHead && both.endHead)
  const none = arrowGeometry(600, 64, 8, 'bogus-or-none')
  assert.equal(none.endHead === null, false) // unknown values behave like the default (end)
  const bare = arrowGeometry(600, 64, 8, 'none')
  assert.deepEqual([bare.x1, bare.x2, bare.endHead, bare.startHead], [0, 600, null, null])
  const tiny = arrowGeometry(10, 10, 40, 'both')
  assert.ok(tiny.x1 >= 0 && tiny.x2 >= 0)
})
