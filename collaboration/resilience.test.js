const test = require('node:test')
const assert = require('node:assert/strict')
const { withRetry, authErrorForStatus } = require('./resilience')

test('authErrorForStatus', () => {
  for (const s of [200, 204]) assert.equal(authErrorForStatus(s), null)
  assert.equal(authErrorForStatus(401), 'unauthorized')
  for (const s of [403, 404]) assert.equal(authErrorForStatus(s), 'forbidden')
  for (const s of [429, 408, 500, 502, 503, 504, 400]) assert.equal(authErrorForStatus(s), 'auth-unavailable')
})

test('withRetry réussit après des échecs', async () => {
  let calls = 0
  const waits = []
  const r = await withRetry(async () => { if (++calls < 3) throw new Error('x'); return 'ok' }, { wait: async (ms) => waits.push(ms) })
  assert.equal(r, 'ok')
  assert.equal(calls, 3)
  assert.deepEqual(waits, [250, 1000])
})

test('withRetry relance la dernière erreur après 4 tentatives', async () => {
  let calls = 0
  const waits = []
  await assert.rejects(
    withRetry(async () => { throw new Error('e' + ++calls) }, { wait: async (ms) => waits.push(ms) }),
    /e4/,
  )
  assert.equal(calls, 4)
  assert.deepEqual(waits, [250, 1000, 3000])
})

test('retryFailedStores relance les documents chargés et oublie les autres', () => {
  const { retryFailedStores } = require('./resilience')
  const failed = new Map([['a', { u: 1 }], ['b', { u: 2 }]])
  const calls = []
  const r = retryFailedStores(failed, new Map([['a', 'docA']]), (d, c) => calls.push([d, c]))
  assert.deepEqual(r, ['a'])
  assert.deepEqual(calls, [['docA', { u: 1 }]])
  assert.deepEqual([...failed.keys()], ['a'])
})

test('describeRefusal ne contient ni cookie ni jeton', () => {
  const { describeRefusal } = require('./resilience')
  const line = describeRefusal({ reason: 'forbidden', status: 403, documentName: 'abc', hasToken: false, hasCookie: true, userAgent: 'Mozilla/5.0 (Android)' })
  assert.equal(line, 'refused reason=forbidden status=403 doc=abc via=cookie ua="Mozilla/5.0 (Android)"')
  assert.match(describeRefusal({ reason: 'unauthenticated', documentName: 'x'.repeat(200), userAgent: 'u'.repeat(300) }), /^refused reason=unauthenticated doc=x{64} via=none ua="u{80}"$/)
})

test('nextRevalidation : deux refus consécutifs avant de fermer, les pannes ne comptent pas', () => {
  const { nextRevalidation } = require('./resilience')
  assert.deepEqual(nextRevalidation(0, 200), { action: 'ok', failures: 0 })
  assert.deepEqual(nextRevalidation(0, 401), { action: 'recheck', failures: 1 })
  assert.deepEqual(nextRevalidation(1, 403), { action: 'forbid', failures: 0 })
  assert.deepEqual(nextRevalidation(1, 503), { action: 'keep', failures: 0 })
  assert.deepEqual(nextRevalidation(1, 200), { action: 'ok', failures: 0 })
})

test('describeRefusal ajoute cookies, empreinte, origine et adresse relayée', () => {
  const { describeRefusal, sessionCookieInfo } = require('./resilience')
  const info = sessionCookieInfo('a=1; looty_session=abc; looty_session=def', (v) => `${v}0000000000`)
  assert.deepEqual(info, { count: 2, hash: 'abc00000' })
  const line = describeRefusal({ reason: 'unauthorized', status: 401, documentName: 'd', hasCookie: true, userAgent: 'ua', cookieCount: 2, cookieHash: info.hash, origin: 'https://x.test', forwardedFor: '1.2.3.4', client: 'abc00000' })
  assert.match(line, /session_cookies=2 session_sha=abc00000 origin="https:\/\/x.test" xff="1.2.3.4" client=abc00000$/)
  assert.deepEqual(sessionCookieInfo('', (v) => v), { count: 0, hash: '' })
})

test('createRefusalLimiter : délai croissant puis abandon, fenêtre glissante, succès remet à zéro', () => {
  const { createRefusalLimiter } = require('./resilience')
  let t = 0
  const l = createRefusalLimiter({ windowMs: 1000, freeAttempts: 2, giveUpAfter: 4, now: () => t })
  assert.deepEqual(l.check('a'), { delayMs: 0, giveUp: false })
  l.note('a'); l.note('a')
  assert.deepEqual(l.check('a'), { delayMs: 500, giveUp: false })
  l.note('a')
  assert.deepEqual(l.check('a'), { delayMs: 1000, giveUp: false })
  l.note('a')
  assert.equal(l.check('a').giveUp, true)
  assert.equal(l.check('b').giveUp, false)
  t = 2000
  assert.equal(l.check('a').giveUp, false)
  l.note('a'); l.success('a')
  assert.deepEqual(l.check('a'), { delayMs: 0, giveUp: false })
})
