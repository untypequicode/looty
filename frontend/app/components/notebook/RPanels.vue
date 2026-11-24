<script setup lang="ts">
import { RefreshCw, Download, Trash2, X, Search, Play } from 'lucide-vue-next'
import type { NbOutput } from '~/composables/useNotebookKernel'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'
import { R_ENV_CODE, base64Bytes, helpTopic, historyStep, outputText, parseEnv, plotSrc, plotsOf, rHelpCode, rPreviewCode, type EnvVar, type PlotItem } from '~/editor/rPanels'

/**
 * Panneaux type RStudio d'un notebook R : environnement, graphiques, aide, console. Tout passe
 * par le noyau Jupyter du notebook : requêtes silencieuses (`query`), jamais une cellule.
 */
type Result = { ok: boolean; outputs: NbOutput[]; count?: number | null }
type Query = (code: string, o?: { silent?: boolean; timeoutMs?: number; onUpdate?: (o: NbOutput[]) => void }) => Promise<Result | null>
const props = defineProps<{
  query: Query
  /** État du noyau : l'environnement se rafraîchit quand un calcul se termine. */
  kernelState: string
  /** Graphiques des cellules du notebook, dans l'ordre. */
  cellPlots: PlotItem[]
  readOnly: boolean
}>()
const emit = defineEmits<{ close: [] }>()
const { t } = useI18n()

type Tab = 'env' | 'plots' | 'help' | 'console'
const tab = ref<Tab>('env')

// --- Environnement ---
const vars = ref<EnvVar[]>([])
const envState = ref<'idle' | 'loading' | 'off' | 'error'>('off')
const envError = ref('')
const preview = ref<{ name: string; text: string } | null>(null)
async function refreshEnv() {
  envState.value = 'loading'
  const r = await props.query(R_ENV_CODE, { silent: true, timeoutMs: 15000 })
  if (!r) { envState.value = 'off'; vars.value = []; return }
  if (!r.ok) { envState.value = 'error'; envError.value = outputText(r.outputs) || String(r.outputs.find((o) => o.output_type === 'error')?.evalue ?? ''); return }
  vars.value = parseEnv(outputText(r.outputs))
  envState.value = 'idle'
}
async function showPreview(v: EnvVar) {
  if (!v.frame) return
  if (preview.value?.name === v.name) { preview.value = null; return }
  const r = await props.query(rPreviewCode(v.name), { silent: true, timeoutMs: 15000 })
  preview.value = r ? { name: v.name, text: outputText(r.outputs) || t('notebook.r.panel.empty') } : null
}
let envTimer: ReturnType<typeof setTimeout> | undefined
watch(() => props.kernelState, (s, was) => {
  // Un calcul vient de finir : les variables ont pu changer.
  if (s === 'idle' && (was === 'busy' || was === 'starting')) { clearTimeout(envTimer); envTimer = setTimeout(() => { if (tab.value === 'env') refreshEnv() }, 300) }
  if (s === 'off' || s === 'error') { vars.value = []; envState.value = 'off' }
})
watch(tab, (v) => { if (v === 'env') refreshEnv() })
onMounted(refreshEnv)
onBeforeUnmount(() => clearTimeout(envTimer))

// --- Graphiques ---
const consolePlots = ref<PlotItem[]>([])
const plots = computed(() => [...props.cellPlots, ...consolePlots.value])
const plotIdx = ref(-1)
const current = computed(() => plots.value[plotIdx.value < 0 ? plots.value.length - 1 : Math.min(plotIdx.value, plots.value.length - 1)])
function save(name: string, blob: Blob) {
  const a = document.createElement('a')
  a.href = URL.createObjectURL(blob)
  a.download = name
  a.click()
  setTimeout(() => URL.revokeObjectURL(a.href), 1000)
}
function exportPlot(kind: 'png' | 'svg') {
  const p = current.value
  if (!p) return
  const n = plots.value.indexOf(p) + 1
  if (kind === 'svg' && p.svg) save(`plot-${n}.svg`, new Blob([p.svg], { type: 'image/svg+xml' }))
  else if (kind === 'png' && p.mime === 'image/png') save(`plot-${n}.png`, new Blob([base64Bytes(p.data)], { type: 'image/png' }))
  else toast(t('notebook.r.panel.noFormat'), { kind: 'error' })
}

// --- Aide ---
const helpInput = ref('')
const helpText = ref('')
const helpState = ref<'none' | 'loading' | 'found' | 'missing' | 'error'>('none')
async function showHelp(raw: string) {
  const topic = helpTopic(raw)
  if (!topic) return
  tab.value = 'help'
  helpInput.value = topic
  helpState.value = 'loading'
  const r = await props.query(rHelpCode(topic), { silent: true, timeoutMs: 20000 })
  if (!r) { helpState.value = 'error'; helpText.value = t('notebook.r.panel.needKernel'); return }
  const text = outputText(r.outputs)
  if (!r.ok) { helpState.value = 'error'; helpText.value = text || String(r.outputs.find((o) => o.output_type === 'error')?.evalue ?? ''); return }
  if (text.startsWith('\u001eNOTFOUND') || !text.trim()) { helpState.value = 'missing'; helpText.value = ''; return }
  helpState.value = 'found'
  helpText.value = text
}

// --- Console ---
interface Entry { id: number; code: string; outputs: NbOutput[]; busy: boolean }
const entries = ref<Entry[]>([])
const input = ref('')
const history = ref<string[]>([])
const histIdx = ref(-1)
let seq = 0
const logEl = ref<HTMLElement | null>(null)
const scrollDown = () => nextTick(() => { if (logEl.value) logEl.value.scrollTop = logEl.value.scrollHeight })
async function runConsole(code: string) {
  const src = code.trim()
  if (!src || props.readOnly) return
  if (history.value[history.value.length - 1] !== src) history.value.push(src)
  histIdx.value = -1
  if (/^\?/.test(src)) { showHelp(src); return }
  const e: Entry = reactive({ id: ++seq, code: src, outputs: [], busy: true })
  entries.value.push(e)
  scrollDown()
  const r = await props.query(src, { silent: false, onUpdate: (o) => { e.outputs = o; scrollDown() } })
  e.outputs = r ? r.outputs : [{ output_type: 'error', ename: 'KernelError', evalue: t('notebook.r.panel.needKernel'), traceback: [t('notebook.r.panel.needKernel')] }]
  e.busy = false
  consolePlots.value = entries.value.flatMap((x) => plotsOf(`console-${x.id}`, x.outputs))
  scrollDown()
}
function submit() {
  const code = input.value
  input.value = ''
  runConsole(code)
}
function onConsoleKey(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); submit(); return }
  const el = e.target as HTMLTextAreaElement
  const up = e.key === 'ArrowUp' && history.value.length > 0 && !input.value.slice(0, el.selectionStart).includes('\n')
  const down = e.key === 'ArrowDown' && histIdx.value >= 0 && !input.value.slice(el.selectionEnd).includes('\n')
  if (up || down) {
    const s = historyStep(history.value, histIdx.value, up ? -1 : 1)
    e.preventDefault()
    histIdx.value = s.index
    input.value = s.text
  }
}
function clearConsole() { entries.value = []; consolePlots.value = [] }

/** Appelé par le notebook : « exécuter la ligne / la sélection » dans la console. */
function send(code: string) {
  tab.value = 'console'
  return runConsole(code)
}
defineExpose({ send, help: showHelp, refresh: refreshEnv })

const tabs = computed<{ id: Tab; label: string }[]>(() => [
  { id: 'env', label: t('notebook.r.panel.env') }, { id: 'plots', label: `${t('notebook.r.panel.plots')}${plots.value.length ? ` (${plots.value.length})` : ''}` },
  { id: 'help', label: t('notebook.r.panel.help') }, { id: 'console', label: t('notebook.r.panel.console') },
])
</script>

<template>
  <aside class="rp" :aria-label="t('notebook.r.panel.title')" data-testid="r-panels">
    <header class="rp-head">
      <div class="rp-tabs" role="tablist">
        <button v-for="x in tabs" :key="x.id" role="tab" class="rp-tab" :class="{ on: tab === x.id }" :aria-selected="tab === x.id" :data-testid="`r-tab-${x.id}`" @click="tab = x.id">{{ x.label }}</button>
      </div>
      <button class="rp-ico" :aria-label="t('common.close')" :title="t('common.close')" @click="emit('close')"><X :size="14" /></button>
    </header>

    <section v-if="tab === 'env'" class="rp-body" data-testid="r-env">
      <div class="rp-tools">
        <button class="rp-ico" :aria-label="t('notebook.r.panel.refresh')" :title="t('notebook.r.panel.refresh')" :disabled="envState === 'loading'" @click="refreshEnv"><RefreshCw :size="13" /></button>
        <span class="rp-note">{{ t('notebook.r.panel.count', { n: vars.length }) }}</span>
      </div>
      <p v-if="envState === 'off'" class="rp-note">{{ t('notebook.r.panel.needKernel') }}</p>
      <pre v-else-if="envState === 'error'" class="rp-pre err">{{ envError }}</pre>
      <p v-else-if="envState === 'idle' && !vars.length" class="rp-note">{{ t('notebook.r.panel.envEmpty') }}</p>
      <table v-else-if="vars.length" class="rp-table">
        <thead><tr><th>{{ t('notebook.r.panel.name') }}</th><th>{{ t('notebook.r.panel.type') }}</th><th>{{ t('notebook.r.panel.dims') }}</th><th>{{ t('notebook.r.panel.size') }}</th></tr></thead>
        <tbody>
          <template v-for="v in vars" :key="v.name">
            <tr :class="{ click: v.frame }" :title="v.preview" data-testid="r-var" @click="showPreview(v)">
              <td class="mono">{{ v.name }}</td><td>{{ v.type }}</td><td>{{ v.dims }}</td><td>{{ v.size }}</td>
            </tr>
            <tr v-if="preview?.name === v.name"><td colspan="4"><pre class="rp-pre" data-testid="r-preview">{{ preview.text }}</pre></td></tr>
          </template>
        </tbody>
      </table>
    </section>

    <section v-else-if="tab === 'plots'" class="rp-body" data-testid="r-plots">
      <p v-if="!plots.length" class="rp-note">{{ t('notebook.r.panel.plotsEmpty') }}</p>
      <template v-else>
        <div class="rp-tools">
          <button class="rp-btn" :disabled="plots.indexOf(current!) <= 0" @click="plotIdx = plots.indexOf(current!) - 1">‹</button>
          <span class="rp-note">{{ plots.indexOf(current!) + 1 }} / {{ plots.length }}</span>
          <button class="rp-btn" :disabled="plots.indexOf(current!) >= plots.length - 1" @click="plotIdx = plots.indexOf(current!) + 1">›</button>
          <span class="rp-spacer" />
          <button class="rp-btn" :disabled="current?.mime !== 'image/png'" data-testid="r-plot-png" @click="exportPlot('png')"><Download :size="12" /> PNG</button>
          <button class="rp-btn" :disabled="!current?.svg" data-testid="r-plot-svg" @click="exportPlot('svg')"><Download :size="12" /> SVG</button>
        </div>
        <img v-if="current" class="rp-plot" :src="plotSrc(current)" alt="" />
      </template>
    </section>

    <section v-else-if="tab === 'help'" class="rp-body" data-testid="r-help">
      <form class="rp-tools" @submit.prevent="showHelp(helpInput)">
        <input v-model="helpInput" class="rp-input" :placeholder="t('notebook.r.panel.helpPlaceholder')" :aria-label="t('notebook.r.panel.help')" data-testid="r-help-input" />
        <button class="rp-btn" type="submit"><Search :size="12" /> {{ t('notebook.r.panel.search') }}</button>
      </form>
      <p v-if="helpState === 'none'" class="rp-note">{{ t('notebook.r.panel.helpHint') }}</p>
      <p v-else-if="helpState === 'loading'" class="rp-note">{{ t('notebook.r.panel.loading') }}</p>
      <p v-else-if="helpState === 'missing'" class="rp-note">{{ t('notebook.r.panel.helpMissing', { topic: helpInput }) }}</p>
      <pre v-else class="rp-pre" :class="{ err: helpState === 'error' }" data-testid="r-help-text">{{ helpText }}</pre>
    </section>

    <section v-else class="rp-body rp-console" data-testid="r-console">
      <div class="rp-tools">
        <span class="rp-note">{{ t('notebook.r.panel.consoleHint') }}</span>
        <span class="rp-spacer" />
        <button class="rp-ico" :aria-label="t('notebook.r.panel.clear')" :title="t('notebook.r.panel.clear')" :disabled="!entries.length" @click="clearConsole"><Trash2 :size="13" /></button>
      </div>
      <div ref="logEl" class="rp-log">
        <div v-for="e in entries" :key="e.id" class="rp-entry">
          <pre class="rp-code">&gt; {{ e.code }}</pre>
          <NotebookOutput :outputs="e.outputs" />
          <span v-if="e.busy" class="rp-note">…</span>
        </div>
      </div>
      <form class="rp-prompt" @submit.prevent="submit">
        <textarea v-model="input" class="rp-input mono" rows="2" :disabled="readOnly" :placeholder="readOnly ? t('notebook.jupyter.readOnly') : t('notebook.r.panel.prompt')" :aria-label="t('notebook.r.panel.console')" data-testid="r-console-input" @keydown="onConsoleKey" />
        <button class="rp-btn" type="submit" :disabled="readOnly || !input.trim()" :aria-label="t('notebook.run')"><Play :size="12" /></button>
      </form>
    </section>
  </aside>
</template>

<style scoped>
.rp { flex: none; width: 360px; min-height: 0; display: flex; flex-direction: column; border-left: 1px solid var(--border); background: var(--bg-panel); }
.rp-head { display: flex; align-items: center; justify-content: space-between; padding: 4px 6px 0; border-bottom: 1px solid var(--border); }
.rp-tabs { display: flex; gap: 2px; min-width: 0; }
.rp-tab { padding: 5px 9px; font-size: var(--fs-sm); color: var(--text-2); border-bottom: 2px solid transparent; white-space: nowrap; }
.rp-tab:hover { color: var(--text); }
.rp-tab.on { color: var(--text); border-bottom-color: var(--accent); font-weight: 600; }
.rp-body { flex: 1; min-height: 0; overflow-y: auto; padding: 8px; display: flex; flex-direction: column; gap: 6px; }
.rp-tools { display: flex; align-items: center; gap: 6px; flex: none; }
.rp-spacer { flex: 1; }
.rp-note { margin: 0; font-size: var(--fs-xs); color: var(--text-3); }
.rp-ico, .rp-btn { display: inline-flex; align-items: center; gap: 4px; padding: 3px 7px; border-radius: var(--r-sm); font-size: var(--fs-xs); color: var(--text-2); }
.rp-ico:hover:not(:disabled), .rp-btn:hover:not(:disabled) { background: var(--bg-hover); color: var(--text); }
.rp-ico:disabled, .rp-btn:disabled { opacity: .4; cursor: default; }
.rp-btn { border: 1px solid var(--border); }
.rp-table { width: 100%; border-collapse: collapse; font-size: var(--fs-xs); }
.rp-table th { text-align: left; font-weight: 600; color: var(--text-3); padding: 2px 6px; }
.rp-table td { padding: 3px 6px; border-top: 1px solid var(--border); color: var(--text-2); max-width: 120px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rp-table tr.click { cursor: pointer; }
.rp-table tr.click:hover { background: var(--bg-hover); }
.mono { font-family: var(--font-mono, ui-monospace, monospace); }
.rp-pre { margin: 0; padding: 6px 8px; font-family: var(--font-mono, ui-monospace, monospace); font-size: 12px; line-height: 1.45; white-space: pre-wrap; word-break: break-word; color: var(--text); background: var(--bg); border-radius: var(--r-sm); overflow-x: auto; }
.rp-pre.err { color: var(--red); }
.rp-plot { max-width: 100%; height: auto; align-self: flex-start; background: #fff; border-radius: var(--r-sm); }
.rp-input { flex: 1; min-width: 0; padding: 5px 8px; border: 1px solid var(--border); border-radius: var(--r-sm); background: var(--bg); color: var(--text); font-size: var(--fs-sm); resize: none; }
.rp-console { padding-bottom: 6px; }
.rp-log { flex: 1; min-height: 0; overflow-y: auto; display: flex; flex-direction: column; gap: 6px; }
.rp-code { margin: 0; font-family: var(--font-mono, ui-monospace, monospace); font-size: 12px; color: var(--accent-text, var(--accent)); white-space: pre-wrap; word-break: break-word; }
.rp-prompt { display: flex; gap: 6px; align-items: flex-end; flex: none; }
</style>
