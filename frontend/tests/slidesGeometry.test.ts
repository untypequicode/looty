// node --test frontend/tests/slidesGeometry.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { align, angleFromCenter, boundsOf, clampToSlide, distribute, fitScale, marquee, normalizeAngle, resizeBox, rotatedBounds, snap } from '../app/slides/geometry.ts'

const slide = { w: 1920, h: 1080 }

test('rotated bounds of a 90 degree box swap width and height around the center', () => {
  const r = rotatedBounds({ x: 100, y: 100, w: 200, h: 100, rotation: 90 })
  assert.deepEqual([Math.round(r.x), Math.round(r.y), Math.round(r.w), Math.round(r.h)], [150, 50, 100, 200])
  assert.equal(boundsOf([]), null)
})

test('resizing from the south-east handle keeps the north-west corner', () => {
  const b = resizeBox({ x: 100, y: 100, w: 200, h: 100 }, 'se', 50, 20)
  assert.deepEqual([b.x, b.y, b.w, b.h], [100, 100, 250, 120])
})

test('resizing from the north-west handle keeps the south-east corner and respects the minimum', () => {
  const b = resizeBox({ x: 100, y: 100, w: 200, h: 100 }, 'nw', -30, -10)
  assert.deepEqual([b.x, b.y, b.w, b.h], [70, 90, 230, 110])
  const tiny = resizeBox({ x: 100, y: 100, w: 200, h: 100 }, 'e', -1000, 0)
  assert.deepEqual([tiny.x, tiny.w], [100, 8])
})

test('keep ratio from a corner and resize from the center', () => {
  const b = resizeBox({ x: 0, y: 0, w: 200, h: 100 }, 'se', 100, 0, { keepRatio: true })
  assert.deepEqual([b.w, b.h], [300, 150])
  const c = resizeBox({ x: 100, y: 100, w: 200, h: 100 }, 'e', 50, 0, { fromCenter: true })
  assert.deepEqual([c.x, c.w], [50, 300])
})

test('resizing a rotated box keeps the opposite side fixed in the canvas', () => {
  const box = { x: 100, y: 100, w: 200, h: 100, rotation: 90 }
  // Rotated 90 degrees, the east handle points down: dragging it down by 50 grows the width by 50.
  const b = resizeBox(box, 'e', 0, 50)
  assert.equal(Math.round(b.w), 250)
  const westTop = (bb: { x: number; y: number; w: number; h: number }) => ({ x: Math.round(bb.x + bb.w / 2), y: Math.round(bb.y + bb.h / 2 - bb.w / 2) })
  assert.deepEqual(westTop(b), westTop(box))
})

test('align to the left, right, center and slide edges', () => {
  const items = [
    { id: 'a', box: { x: 100, y: 0, w: 100, h: 50 } },
    { id: 'b', box: { x: 400, y: 100, w: 200, h: 50 } },
  ]
  assert.equal(align(items, 'left').get('b')!.x, 100)
  assert.equal(align(items, 'right').get('a')!.x, 500)
  assert.equal(align(items, 'center').get('a')!.x, 300)
  assert.equal(align([items[0]!], 'center', slide).get('a')!.x, 910)
  assert.equal(align([items[0]!], 'bottom', slide).get('a')!.y, 1030)
})

test('distribute gives equal gaps and keeps the ends', () => {
  const items = [
    { id: 'a', box: { x: 0, y: 0, w: 100, h: 10 } },
    { id: 'b', box: { x: 130, y: 0, w: 100, h: 10 } },
    { id: 'c', box: { x: 500, y: 0, w: 100, h: 10 } },
  ]
  const p = distribute(items, 'x')
  assert.deepEqual([p.get('a')!.x, p.get('b')!.x, p.get('c')!.x], [0, 250, 500])
  assert.equal(distribute(items.slice(0, 2), 'x').size, 0)
})

test('snap aligns to the slide center and to a neighbouring edge, with guides', () => {
  const s = snap({ x: 855, y: 300, w: 200, h: 100 }, [], slide)
  assert.deepEqual([s.dx, s.dy], [5, 0])
  assert.deepEqual(s.guides.map((g) => [g.axis, g.pos]), [['x', 960]])
  const t = snap({ x: 303, y: 500, w: 100, h: 50 }, [{ x: 100, y: 100, w: 200, h: 80 }], slide)
  assert.equal(t.dx, -3)
  assert.equal(t.guides[0]!.pos, 300)
  const far = snap({ x: 700, y: 400, w: 100, h: 50 }, [], slide)
  assert.deepEqual([far.dx, far.dy, far.guides.length], [0, 0, 0])
})

test('marquee selects boxes that touch the rectangle, in any drag direction', () => {
  const items = [{ id: 'a', box: { x: 0, y: 0, w: 50, h: 50 } }, { id: 'b', box: { x: 200, y: 200, w: 50, h: 50 } }]
  assert.deepEqual(marquee({ x: 40, y: 40, w: 20, h: 20 }, items), ['a'])
  assert.deepEqual(marquee({ x: 260, y: 260, w: -100, h: -100 }, items), ['b'])
  assert.deepEqual(marquee({ x: 100, y: 100, w: 10, h: 10 }, items), [])
})

test('angles, scale and clamping', () => {
  assert.equal(normalizeAngle(-90), 270)
  assert.equal(normalizeAngle(97, 15), 90)
  assert.equal(Math.round(angleFromCenter({ x: 0, y: 0 }, { x: 0, y: -10 })), 0)
  assert.equal(Math.round(angleFromCenter({ x: 0, y: 0 }, { x: 10, y: 0 })), 90)
  assert.equal(fitScale({ w: 960, h: 1000 }, slide), 0.5)
  const c = clampToSlide({ x: 5000, y: -5000, w: 100, h: 100 }, slide)
  assert.deepEqual([c.x, c.y], [1880, -60])
})
