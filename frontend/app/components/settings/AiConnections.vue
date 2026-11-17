<script setup lang="ts">
import { Plus, Trash2, Pencil, Bot, ShieldCheck, ExternalLink, KeyRound, FlaskConical, Check, AlertTriangle } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api, errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { aiApi, type AiConnection, type AiProvider } from '~/composables/useAi'
import { AI_PROVIDER_CATALOG, OTHER_MODEL, connectionErrors, keyIssue, modelChoice, providerInfo, testResultKey } from '~/utils/aiProviders'

const { t } = useI18n()
const qc = useQueryClient()
const { data: connections } = useQuery({ queryKey: ['ai', 'connections'], queryFn: aiApi.connections })

const open = ref(false)
const editing = ref<AiConnection | null>(null)
const saving = ref(false)
const touched = ref(false)
const replacingKey = ref(false)
const otherModel = ref(false)
const testing = ref(false)
const testResult = ref<{ ok: boolean; text: string } | null>(null)
const form = reactive({ provider: 'openai' as AiProvider, label: '', model: '', base_url: '', api_key: '' })
const info = computed(() => providerInfo(form.provider))
const errors = computed(() => connectionErrors(form.provider, form, !!editing.value && !replacingKey.value))
const showKeyField = computed(() => !editing.value || replacingKey.value || !editing.value.has_key)
const prefixWarn = computed(() => keyIssue(form.provider, form.api_key) === 'prefix')
const modelSelect = computed(() => (otherModel.value ? OTHER_MODEL : modelChoice(form.provider, form.model)))
const errText = (k: string) => (touched.value && errors.value.includes(k) ? t(`ai.settings.err.${k}`) : '')
const providerName = (p: AiProvider) => providerInfo(p).name

function resetFeedback() { touched.value = false; testResult.value = null }
function start(c?: AiConnection) {
  editing.value = c ?? null
  const def = AI_PROVIDER_CATALOG[0]!
  Object.assign(form, c
    ? { provider: c.provider, label: c.label, model: c.model, base_url: c.base_url ?? '', api_key: '' }
    : { provider: def.value, label: '', model: def.defaultModel, base_url: '', api_key: '' })
  replacingKey.value = false
  otherModel.value = !!c && modelChoice(c.provider, c.model) === OTHER_MODEL
  resetFeedback()
  open.value = true
}
function pickProvider(p: AiProvider) {
  const def = providerInfo(p)
  form.provider = p
  form.model = def.defaultModel
  form.base_url = def.baseUrl ?? ''
  form.api_key = ''
  otherModel.value = false
  resetFeedback()
}
function pickModel(m: string) {
  testResult.value = null
  if (m === OTHER_MODEL) { otherModel.value = true; form.model = ''; return }
  otherModel.value = false
  form.model = m
}

async function test() {
  touched.value = true
  if (errors.value.some((e) => e !== 'apiKey' || !editing.value)) { testResult.value = null; return }
  testing.value = true
  testResult.value = null
  try {
    const body: Record<string, unknown> = { model: form.model, base_url: form.base_url, api_key: form.api_key }
    if (editing.value) body.id = editing.value.id
    else body.provider = form.provider
    const r = await api<{ ok: boolean; code?: string }>('/api/ai/connections/test', { method: 'POST', body })
    testResult.value = { ok: r.ok, text: t(r.ok ? 'ai.settings.test.ok' : testResultKey(r.code)) }
  } catch (e) {
    testResult.value = { ok: false, text: errorMessage(e) }
  } finally { testing.value = false }
}

async function save() {
  touched.value = true
  if (errors.value.length) return
  saving.value = true
  try {
    const body = { label: form.label, model: form.model, base_url: info.value.baseUrlEditable ? form.base_url : '', api_key: form.api_key }
    if (editing.value) await api(`/api/ai/connections/${editing.value.id}`, { method: 'PATCH', body })
    else await api('/api/ai/connections', { method: 'POST', body: { ...body, provider: form.provider } })
    form.api_key = ''
    open.value = false
    qc.invalidateQueries({ queryKey: ['ai', 'connections'] })
  } finally { saving.value = false }
}
async function remove(c: AiConnection) {
  if (!(await confirmDialog({ title: t('ai.settings.deleteTitle', { name: c.label }), message: t('ai.settings.deleteMessage'), danger: true, confirmLabel: t('common.delete') }))) return
  await api(`/api/ai/connections/${c.id}`, { method: 'DELETE' })
  qc.invalidateQueries({ queryKey: ['ai', 'connections'] })
}
</script>

<template>
  <SettingsSection :title="t('ai.settings.title')" :description="t('ai.settings.description')">
    <template #actions><UiButton size="sm" variant="primary" :icon="Plus" @click="start()">{{ t('ai.settings.add') }}</UiButton></template>
    <SettingsRow v-for="c in connections" :key="c.id" :label="c.label" :hint="`${providerName(c.provider)} · ${c.model}${c.base_url ? ` · ${c.base_url}` : ''}${c.key_hint ? ` · ${c.key_hint}` : ''}`">
      <span class="badge" :style="{ '--c': providerInfo(c.provider).color }" aria-hidden="true">{{ providerInfo(c.provider).badge }}</span>
      <UiButton size="sm" variant="ghost" :icon="Pencil" :aria-label="t('common.edit')" @click="start(c)" />
      <UiButton size="sm" variant="ghost" :icon="Trash2" :aria-label="t('common.delete')" @click="remove(c)" />
    </SettingsRow>

    <div v-if="connections && !connections.length" class="intro">
      <div class="intro-head"><Bot :size="18" /><strong>{{ t('ai.settings.empty.title') }}</strong></div>
      <p>{{ t('ai.settings.empty.intro') }}</p>
      <p class="can">{{ t('ai.settings.empty.can') }}</p>
      <p class="cannot">{{ t('ai.settings.empty.cannot') }}</p>
      <div class="steps-title">{{ t('ai.settings.empty.stepsTitle') }}</div>
      <ol class="steps">
        <li v-for="n in 3" :key="n"><span class="n">{{ n }}</span><div><strong>{{ t(`ai.settings.empty.step${n}`) }}</strong><span>{{ t(`ai.settings.empty.step${n}d`) }}</span></div></li>
      </ol>
      <UiButton variant="primary" :icon="Plus" @click="start()">{{ t('ai.settings.startAdd') }}</UiButton>
    </div>
    <div class="how"><ShieldCheck :size="14" /> {{ t('ai.settings.privacy') }}</div>
  </SettingsSection>

  <UiDialog :open="open" :title="editing ? t('ai.settings.edit') : t('ai.settings.add')" @update:open="(v) => (open = v)">
    <form class="form" @submit.prevent="save">
      <template v-if="!editing">
        <label class="field-label">{{ t('ai.settings.provider') }}</label>
        <div class="picker" role="radiogroup" :aria-label="t('ai.settings.provider')">
          <button v-for="p in AI_PROVIDER_CATALOG" :key="p.value" type="button" role="radio" class="pcard" :class="{ on: form.provider === p.value }" :aria-checked="form.provider === p.value" @click="pickProvider(p.value)">
            <span class="badge" :style="{ '--c': p.color }">{{ p.badge }}</span>
            <span class="pname">{{ p.name }}</span>
          </button>
        </div>
      </template>
      <div v-else class="current"><span class="badge" :style="{ '--c': info.color }">{{ info.badge }}</span><strong>{{ info.name }}</strong></div>
      <p class="help">{{ t(`ai.settings.providerHelp.${form.provider}`) }}</p>

      <label class="field-label">{{ t('ai.settings.label') }}</label>
      <UiInput v-model="form.label" maxlength="80" :invalid="!!errText('label')" :placeholder="t('ai.settings.labelPlaceholder')" />
      <p v-if="errText('label')" class="err">{{ errText('label') }}</p>

      <label class="field-label">{{ t('ai.settings.model') }}</label>
      <div class="chips" role="radiogroup" :aria-label="t('ai.settings.model')">
        <button v-for="m in info.models" :key="m" type="button" role="radio" class="chip" :class="{ on: modelSelect === m }" :aria-checked="modelSelect === m" @click="pickModel(m)">{{ m }}</button>
        <button type="button" role="radio" class="chip" :class="{ on: modelSelect === OTHER_MODEL }" :aria-checked="modelSelect === OTHER_MODEL" @click="pickModel(OTHER_MODEL)">{{ t('ai.settings.modelOther') }}</button>
      </div>
      <UiInput v-if="modelSelect === OTHER_MODEL" v-model="form.model" maxlength="120" :invalid="!!errText('model')" :placeholder="t('ai.settings.modelOtherPlaceholder')" autofocus />
      <p v-if="errText('model')" class="err">{{ errText('model') }}</p>
      <p v-else class="help">{{ t('ai.settings.modelHint') }}</p>

      <template v-if="info.baseUrlEditable">
        <label class="field-label">{{ t('ai.settings.baseUrl') }}</label>
        <UiInput v-model="form.base_url" maxlength="300" :invalid="!!errText('baseUrl') || !!errText('baseUrlFormat')" placeholder="http://localhost:11434/v1" />
        <p v-if="errText('baseUrl') || errText('baseUrlFormat')" class="err">{{ errText('baseUrl') || errText('baseUrlFormat') }}</p>
        <p v-else class="help">{{ t('ai.settings.localNote') }}</p>
      </template>

      <label class="field-label">{{ t('ai.settings.apiKey') }}{{ info.keyRequired ? '' : ` (${t('ai.settings.optional')})` }}</label>
      <template v-if="showKeyField">
        <UiInput v-model="form.api_key" type="password" autocomplete="off" :icon="KeyRound" :invalid="!!errText('apiKey') || !!errText('keySpaces')" :placeholder="info.keyPrefix ? `${info.keyPrefix}…` : ''" />
        <p v-if="errText('apiKey') || errText('keySpaces')" class="err">{{ errText('apiKey') || errText('keySpaces') }}</p>
        <p v-else-if="prefixWarn" class="warn"><AlertTriangle :size="12" /> {{ t('ai.settings.keyPrefixWarn', { prefix: info.keyPrefix ?? '' }) }}</p>
        <p v-if="info.keyUrl"><a class="link" :href="info.keyUrl" target="_blank" rel="noopener noreferrer"><ExternalLink :size="12" /> {{ t('ai.settings.getKey', { host: info.keyHost ?? '' }) }}</a></p>
        <p v-else class="help">{{ t('ai.settings.noKeyNeeded') }}</p>
        <UiButton v-if="editing && replacingKey" size="sm" variant="ghost" @click="replacingKey = false; form.api_key = ''">{{ t('ai.settings.keepCurrentKey') }}</UiButton>
      </template>
      <div v-else class="keyrow">
        <span class="faint">{{ editing?.key_hint ? t('ai.settings.currentKey', { hint: editing.key_hint }) : t('ai.settings.noStoredKey') }}</span>
        <UiButton size="sm" variant="secondary" :icon="KeyRound" @click="replacingKey = true">{{ t('ai.settings.replaceKey') }}</UiButton>
      </div>
      <p v-if="info.keyRequired || showKeyField" class="help"><ShieldCheck :size="12" /> {{ t('ai.settings.keyHelp', { provider: info.name }) }}</p>

      <div class="testrow">
        <UiButton size="sm" variant="secondary" :icon="FlaskConical" :disabled="testing" @click="test">{{ testing ? t('ai.settings.testing') : t('ai.settings.testBtn') }}</UiButton>
        <span v-if="testResult" class="result" :class="testResult.ok ? 'ok' : 'ko'" role="status"><component :is="testResult.ok ? Check : AlertTriangle" :size="13" /> {{ testResult.text }}</span>
      </div>
    </form>
    <template #footer>
      <UiButton variant="ghost" @click="open = false">{{ t('common.cancel') }}</UiButton>
      <UiButton variant="primary" :disabled="saving" @click="save">{{ t('common.save') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.how { display: flex; align-items: center; gap: 8px; padding: 14px 16px; color: var(--text-3); font-size: var(--fs-sm); border-top: 1px solid var(--border); line-height: 1.45; }
.intro { display: flex; flex-direction: column; gap: 8px; padding: 16px; color: var(--text-2); font-size: var(--fs-md); line-height: 1.5; align-items: flex-start; }
.intro p { margin: 0; }
.intro-head { display: flex; align-items: center; gap: 8px; color: var(--text); font-size: var(--fs-lg); }
.can { color: var(--green); }
.cannot { color: var(--text-3); }
.steps-title { margin-top: 6px; font-weight: 600; color: var(--text); font-size: var(--fs-sm); }
.steps { list-style: none; margin: 0 0 6px; padding: 0; display: flex; flex-direction: column; gap: 8px; }
.steps li { display: flex; gap: 10px; align-items: flex-start; }
.steps li div { display: flex; flex-direction: column; color: var(--text-3); font-size: var(--fs-sm); }
.steps li strong { color: var(--text); font-size: var(--fs-md); }
.n { flex: none; width: 20px; height: 20px; display: grid; place-items: center; border-radius: var(--r-full); background: var(--accent-soft); color: var(--accent-text); font-size: var(--fs-xs); font-weight: 600; }

.badge { flex: none; display: inline-grid; place-items: center; min-width: 24px; height: 24px; padding: 0 4px; border-radius: var(--r-sm); font-size: var(--fs-xs); font-weight: 700; color: var(--c); background: color-mix(in srgb, var(--c) 16%, transparent); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--c) 35%, transparent); }
.form { display: flex; flex-direction: column; gap: 6px; }
.field-label { font-size: var(--fs-sm); color: var(--text-2); font-weight: 500; margin-top: 8px; }
.form p { margin: 0; }
.help { display: flex; gap: 6px; align-items: flex-start; font-size: var(--fs-sm); color: var(--text-3); line-height: 1.45; }
.err { font-size: var(--fs-sm); color: var(--red); }
.warn { display: flex; align-items: center; gap: 6px; font-size: var(--fs-sm); color: var(--orange); }
.link { display: inline-flex; align-items: center; gap: 5px; font-size: var(--fs-sm); color: var(--accent-text); text-decoration: none; }
.link:hover { text-decoration: underline; }
.current { display: flex; align-items: center; gap: 8px; }
.picker { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 6px; }
.pcard { display: flex; align-items: center; gap: 8px; padding: 8px 10px; border: 1px solid var(--border); border-radius: var(--r-md); background: var(--bg-input); color: var(--text); text-align: left; transition: border-color var(--dur-fast) var(--ease), background var(--dur-fast) var(--ease); }
.pcard:hover { border-color: var(--border-strong); }
.pcard.on { border-color: var(--accent); background: var(--accent-soft); }
.pname { font-size: var(--fs-sm); font-weight: 500; }
.chips { display: flex; flex-wrap: wrap; gap: 6px; }
.chip { padding: 4px 10px; border: 1px solid var(--border); border-radius: var(--r-full); background: var(--bg-input); color: var(--text-2); font-size: var(--fs-sm); font-family: var(--font-mono); }
.chip:hover { color: var(--text); border-color: var(--border-strong); }
.chip.on { border-color: var(--accent); background: var(--accent-soft); color: var(--accent-text); }
.keyrow { display: flex; align-items: center; justify-content: space-between; gap: 8px; font-size: var(--fs-sm); }
.faint { color: var(--text-3); }
.testrow { display: flex; align-items: center; gap: 10px; margin-top: 10px; flex-wrap: wrap; }
.result { display: inline-flex; align-items: center; gap: 6px; font-size: var(--fs-sm); line-height: 1.4; }
.result.ok { color: var(--green); }
.result.ko { color: var(--red); }
</style>
