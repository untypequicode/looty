// node --test frontend/tests/pollRefresh.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { stalePolled, type PollableQuery } from '../app/composables/pollRefresh.ts'

const mk = (updated: number, interval: unknown, observers = 1, fetchStatus = 'idle'): PollableQuery => ({
  state: { dataUpdatedAt: updated, fetchStatus },
  getObserversCount: () => observers,
  observers: Array.from({ length: observers }, () => ({ options: { refetchInterval: interval } })),
})

test('seules les requêtes périodiques périmées sont rechargées', () => {
  const stale = mk(0, 1000)
  const fresh = mk(9500, 1000)
  const none = mk(0, undefined)
  const inactive = mk(0, 1000, 0)
  const busy = mk(0, 1000, 1, 'fetching')
  assert.deepEqual(stalePolled([stale, fresh, none, inactive, busy], 10_000), [stale])
})
