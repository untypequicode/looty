const test = require('node:test')
const assert = require('node:assert/strict')
const { CallRooms, isCallRoom, parseRoom, limitsOf } = require('./calls')

const ROOM = 'call:page:11111111-1111-1111-1111-111111111111'
const alice = { id: 'u1', name: 'Alice' }
const bob = { id: 'u2', name: 'Bob' }

function setup(options = {}) {
  const timers = []
  let t = 1000
  const rooms = new CallRooms(options, {
    setTimer: (fn, ms) => { const h = { fn, ms, off: false }; timers.push(h); return h },
    clearTimer: (h) => { h.off = true },
    now: () => t,
  })
  const client = (id, user, limits) => {
    const got = []
    rooms.attach(ROOM, id, (m) => got.push(JSON.parse(m)), user, limits)
    return {
      got,
      send: (m) => rooms.handle(ROOM, id, JSON.stringify(m)),
      last: (type) => [...got].reverse().find((m) => m.t === type),
    }
  }
  return { rooms, timers, client, tick: (ms) => { t += ms } }
}

test('room names', () => {
  assert.ok(isCallRoom(ROOM))
  assert.deepEqual(parseRoom(ROOM), { kind: 'page', id: '11111111-1111-1111-1111-111111111111' })
  assert.equal(isCallRoom('call:other:11111111-1111-1111-1111-111111111111'), false)
  assert.equal(isCallRoom('11111111-1111-1111-1111-111111111111'), false)
  assert.equal(isCallRoom('call:page:../../x'), false)
})

test('observer sees presence before joining, join and leave are broadcast', () => {
  const { rooms, client } = setup()
  const a = client('c1', alice)
  const b = client('c2', bob)
  assert.deepEqual(b.last('presence').peers, [])
  a.send({ t: 'join', peerId: 'peer-alice-1' })
  assert.equal(a.last('joined').you, 'peer-alice-1')
  assert.equal(b.last('presence').peers[0].name, 'Alice')
  b.send({ t: 'join', peerId: 'peer-bob-001', muted: true })
  assert.equal(b.last('joined').peers.length, 2)
  assert.equal(a.last('presence').peers.find((p) => p.id === 'peer-bob-001').muted, true)
  b.send({ t: 'leave' })
  assert.equal(a.last('presence').peers.length, 1)
  assert.equal(rooms.stats().peers, 1)
})

test('offer, answer and ice are relayed to the target only, sender is deduced', () => {
  const { client } = setup()
  const a = client('c1', alice)
  const b = client('c2', bob)
  const c = client('c3', { id: 'u3', name: 'Carl' })
  a.send({ t: 'join', peerId: 'peer-alice-1' })
  b.send({ t: 'join', peerId: 'peer-bob-001' })
  c.send({ t: 'join', peerId: 'peer-carl-001' })
  a.send({ t: 'signal', kind: 'offer', to: 'peer-bob-001', data: { sdp: 'x' }, from: 'peer-carl-001' })
  assert.deepEqual(b.last('signal'), { t: 'signal', from: 'peer-alice-1', kind: 'offer', data: { sdp: 'x' } })
  assert.equal(c.last('signal'), undefined)
  b.send({ t: 'signal', kind: 'answer', to: 'peer-alice-1', data: { sdp: 'y' } })
  assert.equal(a.last('signal').kind, 'answer')
  a.send({ t: 'signal', kind: 'ice', to: 'peer-bob-001', data: { candidate: 'z' } })
  assert.equal(b.last('signal').kind, 'ice')
})

test('signals need a joined sender, a known target and a valid kind', () => {
  const { client } = setup()
  const a = client('c1', alice)
  const b = client('c2', bob)
  b.send({ t: 'join', peerId: 'peer-bob-001' })
  assert.equal(a.send({ t: 'signal', kind: 'offer', to: 'peer-bob-001', data: {} }), false)
  a.send({ t: 'join', peerId: 'peer-alice-1' })
  assert.equal(a.send({ t: 'signal', kind: 'offer', to: 'nobody-here', data: {} }), false)
  assert.equal(a.send({ t: 'signal', kind: 'evil', to: 'peer-bob-001', data: {} }), false)
  assert.equal(a.send({ t: 'signal', kind: 'offer', to: 'peer-alice-1', data: {} }), false)
  assert.equal(b.last('signal'), undefined)
})

test('participant cap', () => {
  const { client, rooms } = setup({ maxParticipants: 2 })
  const cs = [1, 2, 3].map((i) => client('c' + i, { id: 'u' + i, name: 'U' + i }))
  cs.forEach((c, i) => c.send({ t: 'join', peerId: 'peer-number-' + i }))
  assert.equal(cs[2].last('error').code, 'room-full')
  assert.equal(rooms.stats().peers, 2)
})

test('disconnect keeps the place during the grace period, rejoin with the same id takes it back', () => {
  const { client, rooms, timers } = setup()
  const a = client('c1', alice)
  const b = client('c2', bob)
  a.send({ t: 'join', peerId: 'peer-alice-1' })
  b.send({ t: 'join', peerId: 'peer-bob-001' })
  rooms.detach(ROOM, 'c1')
  assert.equal(b.last('presence').peers.length, 2, 'still listed')
  const a2 = client('c9', alice)
  a2.send({ t: 'join', peerId: 'peer-alice-1' })
  assert.equal(timers[0].off, true)
  assert.equal(rooms.stats().peers, 2)
  b.send({ t: 'signal', kind: 'offer', to: 'peer-alice-1', data: {} })
  assert.equal(a2.last('signal').from, 'peer-bob-001')
})

test('cleanup: grace timeout removes the peer and the empty room', () => {
  const { client, rooms, timers } = setup()
  const a = client('c1', alice)
  const b = client('c2', bob)
  a.send({ t: 'join', peerId: 'peer-alice-1' })
  rooms.detach(ROOM, 'c1')
  timers[0].fn()
  assert.equal(b.last('presence').peers.length, 0)
  rooms.detach(ROOM, 'c2')
  assert.deepEqual(rooms.stats(), { rooms: 0, peers: 0 })
})

test('another user cannot take over a peer id', () => {
  const { client } = setup()
  const a = client('c1', alice)
  const b = client('c2', bob)
  a.send({ t: 'join', peerId: 'peer-alice-1' })
  assert.equal(b.send({ t: 'join', peerId: 'peer-alice-1' }), false)
})

test('size, shape and rate limits', () => {
  const { client, rooms, tick } = setup({ burst: 2, perSecond: 1, maxBytes: 100 })
  const a = client('c1', alice)
  assert.equal(rooms.handle(ROOM, 'c1', 'x'.repeat(101)), false)
  assert.equal(rooms.handle(ROOM, 'c1', 'not json'), false)
  assert.equal(a.send({ t: 'nope' }), false)
  assert.equal(a.send({ t: 'join', peerId: 'peer-alice-1' }), false, 'bucket empty')
  tick(2000)
  assert.equal(a.send({ t: 'join', peerId: 'bad id' }), false)
  assert.equal(a.send({ t: 'join', peerId: 'peer-alice-1' }), true)
})

test('camera and screen share stream ids are shared, invalid ones are dropped', () => {
  const { client } = setup()
  const a = client('c1', alice)
  const b = client('c2', bob)
  a.send({ t: 'join', peerId: 'peer-alice-1' })
  assert.deepEqual([b.last('presence').peers[0].cam, b.last('presence').peers[0].share], ['', ''])
  a.send({ t: 'state', muted: false, cam: 'stream-cam-1', share: '{abc-def}' })
  assert.equal(b.last('presence').peers[0].cam, 'stream-cam-1')
  assert.equal(b.last('presence').peers[0].share, '{abc-def}')
  a.send({ t: 'state', muted: false, cam: 'x'.repeat(200), share: { evil: 1 } })
  assert.equal(b.last('presence').peers[0].cam, '')
  assert.equal(b.last('presence').peers[0].share, '')
})

test('mute state is shared and an unknown connection is ignored', () => {
  const { client, rooms } = setup()
  const a = client('c1', alice)
  const b = client('c2', bob)
  a.send({ t: 'join', peerId: 'peer-alice-1' })
  a.send({ t: 'state', muted: true })
  assert.equal(b.last('presence').peers[0].muted, true)
  assert.equal(rooms.handle(ROOM, 'ghost', '{"t":"leave"}'), false)
})

test('admin limits: per-connection cap, camera and share forced off', () => {
  const { client, rooms } = setup({ maxParticipants: 6 })
  const lim = limitsOf({ calls: { enabled: true, max_participants: 2, video: false, screen_share: false } })
  assert.deepEqual(lim, { max: 2, video: false, share: false })
  const cs = [1, 2, 3].map((i) => client('c' + i, { id: 'u' + i, name: 'U' + i }, lim))
  cs.forEach((c, i) => c.send({ t: 'join', peerId: 'peer-number-' + i, cam: 'cam-1', share: 'share-1' }))
  assert.equal(cs[2].last('error').code, 'room-full')
  assert.equal(cs[2].last('error').max, 2)
  assert.equal(rooms.stats().peers, 2)
  cs[0].send({ t: 'state', muted: false, cam: 'cam-1', share: 'share-1' })
  const p = cs[1].last('presence').peers[0]
  assert.equal(p.cam, '')
  assert.equal(p.share, '')
})

test('admin limits: raising the cap on the shared object applies to the next join, no limits means defaults', () => {
  const { client } = setup({ maxParticipants: 6 })
  const lim = { max: 1, video: true, share: true }
  const a = client('c1', alice, lim)
  const b = client('c2', bob, lim)
  a.send({ t: 'join', peerId: 'peer-alice-1', cam: 'cam-1' })
  b.send({ t: 'join', peerId: 'peer-bob-001' })
  assert.equal(b.last('error').code, 'room-full')
  lim.max = 3
  b.send({ t: 'join', peerId: 'peer-bob-001' })
  assert.ok(b.last('joined'))
  assert.equal(a.last('presence').peers[0].cam, 'cam-1')
  assert.equal(limitsOf({}), null)
})
