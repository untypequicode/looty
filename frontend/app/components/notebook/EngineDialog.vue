<script setup lang="ts">
import { Cpu, Server, PlugZap, Trash2, CircleCheck, CircleAlert } from 'lucide-vue-next'
import { api, errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'

/**
 * Moteur d'un notebook : Pyodide (navigateur, par défaut) ou le serveur Jupyter du projet.
 * Le choix est enregistré dans le notebook (partagé) ; l'adresse et le jeton du serveur dans
 * le projet, côté serveur : le jeton n'est jamais renvoyé ni affiché.
 */
export interface KernelSpec { name: string; display_name: string; language: string }
interface Config { configured: boolean; url?: string; kernel?: string; has_token?: boolean; can_edit: boolean; allow_private: boolean }

const open = defineModel<boolean>('open', { default: false })
const props = defineProps<{ projectId: string; engine: 'pyodide' | 'jupyter'; kernel?: string; readOnly: boolean }>()
/** `kernel` : noyau choisi pour ce notebook (absent : celui du projet) ; `spec` : décrit le noyau utilisé. */
const emit = defineEmits<{ (e: 'apply', engine: 'pyodide' | 'jupyter', choice?: { kernel?: string; spec: KernelSpec }): void }>()
const { t } = useI18n()

const choice = ref<'pyodide' | 'jupyter'>(props.engine)
const config = ref<Config | null>(null)
const specs = ref<KernelSpec[]>([])
const specsError = ref('')
const nbKernel = ref('')
const loading = ref(false)

// Formulaire du serveur
const editing = ref(false)
const url = ref('')
const token = ref('')
const clearToken = ref(false)
const defaultKernel = ref('python3')
const testing = ref(false)
const saving = ref(false)
const testResult = ref<{ ok: boolean; text: string; kernels?: KernelSpec[] } | null>(null)

const base = computed(() => `/api/projects/${props.projectId}/jupyter`)

async function load() {
  loading.value = true
  specsError.value = ''
  try {
    config.value = await api<Config>(base.value, { silent: true })
    if (config.value.configured && config.value.can_edit) await loadSpecs()
  } catch (e) {
    specsError.value = errorMessage(e)
  } finally {
    loading.value = false
  }
  editing.value = !!config.value?.can_edit && !config.value?.configured
  url.value = config.value?.url ?? ''
  defaultKernel.value = config.value?.kernel ?? 'python3'
  token.value = ''
  clearToken.value = false
  testResult.value = null
}
async function loadSpecs() {
  specsError.value = ''
  try {
    const r = await api<{ kernels: KernelSpec[] }>(`${base.value}/test`, { method: 'POST', body: {}, silent: true })
    specs.value = r.kernels
  } catch (e) {
    specs.value = []
    specsError.value = errorMessage(e)
  }
}
watch(open, (o) => {
  if (!o) return
  choice.value = props.engine
  nbKernel.value = props.kernel ?? ''
  load()
}, { immediate: true })

const tokenBody = () => (clearToken.value ? '' : token.value.trim() ? token.value.trim() : undefined)
async function test() {
  testing.value = true
  testResult.value = null
  try {
    const r = await api<{ kernels: KernelSpec[]; version?: string }>(`${base.value}/test`, { method: 'POST', body: { url: url.value, token: tokenBody() }, silent: true })
    testResult.value = { ok: true, text: t('notebook.jupyter.testOk', { version: r.version ?? '?', n: r.kernels.length }), kernels: r.kernels }
  } catch (e) {
    testResult.value = { ok: false, text: errorMessage(e) }
  } finally {
    testing.value = false
  }
}
async function save() {
  saving.value = true
  try {
    config.value = await api<Config>(base.value, { method: 'PUT', body: { url: url.value, token: tokenBody(), kernel: defaultKernel.value }, silent: true })
    editing.value = false
    token.value = ''
    toast.success(t('notebook.jupyter.saved'))
    await loadSpecs()
  } catch (e) {
    testResult.value = { ok: false, text: errorMessage(e) }
  } finally {
    saving.value = false
  }
}
async function remove() {
  try {
    await api(base.value, { method: 'DELETE' })
    await load()
  } catch { /* message déjà affiché */ }
}

const kernelOptions = computed(() => testResult.value?.kernels ?? specs.value)
const canApply = computed(() => !props.readOnly && (choice.value === 'pyodide' || !!config.value?.configured))
function apply() {
  if (choice.value === 'jupyter') {
    const name = nbKernel.value || config.value?.kernel || 'python3'
    const spec = specs.value.find((s) => s.name === name) ?? { name, display_name: name, language: 'python' }
    emit('apply', 'jupyter', { kernel: nbKernel.value || undefined, spec })
  } else emit('apply', 'pyodide')
  open.value = false
}
</script>

<template>
  <UiDialog v-model:open="open" :title="t('notebook.jupyter.title')" :description="t('notebook.jupyter.description')" :width="560">
    <div class="ed">
      <UiSegmented v-model="choice" size="md" :options="[{ value: 'pyodide', label: t('notebook.jupyter.pyodide'), icon: Cpu }, { value: 'jupyter', label: t('notebook.jupyter.remote'), icon: Server }]" />
      <p v-if="choice === 'pyodide'" class="ed-hint">{{ t('notebook.jupyter.pyodideHint') }}</p>

      <template v-else>
        <p class="ed-hint">{{ t('notebook.jupyter.remoteHint') }}</p>
        <div v-if="loading" class="ed-loading"><UiSpinner /></div>
        <template v-else>
          <!-- Serveur du projet -->
          <section class="ed-box">
            <header class="ed-head">
              <strong>{{ t('notebook.jupyter.server') }}</strong>
              <span v-if="config?.configured && !editing" class="ed-url" :title="config.url">{{ config.url }}</span>
              <span class="ed-spacer" />
              <UiButton v-if="config?.configured && config.can_edit && !editing" size="xs" variant="ghost" @click="editing = true">{{ t('common.edit') }}</UiButton>
            </header>
            <p v-if="!config?.configured && !config?.can_edit" class="ed-warn">{{ t('notebook.jupyter.notConfiguredReader') }}</p>
            <p v-else-if="config?.configured && !editing" class="ed-meta">
              {{ config.has_token ? t('notebook.jupyter.tokenSet') : t('notebook.jupyter.noToken') }} · {{ t('notebook.jupyter.defaultKernel') }} : <code>{{ config.kernel }}</code>
            </p>
            <form v-if="editing" class="ed-form" @submit.prevent="save">
              <label class="ed-field">
                <span>{{ t('notebook.jupyter.url') }}</span>
                <UiInput v-model="url" type="url" placeholder="https://jupyter.example.org/" autocomplete="off" spellcheck="false" autofocus />
              </label>
              <label class="ed-field">
                <span>{{ t('notebook.jupyter.token') }}</span>
                <UiInput v-model="token" type="password" :placeholder="config?.has_token ? t('notebook.jupyter.tokenKeep') : t('notebook.jupyter.tokenPlaceholder')" autocomplete="new-password" :disabled="clearToken" />
              </label>
              <label v-if="config?.has_token" class="ed-check"><input v-model="clearToken" type="checkbox" /> {{ t('notebook.jupyter.tokenClear') }}</label>
              <label class="ed-field">
                <span>{{ t('notebook.jupyter.defaultKernel') }}</span>
                <select v-if="kernelOptions.length" v-model="defaultKernel" class="ed-select">
                  <option v-for="k in kernelOptions" :key="k.name" :value="k.name">{{ k.display_name }} ({{ k.name }})</option>
                </select>
                <UiInput v-else v-model="defaultKernel" placeholder="python3" spellcheck="false" />
              </label>
              <p class="ed-small">{{ t('notebook.jupyter.requirements') }}<template v-if="!config?.allow_private"> {{ t('notebook.jupyter.privateHint') }}</template></p>
              <p v-if="testResult" class="ed-result" :class="{ ok: testResult.ok }" role="status">
                <component :is="testResult.ok ? CircleCheck : CircleAlert" :size="14" /> {{ testResult.text }}
              </p>
              <div class="ed-actions">
                <UiButton v-if="config?.configured" size="sm" variant="ghost" :icon="Trash2" @click="remove">{{ t('notebook.jupyter.remove') }}</UiButton>
                <span class="ed-spacer" />
                <UiButton v-if="config?.configured" size="sm" variant="ghost" @click="editing = false">{{ t('common.cancel') }}</UiButton>
                <UiButton size="sm" :icon="PlugZap" :loading="testing" :disabled="!url.trim()" @click="test">{{ t('notebook.jupyter.test') }}</UiButton>
                <UiButton size="sm" variant="primary" type="submit" :loading="saving" :disabled="!url.trim()">{{ t('common.save') }}</UiButton>
              </div>
            </form>
          </section>

          <!-- Noyau de ce notebook -->
          <label v-if="config?.configured && !editing" class="ed-field">
            <span>{{ t('notebook.jupyter.notebookKernel') }}</span>
            <select v-model="nbKernel" class="ed-select" :disabled="readOnly">
              <option value="">{{ t('notebook.jupyter.useDefault', { kernel: config.kernel ?? 'python3' }) }}</option>
              <option v-for="k in specs" :key="k.name" :value="k.name">{{ k.display_name }} ({{ k.name }})</option>
              <option v-if="nbKernel && !specs.some((s) => s.name === nbKernel)" :value="nbKernel">{{ nbKernel }}</option>
            </select>
          </label>
          <p v-if="specsError && config?.configured" class="ed-result" role="alert"><CircleAlert :size="14" /> {{ specsError }}</p>
        </template>
      </template>
      <p v-if="readOnly" class="ed-warn">{{ t('notebook.jupyter.readOnlyChoice') }}</p>
    </div>
    <template #footer>
      <UiButton variant="ghost" @click="open = false">{{ t('common.close') }}</UiButton>
      <UiButton variant="primary" :disabled="!canApply || editing" @click="apply">{{ t('notebook.jupyter.apply') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.ed { display: flex; flex-direction: column; gap: 12px; }
.ed-hint, .ed-small, .ed-meta { margin: 0; color: var(--text-2); font-size: var(--fs-sm, 13px); line-height: 1.5; }
.ed-small { font-size: var(--fs-xs, 12px); color: var(--text-3); }
.ed-warn { margin: 0; font-size: var(--fs-sm, 13px); color: var(--orange, #e2a03f); }
.ed-loading { display: grid; place-items: center; padding: 20px; }
.ed-box { display: flex; flex-direction: column; gap: 10px; padding: 12px; border: 1px solid var(--border); border-radius: var(--r-md); }
.ed-head { display: flex; align-items: center; gap: 8px; min-width: 0; }
.ed-url { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-2); font-family: var(--font-mono, monospace); font-size: 12px; }
.ed-spacer { flex: 1; }
.ed-form { display: flex; flex-direction: column; gap: 10px; }
.ed-field { display: flex; flex-direction: column; gap: 4px; font-size: var(--fs-sm, 13px); font-weight: 500; }
.ed-check { display: flex; align-items: center; gap: 6px; font-size: var(--fs-sm, 13px); color: var(--text-2); }
.ed-select { height: 32px; padding: 0 8px; border-radius: var(--r-md); border: 1px solid var(--border); background: var(--bg-panel, var(--bg)); color: var(--text); font: inherit; font-weight: 400; }
.ed-result { display: flex; align-items: flex-start; gap: 6px; margin: 0; padding: 8px 10px; border-radius: var(--r-sm); font-size: var(--fs-sm, 13px); color: var(--red); background: color-mix(in srgb, var(--red) 8%, transparent); }
.ed-result.ok { color: var(--green); background: color-mix(in srgb, var(--green) 10%, transparent); }
.ed-result :deep(svg) { flex: none; margin-top: 2px; }
.ed-actions { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
</style>
