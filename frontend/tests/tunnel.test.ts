// node --test frontend/tests/tunnel.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { bridgeCommands, instanceUrl, shellQuote, tunnelPhase, pollDelay, validLocalUrl, validServiceName, formatLatency, machineLabel } from '../app/utils/tunnel.ts'

test('the nix line has the instance and token filled in', () => {
  const c = bridgeCommands('https://looty.example.org', 'example-token-123', [{ name: 'jupyter', url: 'http://127.0.0.1:8888' }])
  assert.equal(c.nix, 'nix run github:untypequicode/looty#bridge -- --instance https://looty.example.org --token example-token-123 --service jupyter=http://127.0.0.1:8888')
  assert.match(c.compose, /^LOOTY_INSTANCE=https:\/\/looty\.example\.org LOOTY_TOKEN=example-token-123 .*docker compose -f docker-compose\.bridge\.yml up$/)
  assert.ok(c.binary.startsWith('looty-bridge --instance'))
})

test('values with shell metacharacters are quoted', () => {
  assert.equal(shellQuote('plain-1.2'), 'plain-1.2')
  assert.equal(shellQuote('a b'), "'a b'")
  assert.equal(shellQuote("it's"), `'it'\\''s'`)
  assert.equal(shellQuote('x;rm -rf /'), "'x;rm -rf /'")
})

test('instance url loses trailing slashes', () => {
  assert.equal(instanceUrl('https://a.b/'), 'https://a.b')
  assert.equal(instanceUrl(' http://localhost:3000 '), 'http://localhost:3000')
})

test('phase: waiting, connected, disconnected, unusable', () => {
  assert.equal(tunnelPhase({ connected: false, usable: true }, true), 'waiting')
  assert.equal(tunnelPhase({ connected: true, usable: true }, true), 'connected')
  assert.equal(tunnelPhase({ connected: false, usable: true }, false), 'disconnected')
  assert.equal(tunnelPhase({ connected: false, usable: false }, false), 'unusable')
  assert.equal(tunnelPhase(null, true), 'waiting')
})

test('polling is quick while waiting and relaxed once connected', () => {
  assert.ok(pollDelay('waiting') < pollDelay('connected'))
})

test('local urls and service names are validated like the protocol', () => {
  assert.ok(validLocalUrl('http://127.0.0.1:8888'))
  assert.ok(!validLocalUrl('http://127.0.0.1:8888/?token=x'))
  assert.ok(!validLocalUrl('http://u:p@host'))
  assert.ok(!validLocalUrl('ftp://host'))
  assert.ok(validServiceName('jupyter') && validServiceName('r_2-x'))
  assert.ok(!validServiceName('') && !validServiceName('A') && !validServiceName('a'.repeat(33)))
})

test('latency and machine labels', () => {
  assert.equal(formatLatency(12.4), '12 ms')
  assert.equal(formatLatency(null), '')
  assert.equal(machineLabel('looty-bridge/0.1.0 linux-x86_64'), 'linux-x86_64')
  assert.equal(machineLabel('looty-bridge/0.1.0'), 'looty-bridge/0.1.0')
})
