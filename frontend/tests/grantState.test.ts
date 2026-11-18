// node --test frontend/tests/grantState.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { cardGrantState, type EndedGrant } from '../app/utils/grantState.ts'

const none = new Set<string>()
const ended = (key: string, created_at: string): EndedGrant => ({ key, created_at, ended_at: '2026-01-01T12:00:00Z' })

test('une autorisation en cours prime', () => {
  assert.equal(cardGrantState('p:1', '2026-01-01T10:00:00Z', new Set(['p:1']), []), 'active')
})

test('autorisation donnée après la carte puis échue : carte expirée', () => {
  assert.equal(cardGrantState('p:1', '2026-01-01T10:00:00Z', none, [ended('p:1', '2026-01-01T10:05:00Z')]), 'expired')
})

test('autorisation antérieure à la carte : la carte reste à traiter', () => {
  assert.equal(cardGrantState('p:1', '2026-01-01T11:00:00Z', none, [ended('p:1', '2026-01-01T10:00:00Z')]), 'pending')
})

test('autre cible ou aucune autorisation : à traiter', () => {
  assert.equal(cardGrantState('p:1', '2026-01-01T10:00:00Z', none, [ended('p:2', '2026-01-01T10:05:00Z')]), 'pending')
  assert.equal(cardGrantState('c:personal', undefined, none, []), 'pending')
})
