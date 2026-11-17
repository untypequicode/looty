// Catalogue des fournisseurs d'IA proposés dans « Connexions IA » : où obtenir la clé, format attendu,
// modèles suggérés, adresse par défaut. Pur (sans Vue) pour être testé : frontend/tests/aiProviders.test.ts.
// Les identifiants de modèle sont des suggestions : « Autre modèle » permet d'en saisir un autre.

export type AiProviderId = 'openai' | 'anthropic' | 'gemini' | 'perplexity' | 'openai_compatible'

export interface AiProviderInfo {
  value: AiProviderId
  name: string
  /** Lettre du badge (pas de logo officiel embarqué). */
  badge: string
  /** Couleur du badge : un jeton de tokens.css. */
  color: string
  /** Page officielle où créer une clé d'API (aucune pour un serveur local). */
  keyUrl: string | null
  keyHost: string | null
  /** Début attendu de la clé (contrôle indicatif, jamais bloquant). */
  keyPrefix: string | null
  keyRequired: boolean
  /** Adresse du serveur : modifiable uniquement pour un serveur compatible. */
  baseUrl: string | null
  baseUrlEditable: boolean
  models: string[]
  defaultModel: string
  /** Le fournisseur ne sait pas appeler d'outils (lecture des pages, etc.). */
  noTools?: boolean
}

export const AI_PROVIDER_CATALOG: AiProviderInfo[] = [
  {
    value: 'openai', name: 'OpenAI (GPT)', badge: 'G', color: 'var(--green)',
    keyUrl: 'https://platform.openai.com/api-keys', keyHost: 'platform.openai.com', keyPrefix: 'sk-', keyRequired: true,
    baseUrl: null, baseUrlEditable: false, models: ['gpt-4.1', 'gpt-4.1-mini', 'gpt-4o'], defaultModel: 'gpt-4.1',
  },
  {
    value: 'anthropic', name: 'Anthropic (Claude)', badge: 'C', color: 'var(--orange)',
    keyUrl: 'https://console.anthropic.com/settings/keys', keyHost: 'console.anthropic.com', keyPrefix: 'sk-ant-', keyRequired: true,
    baseUrl: null, baseUrlEditable: false, models: ['claude-sonnet-4-5', 'claude-haiku-4-5', 'claude-opus-4-1'], defaultModel: 'claude-sonnet-4-5',
  },
  {
    value: 'gemini', name: 'Google Gemini', badge: 'Ge', color: 'var(--blue)',
    keyUrl: 'https://aistudio.google.com/apikey', keyHost: 'aistudio.google.com', keyPrefix: 'AIza', keyRequired: true,
    baseUrl: null, baseUrlEditable: false, models: ['gemini-3.5-flash-lite', 'gemini-2.5-flash', 'gemini-2.5-pro'], defaultModel: 'gemini-3.5-flash-lite',
  },
  {
    value: 'perplexity', name: 'Perplexity', badge: 'P', color: 'var(--purple)',
    keyUrl: 'https://www.perplexity.ai/settings/api', keyHost: 'perplexity.ai', keyPrefix: 'pplx-', keyRequired: true,
    baseUrl: null, baseUrlEditable: false, models: ['sonar', 'sonar-pro'], defaultModel: 'sonar', noTools: true,
  },
  {
    value: 'openai_compatible', name: 'Ollama / compatible OpenAI', badge: 'O', color: 'var(--text-3)',
    keyUrl: null, keyHost: null, keyPrefix: null, keyRequired: false,
    baseUrl: 'http://localhost:11434/v1', baseUrlEditable: true, models: ['llama3.1', 'qwen2.5', 'mistral'], defaultModel: 'llama3.1',
  },
]

export function providerInfo(p: string): AiProviderInfo {
  return AI_PROVIDER_CATALOG.find((x) => x.value === p) ?? AI_PROVIDER_CATALOG[AI_PROVIDER_CATALOG.length - 1]!
}

/** Valeur du sélecteur de modèle : le modèle suggéré, ou `OTHER_MODEL` pour une saisie libre. */
export const OTHER_MODEL = '__other'
export function modelChoice(provider: string, model: string): string {
  return providerInfo(provider).models.includes(model) ? model : OTHER_MODEL
}

/** Contrôle indicatif de la clé : 'ok' (ou vide), 'spaces' (espaces/retours dedans), 'prefix' (mauvais début). */
export function keyIssue(provider: string, key: string): 'ok' | 'spaces' | 'prefix' {
  const k = key.trim()
  if (!k) return 'ok'
  if (/\s/.test(k)) return 'spaces'
  const prefix = providerInfo(provider).keyPrefix
  return prefix && !k.startsWith(prefix) ? 'prefix' : 'ok'
}

/** Champs manquants ou invalides (codes de clés i18n `ai.settings.err.*`) ; vide = prêt à enregistrer. */
export function connectionErrors(
  provider: string,
  f: { label: string; model: string; base_url: string; api_key: string },
  editing: boolean,
): string[] {
  const info = providerInfo(provider)
  const out: string[] = []
  if (!f.label.trim()) out.push('label')
  if (!f.model.trim()) out.push('model')
  if (info.baseUrlEditable) {
    if (!f.base_url.trim()) out.push('baseUrl')
    else if (!/^https?:\/\/[^\s/]+/i.test(f.base_url.trim())) out.push('baseUrlFormat')
  }
  if (info.keyRequired && !editing && !f.api_key.trim()) out.push('apiKey')
  if (keyIssue(provider, f.api_key) === 'spaces') out.push('keySpaces')
  return out
}

/** Code renvoyé par POST /api/ai/connections/test → clé de traduction. */
export function testResultKey(code: string | null | undefined): string {
  const known = ['no_key', 'bad_url', 'invalid_key', 'quota', 'model_not_found', 'server', 'request', 'unreachable']
  return `ai.settings.test.${known.includes(code ?? '') ? code : 'request'}`
}
