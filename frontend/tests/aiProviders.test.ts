// node --test frontend/tests/aiProviders.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import {
  AI_PROVIDER_CATALOG, OTHER_MODEL, canLoadModels, connectionErrors, keyIssue, modelChoice, modelOptions, normalizeModels, providerInfo, testResultKey,
} from '../app/utils/aiProviders.ts'

test('every provider has a consistent default model and a key page when a key is needed', () => {
  assert.equal(AI_PROVIDER_CATALOG.length, 5)
  for (const p of AI_PROVIDER_CATALOG) {
    assert.ok(p.defaultModel === '' ? p.models.length === 0 : p.models.includes(p.defaultModel), p.value)
    if (p.keyRequired) assert.ok(p.keyUrl?.startsWith('https://') && p.keyPrefix, p.value)
    assert.equal(p.baseUrlEditable, p.value === 'openai_compatible')
  }
  assert.equal(providerInfo('anthropic').keyUrl, 'https://console.anthropic.com/settings/keys')
})

test('no hardcoded OpenAI ids; Anthropic and Gemini fallbacks are the agreed ids only', () => {
  assert.deepEqual(providerInfo('openai').models, [])
  assert.deepEqual(providerInfo('anthropic').models, ['claude-sonnet-5-5', 'claude-opus-5-5', 'claude-haiku-5-5', 'claude-fable-5-2'])
  assert.deepEqual(providerInfo('gemini').models, ['gemini-3-flash-preview', 'gemini-3.1-pro-preview'])
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

test('loaded models replace the fallback; empty or failed loads keep it', () => {
  const loaded = [{ id: 'gemini-9-pro' }, { id: 'gemini-9-flash', label: 'Flash 9' }]
  assert.deepEqual(modelOptions('gemini', loaded), loaded)
  assert.deepEqual(modelOptions('gemini', null).map((m) => m.id), ['gemini-3-flash-preview', 'gemini-3.1-pro-preview'])
  assert.deepEqual(modelOptions('gemini', []).map((m) => m.id), ['gemini-3-flash-preview', 'gemini-3.1-pro-preview'])
  assert.deepEqual(modelOptions('openai', null), [])
  assert.equal(modelChoice('openai', 'gpt-live', [{ id: 'gpt-live' }]), 'gpt-live')
  assert.equal(modelChoice('openai', 'gpt-live'), OTHER_MODEL)
  assert.equal(modelChoice('anthropic', 'claude-sonnet-5-5'), 'claude-sonnet-5-5')
})

test('server model list is normalised', () => {
  assert.deepEqual(
    normalizeModels([{ id: ' a ', label: 'A' }, { id: 'a' }, { id: '' }, null, { id: 'b', label: 'b' }, { label: 'x' }]),
    [{ id: 'a', label: 'A' }, { id: 'b' }],
  )
  assert.deepEqual(normalizeModels(undefined), [])
})

test('when the model list can be loaded', () => {
  const long = 'sk-' + 'x'.repeat(30)
  assert.equal(canLoadModels('openai', { api_key: long, base_url: '' }, false), true)
  assert.equal(canLoadModels('openai', { api_key: 'sk-short', base_url: '' }, false), false)
  assert.equal(canLoadModels('anthropic', { api_key: long, base_url: '' }, false), false)
  assert.equal(canLoadModels('openai', { api_key: '', base_url: '' }, true), true)
  assert.equal(canLoadModels('perplexity', { api_key: 'pplx-' + 'x'.repeat(30), base_url: '' }, true), false)
  assert.equal(canLoadModels('openai_compatible', { api_key: '', base_url: 'http://localhost:11434/v1' }, false), true)
  assert.equal(canLoadModels('openai_compatible', { api_key: '', base_url: 'localhost' }, false), false)
})

test('form validation per provider', () => {
  const f = { label: 'Perso', model: 'gpt-x', base_url: '', api_key: '' }
  assert.deepEqual(connectionErrors('openai', f, false), ['apiKey'])
  assert.deepEqual(connectionErrors('openai', f, true), [])
  assert.deepEqual(connectionErrors('openai', { ...f, label: ' ', api_key: 'sk-x' }, false), ['label'])
  assert.deepEqual(connectionErrors('openai_compatible', f, false), ['baseUrl'])
  assert.deepEqual(connectionErrors('openai_compatible', { ...f, base_url: 'localhost' }, false), ['baseUrlFormat'])
  assert.deepEqual(connectionErrors('openai_compatible', { ...f, base_url: 'http://localhost:11434/v1' }, false), [])
})

test('test result codes map to translation keys', () => {
  assert.equal(testResultKey('invalid_key'), 'ai.settings.test.invalid_key')
  assert.equal(testResultKey('models_unsupported'), 'ai.settings.test.models_unsupported')
  assert.equal(testResultKey('weird'), 'ai.settings.test.request')
  assert.equal(testResultKey(null), 'ai.settings.test.request')
})
