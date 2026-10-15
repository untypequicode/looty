import { onBeforeUnmount, ref } from 'vue'

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

export type KernelFile = { path: string; data: ArrayBuffer }

export function useNotebookKernel(opts: { signature?: () => string; files?: () => Promise<KernelFile[]> } = {}) {
  const state = ref<KernelState>('off')
  const message = ref('')
  let worker: Worker | null = null
  let readyP: Promise<void> | null = null
  let filesSignature = ''
  const pending = new Map<string, Pending>()
  let seq = 0
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
          case 'status': message.value = d.text; break
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

  /** Fichiers du projet visibles par le code (lecture de .csv, etc.), remis à jour si besoin. */
  async function syncFiles() {
    if (!opts.files || !opts.signature || !worker) return
    // Les octets ne sont lus que si la liste des fichiers du projet a changé.
    const signature = opts.signature()
    if (signature === filesSignature) return
    filesSignature = signature
    const files = await opts.files()
    worker.postMessage({ type: 'files', files }, files.map((f) => f.data))
  }

  /** Exécute une cellule ; `onUpdate` reçoit les sorties au fil de l'eau. */
  function run(code: string, onUpdate: (o: NbOutput[]) => void) {
    const job = chain.then(async () => {
      try {
        await start()
      } catch {
        return { ok: false, outputs: [{ output_type: 'error', ename: 'KernelError', evalue: message.value || 'Python n’a pas pu démarrer', traceback: [message.value || 'Python n’a pas pu démarrer'] } as NbOutput] }
      }
      await syncFiles().catch(() => {})
      const id = `r${++seq}`
      state.value = 'busy'
      const result = await new Promise<{ ok: boolean; outputs: NbOutput[] }>((resolve) => {
        pending.set(id, { outputs: [], onUpdate, resolve })
        worker!.postMessage({ type: 'run', id, code })
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
    filesSignature = ''
    for (const [, p] of pending) p.resolve({ ok: false, outputs: [...p.outputs, { output_type: 'error', ename: 'KeyboardInterrupt', evalue: 'Exécution interrompue', traceback: ['Exécution interrompue'] }] })
    pending.clear()
    chain = Promise.resolve()
    state.value = 'off'
    message.value = ''
  }

  onBeforeUnmount(() => { worker?.terminate(); worker = null })
  return { state, message, run, restart, start }
}
