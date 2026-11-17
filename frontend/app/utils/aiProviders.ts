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
    baseUrl: null, baseUrlEditable: false, models: [], defaultModel: '',
  },
  {
    value: 'anthropic', name: 'Anthropic (Claude)', badge: 'C', color: 'var(--orange)',
    keyUrl: 'https://console.anthropic.com/settings/keys', keyHost: 'console.anthropic.com', keyPrefix: 'sk-ant-', keyRequired: true,
    baseUrl: null, baseUrlEditable: false, models: ['claude-sonnet-5-5', 'claude-opus-5-5', 'claude-haiku-5-5', 'claude-fable-5-2'], defaultModel: 'claude-sonnet-5-5',
  },
  {
    value: 'gemini', name: 'Google Gemini', badge: 'Ge', color: 'var(--blue)',
    keyUrl: 'https://aistudio.google.com/apikey', keyHost: 'aistudio.google.com', keyPrefix: 'AIza', keyRequired: true,
    baseUrl: null, baseUrlEditable: false, models: ['gemini-3-flash-preview', 'gemini-3.1-pro-preview'], defaultModel: 'gemini-3-flash-preview',
  },
  {
    value: 'perplexity', name: 'Perplexity', badge: 'P', color: 'var(--purple)',
    keyUrl: 'https://www.perplexity.ai/settings/api', keyHost: 'perplexity.ai', keyPrefix: 'pplx-', keyRequired: true,
    baseUrl: null, baseUrlEditable: false, models: ['sonar', 'sonar-pro'], defaultModel: 'sonar', noTools: true,
  },
  {
    value: 'openai_compatible', name: 'Ollama / compatible OpenAI', badge: 'O', color: 'var(--text-3)',
    keyUrl: null, keyHost: null, keyPrefix: null, keyRequired: false,
    baseUrl: 'http://localhost:11434/v1', baseUrlEditable: true, models: [], defaultModel: '',
  },
]

export function providerInfo(p: string): AiProviderInfo {
  return AI_PROVIDER_CATALOG.find((x) => x.value === p) ?? AI_PROVIDER_CATALOG[AI_PROVIDER_CATALOG.length - 1]!
}

/** Valeur du sélecteur de modèle : le modèle suggéré, ou `OTHER_MODEL` pour une saisie libre. */
export const OTHER_MODEL = '__other'
export interface ModelOption { id: string; label?: string }

/** Réponse de POST /api/ai/connections/models → liste propre (ids uniques et non vides ; ordre du serveur). */
export function normalizeModels(raw: unknown): ModelOption[] {
  if (!Array.isArray(raw)) return []
  const seen = new Set<string>()
  const out: ModelOption[] = []
  for (const m of raw) {
    const id = typeof m?.id === 'string' ? m.id.trim() : ''
    if (!id || seen.has(id)) continue
    seen.add(id)
    out.push(typeof m.label === 'string' && m.label && m.label !== id ? { id, label: m.label } : { id })
  }
  return out
}

/** Modèles proposés : la liste chargée avec la clé, sinon (avant chargement ou en cas d'échec) les suggestions du catalogue. */
export function modelOptions(provider: string, loaded: ModelOption[] | null | undefined): ModelOption[] {
  return loaded?.length ? loaded : providerInfo(provider).models.map((id) => ({ id }))
}

export function modelChoice(provider: string, model: string, loaded?: ModelOption[] | null): string {
  return modelOptions(provider, loaded).some((m) => m.id === model) ? model : OTHER_MODEL
}

/** Peut-on charger la liste ? Pas de point d'entrée chez Perplexity ; sinon clé plausible (ou clé déjà enregistrée), ou adresse pour un serveur local. */
export function canLoadModels(provider: string, f: { api_key: string; base_url: string }, hasStoredKey: boolean): boolean {
  if (provider === 'perplexity') return false
  const info = providerInfo(provider)
  if (info.baseUrlEditable) return /^https?:\/\/[^\s/]+/i.test(f.base_url.trim())
  const k = f.api_key.trim()
  if (k) return keyIssue(provider, k) === 'ok' && k.length >= 20
  return hasStoredKey
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
  const known = ['models_unsupported', 'no_key', 'bad_url', 'invalid_key', 'quota', 'model_not_found', 'server', 'request', 'unreachable']
  return `ai.settings.test.${known.includes(code ?? '') ? code : 'request'}`
}
