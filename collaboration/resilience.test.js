const test = require('node:test')
const assert = require('node:assert/strict')
const { withRetry, authErrorForStatus } = require('./resilience')

test('authErrorForStatus', () => {
  for (const s of [200, 204]) assert.equal(authErrorForStatus(s), null)
  for (const s of [401, 403, 404]) assert.equal(authErrorForStatus(s), 'forbidden')
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
