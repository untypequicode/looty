// node --test tests/slidesPresent.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import * as Y from 'yjs'
import { addElement, createSlide, deleteElements, initDoc, slideIds } from '../app/slides/model.ts'
import {
  addAnim, clampPos, deckTransition, effectiveTransition, enteringAt, firstPos, getNotes, gotoSlide, hiddenAt, lastPos, moveAnim, nextPos, prevPos, readAnims,
  removeAnim, schedule, setDeckTransition, setNotes, setSlideTransition, stepCount, stepsOf, toggleScreen, updateAnim,
} from '../app/slides/present.ts'
import { elapsed, formatClock, pickPresenter } from '../app/slides/presentSync.ts'

const fresh = () => { const d = new Y.Doc(); initDoc(d); return d }
const box = { type: 'rect' as const, x: 0, y: 0, w: 10, h: 10 }

test('navigation walks steps then slides and stops at the ends', () => {
  const steps = [2, 0, 1]
  let p = firstPos(steps)
  const seen: string[] = []
  for (let i = 0; i < 7; i++) { seen.push(`${p.slide}.${p.step}`); p = nextPos(p, steps) }
  assert.deepEqual(seen, ['0.0', '0.1', '0.2', '1.0', '2.0', '2.1', '2.1'])
  assert.deepEqual(prevPos({ slide: 1, step: 0 }, steps), { slide: 0, step: 2 })
  assert.deepEqual(prevPos({ slide: 0, step: 0 }, steps), { slide: 0, step: 0 })
  assert.deepEqual(lastPos(steps), { slide: 2, step: 1 })
  assert.deepEqual(gotoSlide(2, steps), { slide: 2, step: 0 })
  assert.deepEqual(clampPos({ slide: 99, step: 99 }, steps), { slide: 2, step: 1 })
  assert.deepEqual(clampPos({ slide: 1, step: -4 }, steps), { slide: 1, step: 0 })
  assert.deepEqual(clampPos({ slide: 0, step: 0 }, []), { slide: 0, step: 0 })
})

test('black and white screens toggle and replace each other', () => {
  assert.equal(toggleScreen('', 'black'), 'black')
  assert.equal(toggleScreen('black', 'black'), '')
  assert.equal(toggleScreen('black', 'white'), 'white')
})

test('animation order: clicks open steps, "after" joins the running step', () => {
  const d = fresh()
  const s = slideIds(d)[0]!
  const a = addElement(d, s, box)!, b = addElement(d, s, box)!, c = addElement(d, s, box)!, e = addElement(d, s, box)!
  addAnim(d, s, a, 'fade', 'after')
  addAnim(d, s, b, 'move', 'click')
  addAnim(d, s, c, 'fade', 'after')
  const ide = addAnim(d, s, e, 'appear', 'click')!
  const sched = schedule(readAnims(d, s))
  assert.deepEqual(sched.map((x) => [x.step, x.delay]), [[0, 0], [1, 0], [1, 500], [2, 0]])
  assert.equal(stepCount(readAnims(d, s)), 2)
  assert.deepEqual([...hiddenAt(sched, 0)].sort(), [b, c, e].sort())
  assert.deepEqual([...hiddenAt(sched, 1)], [e])
  assert.equal(hiddenAt(sched, 2).size, 0)
  assert.deepEqual(enteringAt(sched, 1).map((x) => x.el), [b, c])
  assert.deepEqual(stepsOf(d), [2])
  moveAnim(d, s, ide, -1)
  assert.equal(readAnims(d, s)[2]!.id, ide)
  updateAnim(d, s, ide, { kind: 'fade' })
  assert.equal(readAnims(d, s).find((x) => x.id === ide)!.kind, 'fade')
  removeAnim(d, s, ide)
  assert.equal(readAnims(d, s).length, 3)
})

test('an element reappearing keeps its first entry; deleted elements drop out of the order', () => {
  const d = fresh()
  const s = slideIds(d)[0]!
  const a = addElement(d, s, box)!
  addAnim(d, s, a, 'fade', 'click')
  addAnim(d, s, a, 'move', 'click')
  assert.deepEqual([...hiddenAt(schedule(readAnims(d, s)), 0)], [a])
  assert.equal(hiddenAt(schedule(readAnims(d, s)), 1).size, 0)
  deleteElements(d, s, [a])
  assert.deepEqual(readAnims(d, s), [])
})

test('transitions: slide value wins over the deck default', () => {
  const d = fresh()
  const s = slideIds(d)[0]!
  const s2 = createSlide(d)!
  assert.equal(deckTransition(d), 'none')
  setDeckTransition(d, 'fade')
  assert.equal(effectiveTransition(d, s), 'fade')
  setSlideTransition(d, s2, 'zoom')
  assert.equal(effectiveTransition(d, s2), 'zoom')
  setSlideTransition(d, s2, '')
  assert.equal(effectiveTransition(d, s2), 'fade')
})

test('speaker notes edit only the changed part and merge between people', () => {
  const a = fresh()
  const s = slideIds(a)[0]!
  setNotes(a, s, 'Welcome everyone')
  const b = new Y.Doc()
  Y.applyUpdate(b, Y.encodeStateAsUpdate(a))
  setNotes(a, s, 'Hello, welcome everyone')
  setNotes(b, s, 'Welcome everyone!')
  Y.applyUpdate(a, Y.encodeStateAsUpdate(b))
  Y.applyUpdate(b, Y.encodeStateAsUpdate(a))
  assert.equal(getNotes(a, s), getNotes(b, s))
  assert.ok(getNotes(a, s).includes('everyone!') && getNotes(a, s).startsWith('Hello'))
})

test('follow: the most recent presenter is picked, never oneself', () => {
  const u = { name: 'A', color: '#111' }
  const states: [number, any][] = [
    [1, { user: u, present: { on: true, slide: 's1', step: 1, ts: 5 } }],
    [2, { user: { ...u, name: 'B' }, present: { on: true, slide: 's2', step: 0, ts: 9 } }],
    [3, { user: u, present: { on: false, slide: 's3', step: 0, ts: 20 } }],
    [4, { user: u }],
  ]
  assert.equal(pickPresenter(states, 9)!.name, 'B')
  assert.equal(pickPresenter(states, 2)!.clientId, 1)
  assert.equal(pickPresenter([[3, states[2]![1]]], 9), null)
})

test('timer', () => {
  assert.equal(elapsed(null, null, 1000), 0)
  assert.equal(elapsed(1000, null, 4000), 3000)
  assert.equal(elapsed(1000, 2500, 9000), 1500)
  assert.equal(formatClock(65000), '01:05')
  assert.equal(formatClock(3725000), '1:02:05')
})
