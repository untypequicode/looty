<script setup lang="ts">
import * as Y from 'yjs'
import { HocuspocusProvider } from '@hocuspocus/provider'
import { Plus, Play, RotateCcw, Eraser, Type, Code2, CircleDot, Info, ListEnd, Square, Cpu, Server } from 'lucide-vue-next'
import 'katex/dist/katex.min.css'
import { socketBase } from '~/composables/useRealtime'
import { useNotebookKernel, type NbOutput } from '~/composables/useNotebookKernel'
import { useJupyterKernel } from '~/composables/useJupyterKernel'
import type { KernelSpec } from '~/components/notebook/EngineDialog.vue'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'

/**
 * Notebook Jupyter partagé : cellules édités à plusieurs en direct (document Yjs
 * « file:<id> » du serveur de collaboration, enregistré au format .ipynb) ; le code Python
 * s'exécute dans le navigateur de la personne qui clique (Pyodide, par défaut) ou sur le
 * serveur Jupyter du projet (choix enregistré dans le notebook : `metadata.looty.engine`) ;
 * les sorties sont partagées avec tout le monde.
 */
interface ProjectFileLite { id: string; path: string; size: number; is_dir: boolean }
const props = defineProps<{
  fileId: string
  path: string
  projectId: string
  projectFiles?: ProjectFileLite[]
  readOnly?: boolean
  user: { name: string; color: string }
}>()
const emit = defineEmits<{
  (e: 'status', s: 'connecting' | 'synced' | 'offline' | 'denied'): void
  (e: 'change', text: string): void
  (e: 'collaborators', users: { name: string; color: string }[]): void
}>()
const { t } = useI18n()

// --- Document partagé ---
const config = useRuntimeConfig()
const ydoc = new Y.Doc()
const ycells = ydoc.getArray<Y.Map<any>>('cells')
const collabUrl = (config.public.collabUrl as string) || `${socketBase()}/collab`
const provider = new HocuspocusProvider({ url: collabUrl, name: `file:${props.fileId}`, document: ydoc })
provider.awareness?.setLocalStateField('user', { name: props.user.name, color: props.user.color, colorLight: `${props.user.color}33` })
const denied = ref(false)
const ro = computed(() => !!props.readOnly || denied.value)

interface CellVM { key: string; id: string; ymap: Y.Map<any>; type: 'code' | 'markdown' | 'raw'; ytext: Y.Text; count: number | null; outputs: NbOutput[] }
const cells = shallowRef<CellVM[]>([])
function parseOutputs(v: unknown): NbOutput[] { try { const o = JSON.parse(String(v ?? '[]')); return Array.isArray(o) ? o : [] } catch { return [] } }
function sync() {
  const seen = new Map<string, number>()
  cells.value = ycells.toArray().map((m) => {
    const id = String(m.get('id') ?? '')
    const n = (seen.get(id) ?? 0) + 1
    seen.set(id, n)
    const type = (m.get('type') as CellVM['type']) || 'code'
    return { key: `${id}:${n}:${type}`, id, ymap: m, type, ytext: m.get('source') as Y.Text, count: (m.get('count') as number | null) ?? null, outputs: parseOutputs(m.get('outputs')) }
  })
}
// Les frappes dans une cellule sont gérées par son éditeur : seule la structure et les sorties nous concernent.
ycells.observeDeep((events) => { if (events.some((e) => !(e.target instanceof Y.Text))) sync() })
sync()

const synced = ref(false)
provider.on('synced', () => {
  synced.value = true
  sync()
  // Notebook jamais ouvert (fichier vide) : le serveur en construit un ; en dernier recours, une cellule vide.
  if (!ycells.length && !ro.value) ydoc.transact(() => ycells.push([makeCell('code', '')]))
  emit('status', 'synced')
})
provider.on('authenticationFailed', () => { denied.value = true; emit('status', 'denied') })
provider.on('status', ({ status }: { status: string }) => { if (status === 'disconnected') emit('status', 'offline'); else if (status === 'connected' && provider.isSynced) emit('status', 'synced') })
provider.on('awarenessUpdate', ({ states }: { states: { user?: { name: string; color: string } }[] }) => {
  emit('collaborators', states.map((s) => s.user).filter((u): u is { name: string; color: string } => !!u && u.name !== props.user.name))
})
emit('status', 'connecting')

// --- Cellules ---
const selectedId = ref<string | null>(null)
const cellRefs = new Map<string, any>()
const newId = () => (globalThis.crypto?.randomUUID?.() ?? Math.random().toString(36).slice(2)).replace(/-/g, '').slice(0, 8)
function makeCell(type: 'code' | 'markdown', source: string, extra: Record<string, unknown> = {}) {
  const m = new Y.Map<any>()
  const tx = new Y.Text()
  tx.insert(0, source)
  m.set('id', newId())
  m.set('type', type)
  m.set('source', tx)
  m.set('outputs', '[]')
  m.set('count', null)
  m.set('meta', '{}')
  for (const [k, v] of Object.entries(extra)) m.set(k, v)
  return m
}
/** Où insérer : sous la cellule sélectionnée, sinon à la fin. */
const selectedIndex = () => {
  const i = cells.value.findIndex((c) => c.id === selectedId.value)
  return i < 0 ? cells.value.length : i + 1
}
function addAt(index: number, type: 'code' | 'markdown') {
  if (ro.value) return
  const m = makeCell(type, '')
  ydoc.transact(() => ycells.insert(Math.max(0, Math.min(index, ycells.length)), [m]))
  selectedId.value = m.get('id')
  nextTick(() => cellRefs.get(m.get('id'))?.focus())
}
async function removeAt(index: number) {
  if (ro.value) return
  const c = cells.value[index]
  if (!c) return
  if (c.ytext.length > 80 && !(await confirmDialog({ title: t('notebook.deleteTitle'), message: t('notebook.deleteHint'), danger: true, confirmLabel: t('common.delete') }))) return
  ydoc.transact(() => {
    ycells.delete(index, 1)
    if (!ycells.length) ycells.push([makeCell('code', '')])
  })
}
function moveAt(index: number, dir: -1 | 1) {
  const to = index + dir
  const c = cells.value[index]
  if (ro.value || !c || to < 0 || to >= cells.value.length) return
  // Y.Array ne déplace pas : on réinsère une copie.
  const copy = makeCell(c.type === 'raw' ? 'code' : c.type, c.ytext.toString(), { id: c.id, type: c.type, outputs: JSON.stringify(c.outputs), count: c.count, meta: c.ymap.get('meta') ?? '{}' })
  ydoc.transact(() => { ycells.delete(index, 1); ycells.insert(to, [copy]) })
  selectedId.value = c.id
}
function retype(c: CellVM, type: 'code' | 'markdown') { if (!ro.value) ydoc.transact(() => c.ymap.set('type', type)) }
function clearOutput(c: CellVM) {
  if (ro.value) return
  ydoc.transact(() => { c.ymap.set('outputs', '[]'); c.ymap.set('count', null) })
  delete local[c.id]
}
function clearAll() { for (const c of cells.value) if (c.type === 'code') clearOutput(c) }

// --- Moteur d'exécution (métadonnées du notebook, partagées) ---
const ymeta = ydoc.getMap<string>('meta')
const nbMeta = shallowRef<any>({})
function readMeta() { try { nbMeta.value = JSON.parse(ymeta.get('nb') || '{}') } catch { nbMeta.value = {} } }
ymeta.observe(readMeta)
readMeta()
const engine = computed<'pyodide' | 'jupyter'>(() => (nbMeta.value?.metadata?.looty?.engine === 'jupyter' ? 'jupyter' : 'pyodide'))
const remote = computed(() => engine.value === 'jupyter')
const nbKernelName = computed(() => (typeof nbMeta.value?.metadata?.looty?.kernel === 'string' ? nbMeta.value.metadata.looty.kernel as string : undefined))
const engineOpen = ref(false)
function setEngine(next: 'pyodide' | 'jupyter', choice?: { kernel?: string; spec: KernelSpec }) {
  if (ro.value) return
  let meta: any = {}
  try { meta = JSON.parse(ymeta.get('nb') || '{}') } catch { meta = {} }
  meta.metadata = meta.metadata && typeof meta.metadata === 'object' ? meta.metadata : {}
  if (next === 'jupyter') {
    meta.metadata.looty = { ...(meta.metadata.looty ?? {}), engine: 'jupyter' }
    if (choice?.kernel) meta.metadata.looty.kernel = choice.kernel
    else delete meta.metadata.looty.kernel
    if (choice?.spec?.name) meta.metadata.kernelspec = { name: choice.spec.name, display_name: choice.spec.display_name, language: choice.spec.language || 'python' }
  } else {
    delete meta.metadata.looty
    meta.metadata.kernelspec = { name: 'python3', display_name: 'Python 3 (Pyodide)', language: 'python' }
  }
  meta.nbformat ??= 4
  meta.nbformat_minor ??= 5
  ydoc.transact(() => ymeta.set('nb', JSON.stringify(meta)))
}

// --- Noyau Python ---
const fileList = computed(() => (props.projectFiles ?? []).filter((f) => !f.is_dir && !/\.ipynb$/i.test(f.path) && f.size <= 5_000_000).slice(0, 200))
const pyodide = useNotebookKernel({
  signature: () => fileList.value.map((f) => `${f.id}:${f.size}`).join('|'),
  files: async () => {
    const out: { path: string; data: ArrayBuffer }[] = []
    let total = 0
    for (const f of fileList.value) {
      if (total > 25_000_000) break
      try {
        const r = await fetch(`/api/projects/${props.projectId}/files/${f.id}/raw`, { credentials: 'same-origin' })
        if (!r.ok) continue
        const data = await r.arrayBuffer()
        total += data.byteLength
        out.push({ path: f.path, data })
      } catch { /* fichier illisible : ignoré */ }
    }
    return out
  },
})
const jupyter = useJupyterKernel({ projectId: () => props.projectId, kernel: () => nbKernelName.value })
// Moteur changé (ici ou par quelqu'un d'autre) : le noyau distant est arrêté, rien ne bascule en silence.
watch([engine, nbKernelName], ([e], [was, wasKernel]) => {
  if (was === 'jupyter' && (e !== 'jupyter' || nbKernelName.value !== wasKernel)) { jupyter.shutdown(); running.value = null; queued.value = new Set() }
})
const kernel = computed(() => (remote.value ? jupyter : pyodide))
/** Sorties affichées pendant le calcul (et, en lecture seule, gardées sur cet appareil). */
const local = reactive<Record<string, { outputs: NbOutput[]; count: number | null }>>({})
const running = ref<string | null>(null)
const queued = ref<Set<string>>(new Set())
const shown = (c: CellVM) => local[c.id]?.outputs ?? c.outputs
const shownCount = (c: CellVM) => (local[c.id] ? local[c.id]!.count : c.count)
const MAX_OUTPUT = 1_500_000
/** Sorties enregistrées dans le fichier : textes et images trop gros écartés. */
function capped(outputs: NbOutput[]): NbOutput[] {
  return outputs.map((o) => {
    if (o.output_type === 'stream' && String(o.text ?? '').length > 200_000) return { ...o, text: String(o.text).slice(-200_000) }
    if (o.data) {
      const data: Record<string, string | string[]> = {}
      for (const [k, v] of Object.entries(o.data)) data[k] = String(Array.isArray(v) ? v.join('') : v).length > MAX_OUTPUT ? `[${k}: ${t('notebook.tooBig')}]` : v
      return { ...o, data }
    }
    return o
  })
}
const nextCount = () => Math.max(0, ...cells.value.map((c) => c.count ?? 0), ...Object.values(local).map((l) => l.count ?? 0)) + 1

async function runCell(c: CellVM): Promise<boolean> {
  if (c.type !== 'code') return true
  const code = c.ytext.toString()
  // Exécuter à distance demande le droit de modifier : erreur visible, pas de repli sur Pyodide.
  if (remote.value && ro.value) {
    local[c.id] = { outputs: [{ output_type: 'error', ename: 'JupyterError', evalue: t('notebook.jupyter.readOnly'), traceback: [t('notebook.jupyter.readOnly')] }], count: null }
    return false
  }
  queued.value = new Set(queued.value).add(c.id)
  const res: { ok: boolean; outputs: NbOutput[]; count?: number | null } = await kernel.value.run(code, (o) => { running.value = c.id; queued.value.delete(c.id); queued.value = new Set(queued.value); local[c.id] = { outputs: o, count: null } })
  running.value = null
  queued.value.delete(c.id)
  queued.value = new Set(queued.value)
  const count = res.count ?? nextCount()
  const outputs = res.outputs.map((o) => (o.output_type === 'execute_result' ? { ...o, execution_count: count } : o))
  if (!ro.value && c.ymap.doc) {
    ydoc.transact(() => { c.ymap.set('outputs', JSON.stringify(capped(outputs))); c.ymap.set('count', count) })
    delete local[c.id]
  } else {
    local[c.id] = { outputs, count }
  }
  return res.ok
}
async function runAt(index: number, mode: 'stay' | 'next' | 'below') {
  const c = cells.value[index]
  if (!c) return
  if (mode === 'below') addAt(index + 1, 'code')
  else if (mode === 'next') {
    const nxt = cells.value[index + 1]
    if (nxt) { selectedId.value = nxt.id; nextTick(() => cellRefs.get(nxt.id)?.focus()) } else addAt(index + 1, 'code')
  }
  await runCell(c)
}
async function runAll() {
  const list = cells.value.filter((c) => c.type === 'code')
  queued.value = new Set(list.map((c) => c.id))
  for (const c of list) {
    const ok = await runCell(c)
    if (!ok) { queued.value = new Set(); break }
  }
}
function interrupt() { jupyter.interrupt() }
function restart() {
  kernel.value.restart()
  running.value = null
  queued.value = new Set()
}
const kernelLabel = computed(() => nbKernelName.value || jupyter.kernelName.value || t('notebook.jupyter.projectKernel'))
const kernelState = computed(() => kernel.value.state.value)
const statusText = computed(() => {
  const k = kernel.value
  if (remote.value) {
    const s = jupyter.state.value
    if (s === 'starting') return jupyter.message.value || t('notebook.jupyter.connecting')
    if (s === 'busy') return t('notebook.jupyter.busy', { kernel: kernelLabel.value })
    if (s === 'idle') return t('notebook.jupyter.ready', { kernel: kernelLabel.value })
    if (s === 'reconnecting') return t('notebook.jupyter.reconnecting')
    if (s === 'error') return jupyter.message.value || t('notebook.jupyter.lost')
    return t('notebook.jupyter.off', { kernel: kernelLabel.value })
  }
  if (k.state.value === 'starting') return k.message.value || t('notebook.starting')
  if (k.state.value === 'busy') return k.message.value || t('notebook.busy')
  if (k.state.value === 'idle') return t('notebook.ready')
  return k.message.value || t('notebook.off')
})

onBeforeUnmount(() => { provider.destroy(); ydoc.destroy() })
defineExpose({ getText: () => '', goToLine: (_n: number) => {}, focus: () => {} })
</script>

<template>
  <div class="nb" :class="{ ro }">
    <div class="nb-bar">
      <template v-if="!ro">
        <UiButton size="xs" variant="ghost" :icon="Code2" @click="addAt(selectedIndex(), 'code')">{{ t('notebook.addCode') }}</UiButton>
        <UiButton size="xs" variant="ghost" :icon="Type" @click="addAt(selectedIndex(), 'markdown')">{{ t('notebook.addText') }}</UiButton>
        <span class="nb-sep" />
      </template>
      <UiButton size="xs" variant="ghost" :icon="Play" :disabled="!!running || queued.size > 0" @click="runAll">{{ t('notebook.runAll') }}</UiButton>
      <UiButton v-if="remote" size="xs" variant="ghost" :icon="Square" :disabled="kernelState !== 'busy' && !running && queued.size === 0" @click="interrupt">{{ t('notebook.jupyter.interrupt') }}</UiButton>
      <UiButton size="xs" variant="ghost" :icon="RotateCcw" :disabled="kernelState === 'off'" @click="restart">{{ remote ? t('notebook.jupyter.restart') : t('notebook.restart') }}</UiButton>
      <UiButton v-if="!ro" size="xs" variant="ghost" :icon="Eraser" @click="clearAll">{{ t('notebook.clearAll') }}</UiButton>
      <span class="nb-spacer" />
      <UiButton size="xs" variant="ghost" class="nb-engine" :icon="remote ? Server : Cpu" :title="t('notebook.jupyter.title')" data-testid="nb-engine" @click="engineOpen = true">{{ remote ? t('notebook.jupyter.remoteShort') : t('notebook.jupyter.pyodideShort') }}</UiButton>
      <UiTooltip :text="remote ? t('notebook.jupyter.remoteHint') : t('notebook.pyodideHint')"><Info :size="13" class="nb-info" /></UiTooltip>
      <span class="nb-kernel" :class="kernelState" role="status" data-testid="nb-kernel" :title="statusText"><CircleDot :size="11" /> {{ statusText }}</span>
    </div>
    <div class="nb-scroll" @click.self="selectedId = null">
      <div v-if="!synced" class="nb-loading"><UiSpinner /></div>
      <template v-else>
        <NotebookCell
          v-for="(c, i) in cells" :key="c.key" :ref="(el: any) => { if (el) cellRefs.set(c.id, el); else cellRefs.delete(c.id) }"
          :id="c.id" :type="c.type" :ytext="c.ytext" :count="shownCount(c)" :outputs="shown(c)" :running="running === c.id" :queued="queued.has(c.id) && running !== c.id"
          :selected="selectedId === c.id" :read-only="ro" :awareness="provider.awareness"
          @select="selectedId = c.id" @run="(m) => runAt(i, m)" @add="(w, ty) => addAt(w === 'above' ? i : i + 1, ty)" @remove="removeAt(i)" @move="(d) => moveAt(i, d)" @clear="clearOutput(c)" @retype="(ty) => retype(c, ty)"
        />
        <div v-if="!ro" class="nb-end">
          <UiButton size="sm" variant="ghost" :icon="Plus" @click="addAt(cells.length, 'code')">{{ t('notebook.addCode') }}</UiButton>
          <UiButton size="sm" variant="ghost" :icon="ListEnd" @click="addAt(cells.length, 'markdown')">{{ t('notebook.addText') }}</UiButton>
        </div>
      </template>
    </div>
    <NotebookEngineDialog v-model:open="engineOpen" :project-id="projectId" :engine="engine" :kernel="nbKernelName" :read-only="ro" @apply="setEngine" />
  </div>
</template>

<style scoped>
.nb { display: flex; flex-direction: column; height: 100%; min-height: 0; background: var(--bg); }
.nb-bar { display: flex; align-items: center; gap: 2px; flex: none; height: 34px; padding: 0 8px; border-bottom: 1px solid var(--border); background: var(--bg-sidebar, var(--bg)); }
.nb-sep { width: 1px; height: 16px; margin: 0 6px; background: var(--border); }
.nb-spacer { flex: 1; }
.nb-info { color: var(--text-3); margin-right: 8px; cursor: help; }
.nb-kernel { display: inline-flex; align-items: center; gap: 5px; max-width: 280px; padding: 1px 9px; border-radius: var(--r-full); background: var(--bg-hover); color: var(--text-2); font-size: var(--fs-xs); font-weight: 500; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.nb-kernel.idle { color: var(--green); background: color-mix(in srgb, var(--green) 12%, transparent); }
.nb-kernel.error { color: var(--red); background: color-mix(in srgb, var(--red) 10%, transparent); }
.nb-engine { margin-right: 4px; }
.nb-kernel.busy, .nb-kernel.starting, .nb-kernel.reconnecting { color: var(--orange, #e2a03f); background: color-mix(in srgb, var(--orange, #e2a03f) 14%, transparent); }
.nb-scroll { flex: 1; min-height: 0; overflow-y: auto; padding: 18px 12px 80px 0; display: flex; flex-direction: column; gap: 6px; }
.nb-loading { display: grid; place-items: center; padding: 40px; }
.nb-end { display: flex; gap: 6px; padding-left: 68px; margin-top: 8px; }
</style>
