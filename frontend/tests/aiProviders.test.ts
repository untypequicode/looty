// node --test frontend/tests/aiProviders.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { AI_PROVIDER_CATALOG, OTHER_MODEL, connectionErrors, keyIssue, modelChoice, providerInfo, testResultKey } from '../app/utils/aiProviders.ts'

test('every provider has a default model among its suggestions and a key page when a key is needed', () => {
  assert.equal(AI_PROVIDER_CATALOG.length, 5)
  for (const p of AI_PROVIDER_CATALOG) {
    assert.ok(p.models.includes(p.defaultModel), p.value)
    if (p.keyRequired) assert.ok(p.keyUrl?.startsWith('https://') && p.keyPrefix, p.value)
    assert.equal(p.baseUrlEditable, p.value === 'openai_compatible')
  }
  assert.equal(providerInfo('anthropic').keyUrl, 'https://console.anthropic.com/settings/keys')
})

test('key prefix and whitespace checks are indicative', () => {
  assert.equal(keyIssue('anthropic', 'sk-ant-abc'), 'ok')
  assert.equal(keyIssue('anthropic', 'sk-abc'), 'prefix')
  assert.equal(keyIssue('gemini', 'AIzaXYZ'), 'ok')
  assert.equal(keyIssue('perplexity', 'pplx-1'), 'ok')
  assert.equal(keyIssue('openai', 'sk-a b'), 'spaces')
  assert.equal(keyIssue('openai', '  '), 'ok')
  assert.equal(keyIssue('openai_compatible', 'whatever'), 'ok')
})

test('model choice falls back to free text', () => {
  assert.equal(modelChoice('openai', 'gpt-4.1'), 'gpt-4.1')
  assert.equal(modelChoice('openai', 'my-ft-model'), OTHER_MODEL)
})

test('form validation per provider', () => {
  const f = { label: 'Perso', model: 'gpt-4.1', base_url: '', api_key: '' }
  assert.deepEqual(connectionErrors('openai', f, false), ['apiKey'])
  assert.deepEqual(connectionErrors('openai', f, true), [])
  assert.deepEqual(connectionErrors('openai', { ...f, label: ' ', api_key: 'sk-x' }, false), ['label'])
  assert.deepEqual(connectionErrors('openai_compatible', f, false), ['baseUrl'])
  assert.deepEqual(connectionErrors('openai_compatible', { ...f, base_url: 'localhost' }, false), ['baseUrlFormat'])
  assert.deepEqual(connectionErrors('openai_compatible', { ...f, base_url: 'http://localhost:11434/v1' }, false), [])
})

test('test result codes map to translation keys', () => {
  assert.equal(testResultKey('invalid_key'), 'ai.settings.test.invalid_key')
  assert.equal(testResultKey('weird'), 'ai.settings.test.request')
  assert.equal(testResultKey(null), 'ai.settings.test.request')
})
