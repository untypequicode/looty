import { onBeforeUnmount, ref } from 'vue'
import { translate } from './useI18n'
import { toast } from './useToast'

/**
 * Noyau Python d'un notebook : Pyodide dans un Worker (`public/notebook-worker.js`).
 * Un noyau par notebook ouvert et par personne : il démarre au premier calcul, garde ses
 * variables entre les cellules, et redémarre à la demande (état perdu).
 */
export interface NbOutput {
  output_type: 'stream' | 'execute_result' | 'display_data' | 'error'
  name?: string
  text?: string | string[]
  data?: Record<string, string | string[]>
  metadata?: Record<string, unknown>
  execution_count?: number | null
  ename?: string
  evalue?: string
  traceback?: string[]
}
export type KernelState = 'off' | 'starting' | 'idle' | 'busy'

interface Pending {
  outputs: NbOutput[]
  onUpdate: (outputs: NbOutput[]) => void
  resolve: (r: { ok: boolean; outputs: NbOutput[] }) => void
}

/** Fichier du projet vu par le noyau ; `key` change quand son contenu change (version, taille…). */
export interface KernelEntry { path: string; key: string; size: number }
export interface InstallResult { ok: boolean; error?: string; wasm?: boolean }
const MAX_TOTAL = 25_000_000

export function useNotebookKernel(opts: {
  /** Liste à jour des fichiers du projet visibles par le code (appelée avant chaque exécution). */
  list?: () => Promise<KernelEntry[]>
  /** Contenu d'un fichier de la liste. */
  read?: (path: string) => Promise<ArrayBuffer | null>
  /** Paquets installés par « %pip install » : à inscrire dans requirements.txt. */
  onInstalled?: (specs: string[]) => void | Promise<void>
} = {}) {
  const state = ref<KernelState>('off')
  const message = ref('')
  let worker: Worker | null = null
  let readyP: Promise<void> | null = null
  /** Fichiers déjà envoyés au Worker : chemin → clé de version. */
  let known = new Map<string, string>()
  let synced: (() => void) | null = null
  let warnings: NbOutput[] = []
  let installedP: Promise<void> = Promise.resolve()
  const installs = new Map<string, (r: InstallResult) => void>()
  const pending = new Map<string, Pending>()
  let seq = 0
  /** Premier envoi des fichiers fait (le Worker installe alors requirements.txt avant la première cellule). */
  let synchronized = false
  /** Exécutions l'une après l'autre. */
  let chain: Promise<unknown> = Promise.resolve()

  function start() {
    if (readyP) return readyP
    state.value = 'starting'
    message.value = ''
    worker = new Worker('/notebook-worker.js', { type: 'module' })
    readyP = new Promise<void>((resolve, reject) => {
      worker!.onmessage = (e: MessageEvent) => {
        const d = e.data
        switch (d.type) {
          case 'status': message.value = d.key ? translate(d.key, d.params) : d.text; break
          case 'synced': synced?.(); synced = null; break
          case 'installed': if (opts.onInstalled) installedP = installedP.then(() => opts.onInstalled!(d.specs)).catch(() => {}); break
          case 'warn': warnings.push({ output_type: 'stream', name: 'stderr', text: `${translate('notebook.deps.warn', { pkg: d.pkg })}${d.wasm ? ` ${translate('notebook.deps.wasm')}` : ''}\n${String(d.error ?? '').trim().split('\n').pop()}\n` }); break
          case 'install-done': {
            const done = installs.get(d.id)
            installs.delete(d.id)
            installedP.then(() => done?.({ ok: d.ok, error: d.error, wasm: d.wasm }))
            break
          }
          case 'ready': state.value = 'idle'; message.value = ''; resolve(); break
          case 'fatal': state.value = 'off'; message.value = d.text; reject(new Error(d.text)); break
          case 'stream': {
            const p = pending.get(d.id)
            if (!p) break
            const last = p.outputs[p.outputs.length - 1]
            if (last?.output_type === 'stream' && last.name === d.name) last.text = String(last.text ?? '') + d.text
            else p.outputs.push({ output_type: 'stream', name: d.name, text: d.text })
            p.onUpdate(p.outputs.slice())
            break
          }
          case 'done': {
            const p = pending.get(d.id)
            if (!p) break
            pending.delete(d.id)
            for (const png of d.figures ?? []) p.outputs.push({ output_type: 'display_data', data: { 'image/png': png, 'text/plain': '<Figure>' }, metadata: {} })
            if (d.value) p.outputs.push({ output_type: 'execute_result', data: d.value, metadata: {}, execution_count: null })
            if (d.error) p.outputs.push({ output_type: 'error', ename: d.error.ename, evalue: d.error.evalue, traceback: d.error.traceback })
            p.resolve({ ok: !d.error, outputs: p.outputs })
            break
          }
        }
      }
      worker!.onerror = (e) => { state.value = 'off'; message.value = e.message; reject(new Error(e.message)) }
    })
    worker.postMessage({ type: 'boot' })
    return readyP
  }

  let refreshWarned = false
  function warnRefresh() {
    if (refreshWarned) return
    refreshWarned = true
    toast(translate('notebook.deps.refreshFailed'), { kind: 'error', duration: 8000 })
  }

  /** Fichiers du projet visibles par le code (.py importables, .csv…) : seuls ceux qui ont changé sont renvoyés au Worker. */
  async function syncFiles() {
    if (!opts.list || !opts.read || !worker) return
    const first = known.size === 0 && !synchronized
    let list: KernelEntry[]
    try { list = await opts.list() } catch {
      // Première synchro ratée : rien n'est envoyé (requirements.txt sera installé à la prochaine exécution).
      if (!first) warnRefresh()
      return
    }
    const present = new Set(list.map((f) => f.path))
    const removed = [...known.keys()].filter((p) => !present.has(p))
    const sizes = new Map(list.map((f) => [f.path, f.size]))
    let total = [...known.keys()].reduce((n, p) => n + (sizes.get(p) ?? 0), 0)
    const files: { path: string; data: ArrayBuffer }[] = []
    const keys: [string, string][] = []
    for (const f of list) {
      if (known.get(f.path) === f.key) continue
      if (total + f.size > MAX_TOTAL) continue
      try {
        const data = await opts.read(f.path)
        if (!data) continue
        total += data.byteLength
        files.push({ path: f.path, data })
        keys.push([f.path, f.key])
      } catch { /* fichier illisible : ignoré */ }
    }
    if (!first && !files.length && !removed.length) return
    synchronized = true
    for (const p of removed) known.delete(p)
    for (const [p, k] of keys) known.set(p, k)
    const ack = new Promise<void>((resolve) => { synced = resolve })
    worker.postMessage({ type: 'sync', files, removed }, files.map((f) => f.data))
    await ack
  }

  /** Installe des paquets (bouton « Installer ») ; ceux qui réussissent sont inscrits dans requirements.txt. */
  function install(specs: string[]) {
    const job = chain.then(async (): Promise<InstallResult> => {
      try { await start() } catch { return { ok: false, error: message.value } }
      state.value = 'busy'
      const id = `i${++seq}`
      const r = await new Promise<InstallResult>((resolve) => { installs.set(id, resolve); worker!.postMessage({ type: 'install', id, specs }) })
      if (state.value === 'busy') state.value = 'idle'
      message.value = ''
      return r
    })
    chain = job.catch(() => {})
    return job
  }

  /** Exécute une cellule ; `onUpdate` reçoit les sorties au fil de l'eau. */
  function run(code: string, onUpdate: (o: NbOutput[]) => void) {
    const job = chain.then(async () => {
      try {
        await start()
      } catch {
        return { ok: false, outputs: [{ output_type: 'error', ename: 'KernelError', evalue: message.value || 'Python n’a pas pu démarrer', traceback: [message.value || 'Python n’a pas pu démarrer'] } as NbOutput] }
      }
      state.value = 'busy'
      await syncFiles().catch(() => {})
      message.value = ''
      const id = `r${++seq}`
      const result = await new Promise<{ ok: boolean; outputs: NbOutput[] }>((resolve) => {
        const outputs = warnings
        warnings = []
        pending.set(id, { outputs, onUpdate, resolve })
        worker!.postMessage({ type: 'run', id, code })
        // Début de l'exécution (après celles en attente) : cellule « en cours », durée mesurée d'ici.
        onUpdate(outputs.slice())
      })
      if (state.value === 'busy') state.value = 'idle'
      return result
    })
    chain = job.catch(() => {})
    return job
  }

  /** Arrête le calcul en cours (ou remet à zéro) : le noyau repart de rien. */
  function restart() {
    worker?.terminate()
    worker = null
    readyP = null
    known = new Map()
    synchronized = false
    synced?.()
    synced = null
    warnings = []
    for (const [, done] of installs) done({ ok: false, error: 'interrupted' })
    installs.clear()
    for (const [, p] of pending) p.resolve({ ok: false, outputs: [...p.outputs, { output_type: 'error', ename: 'KeyboardInterrupt', evalue: 'Exécution interrompue', traceback: ['Exécution interrompue'] }] })
    pending.clear()
    chain = Promise.resolve()
    state.value = 'off'
    message.value = ''
  }

  onBeforeUnmount(() => { worker?.terminate(); worker = null })
  return { state, message, run, restart, start, install }
}
