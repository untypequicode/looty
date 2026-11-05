import { onBeforeUnmount, ref } from 'vue'
import { api, errorMessage } from '~/composables/api'
import { socketBase } from '~/composables/useRealtime'
import { translate } from '~/composables/useI18n'
import type { NbOutput } from '~/composables/useNotebookKernel'

/**
 * Noyau distant d'un notebook : serveur Jupyter du projet, joint à travers le backend
 * (`/api/projects/<id>/jupyter/…`, qui ajoute le jeton : il ne passe jamais par le navigateur).
 * Même interface que le noyau Pyodide : un noyau par notebook ouvert et par personne,
 * démarré au premier calcul, reconnecté si la connexion tombe, arrêté à la fermeture.
 * Aucune erreur n'est masquée : connexion ou droits refusés deviennent une sortie d'erreur.
 */
export type JupyterState = 'off' | 'starting' | 'idle' | 'busy' | 'reconnecting' | 'error'
type Result = { ok: boolean; outputs: NbOutput[]; count?: number | null }

interface Pending {
  outputs: NbOutput[]
  count: number | null
  failed: boolean
  clearNext: boolean
  started: boolean
  onUpdate: (outputs: NbOutput[]) => void
  resolve: (r: Result) => void
}

const msgId = () => (globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(36).slice(2)}`)

export function useJupyterKernel(opts: { projectId: () => string; kernel: () => string | undefined }) {
  const state = ref<JupyterState>('off')
  const message = ref('')
  const kernelName = ref('')
  let kernelId: string | null = null
  let ws: WebSocket | null = null
  let connectP: Promise<void> | null = null
  let closing = false
  let retries = 0
  let epoch = 0
  const session = msgId()
  const pending = new Map<string, Pending>()
  let chain: Promise<unknown> = Promise.resolve()
  const base = () => `/api/projects/${opts.projectId()}/jupyter/kernels`

  const fail = (text: string): NbOutput => ({ output_type: 'error', ename: 'JupyterError', evalue: text, traceback: [text] })
  function setError(text: string) {
    state.value = 'error'
    message.value = text
  }
  /** Les calculs en attente se terminent sur une erreur visible (jamais en silence). */
  function abortPending(text: string, ename = 'JupyterError') {
    for (const [, p] of pending) p.resolve({ ok: false, count: p.count, outputs: [...p.outputs, { ...fail(text), ename }] })
    pending.clear()
  }

  function onMessage(raw: string) {
    let m: any
    try { m = JSON.parse(raw) } catch { return }
    const type: string = m?.header?.msg_type ?? m?.msg_type ?? ''
    const parent: string | undefined = m?.parent_header?.msg_id
    const c = m?.content ?? {}
    if (type === 'status') {
      if (c.execution_state === 'busy') state.value = 'busy'
      else if (c.execution_state === 'idle' && state.value !== 'error') state.value = 'idle'
      else if (c.execution_state === 'restarting' || c.execution_state === 'starting') state.value = 'starting'
      else if (c.execution_state === 'dead') setError(translate('notebook.jupyter.dead'))
    }
    const p = parent ? pending.get(parent) : undefined
    if (!p) return
    const push = (o: NbOutput) => {
      if (p.clearNext) { p.outputs = []; p.clearNext = false }
      p.outputs.push(o)
      p.onUpdate(p.outputs.slice())
    }
    switch (type) {
      case 'status':
        if (c.execution_state === 'busy' && !p.started) { p.started = true; p.onUpdate(p.outputs.slice()) }
        break
      case 'execute_input':
        p.count = typeof c.execution_count === 'number' ? c.execution_count : p.count
        if (!p.started) { p.started = true; p.onUpdate(p.outputs.slice()) }
        break
      case 'stream': {
        if (p.clearNext) { p.outputs = []; p.clearNext = false }
        const last = p.outputs[p.outputs.length - 1]
        if (last?.output_type === 'stream' && last.name === c.name) last.text = String(last.text ?? '') + String(c.text ?? '')
        else p.outputs.push({ output_type: 'stream', name: c.name, text: String(c.text ?? '') })
        p.onUpdate(p.outputs.slice())
        break
      }
      case 'execute_result':
        push({ output_type: 'execute_result', data: c.data ?? {}, metadata: c.metadata ?? {}, execution_count: c.execution_count ?? null })
        break
      case 'display_data':
        push({ output_type: 'display_data', data: c.data ?? {}, metadata: { ...(c.metadata ?? {}), ...(c.transient?.display_id ? { looty_display_id: c.transient.display_id } : {}) } })
        break
      case 'update_display_data': {
        const id = c.transient?.display_id
        const i = id ? p.outputs.findIndex((o) => o.metadata?.looty_display_id === id) : -1
        if (i >= 0) { p.outputs[i] = { ...p.outputs[i]!, data: c.data ?? {} }; p.onUpdate(p.outputs.slice()) }
        break
      }
      case 'clear_output':
        if (c.wait) p.clearNext = true
        else { p.outputs = []; p.onUpdate([]) }
        break
      case 'error':
        p.failed = true
        push({ output_type: 'error', ename: c.ename, evalue: c.evalue, traceback: c.traceback ?? [] })
        break
      case 'execute_reply':
        p.count = typeof c.execution_count === 'number' ? c.execution_count : p.count
        if (c.status && c.status !== 'ok') p.failed = true
        if (c.status === 'aborted' && !p.outputs.some((o) => o.output_type === 'error')) p.outputs.push({ ...fail(translate('notebook.jupyter.aborted')), ename: 'Aborted' })
        break
    }
    // Toutes les sorties d'une exécution précèdent le « idle » qui la clôt.
    if (type === 'status' && c.execution_state === 'idle' && p.started) {
      pending.delete(parent!)
      p.resolve({ ok: !p.failed, outputs: p.outputs, count: p.count })
    }
  }

  /** Erreur explicite du serveur : l'échec d'un WebSocket ne dit rien, l'état du noyau si. */
  async function explain(): Promise<string> {
    if (!kernelId) return translate('notebook.jupyter.lost')
    try {
      await api(`${base()}/${kernelId}`, { silent: true })
      return translate('notebook.jupyter.lost')
    } catch (e: any) {
      // 404 : noyau inconnu du backend (redémarré) ou disparu de Jupyter → en démarrer un autre.
      if ((e?.status ?? e?.statusCode) === 404 || e?.data?.error === 'jupyter_kernel_gone') {
        kernelId = null
        return translate('errors.jupyter_kernel_gone')
      }
      return errorMessage(e)
    }
  }

  function openSocket(): Promise<void> {
    return new Promise((resolve, reject) => {
      const sock = new WebSocket(`${socketBase()}${base()}/${kernelId}/channels`)
      let opened = false
      sock.onopen = () => { opened = true; retries = 0; ws = sock; if (state.value === 'reconnecting' || state.value === 'starting') state.value = 'idle'; message.value = ''; resolve() }
      sock.onmessage = (e) => { if (typeof e.data === 'string') onMessage(e.data) }
      sock.onclose = async () => {
        if (ws === sock) ws = null
        if (closing) return
        if (!opened) { reject(new Error(await explain())); return }
        // Connexion perdue : le noyau tourne toujours sur le serveur, on s'y reconnecte.
        reconnect()
      }
    })
  }

  async function reconnect() {
    if (closing || !kernelId) return
    state.value = 'reconnecting'
    message.value = ''
    while (!closing && kernelId && retries < 5) {
      retries++
      await new Promise((r) => setTimeout(r, Math.min(8000, 500 * 2 ** retries)))
      try { connectP = openSocket(); await connectP; return } catch (e: any) {
        connectP = null
        if (!kernelId) break // noyau disparu : inutile d'insister
        message.value = e?.message ?? ''
      }
    }
    connectP = null
    const text = message.value || (await explain())
    setError(text)
    abortPending(text)
  }

  async function start(): Promise<void> {
    if (ws && ws.readyState === WebSocket.OPEN) return
    if (connectP) return connectP
    closing = false
    connectP = (async () => {
      state.value = 'starting'
      message.value = translate('notebook.jupyter.connecting')
      try {
        if (!kernelId) {
          const k = await api<{ id: string; name: string }>(base(), { method: 'POST', body: { kernel: opts.kernel() || undefined }, silent: true })
          kernelId = k.id
          kernelName.value = k.name
        }
        await openSocket()
      } catch (e: any) {
        const text = e instanceof Error && !(e as any).data ? e.message : errorMessage(e)
        setError(text)
        throw new Error(text)
      }
    })()
    try { await connectP } finally { connectP = null }
  }

  function send(code: string): string {
    const id = msgId()
    ws!.send(JSON.stringify({
      header: { msg_id: id, msg_type: 'execute_request', session, username: 'looty', date: new Date().toISOString(), version: '5.3' },
      parent_header: {}, metadata: {}, channel: 'shell', buffers: [],
      content: { code, silent: false, store_history: true, user_expressions: {}, allow_stdin: false, stop_on_error: true },
    }))
    return id
  }

  /** Exécute une cellule sur le noyau distant ; `onUpdate` reçoit les sorties au fil de l'eau. */
  function run(code: string, onUpdate: (o: NbOutput[]) => void): Promise<Result> {
    const mine = epoch
    const job = chain.then(async (): Promise<Result> => {
      if (mine !== epoch) return { ok: false, outputs: [{ ...fail(translate('notebook.jupyter.interrupted')), ename: 'KeyboardInterrupt' }] }
      try { await start() } catch (e: any) { return { ok: false, outputs: [fail(e?.message || translate('notebook.jupyter.lost'))] } }
      if (!ws || ws.readyState !== WebSocket.OPEN) return { ok: false, outputs: [fail(message.value || translate('notebook.jupyter.lost'))] }
      return new Promise<Result>((resolve) => {
        const id = send(code)
        pending.set(id, { outputs: [], count: null, failed: false, clearNext: false, started: false, onUpdate, resolve })
      })
    })
    chain = job.catch(() => {})
    return job
  }

  /** Interrompt le calcul en cours (le noyau garde ses variables) et vide la file. */
  async function interrupt() {
    epoch++
    if (!kernelId) return
    try { await api(`${base()}/${kernelId}/interrupt`, { method: 'POST', silent: true }) } catch (e) { setError(errorMessage(e)); abortPending(message.value) }
  }

  /** Redémarre le noyau distant (variables perdues). Après une erreur : nouvelle tentative de connexion. */
  async function restart() {
    epoch++
    abortPending(translate('notebook.jupyter.restarted'), 'KeyboardInterrupt')
    chain = Promise.resolve()
    if (state.value === 'error' || !kernelId) {
      closing = true
      ws?.close()
      ws = null
      closing = false
      retries = 0
      state.value = 'off'
      message.value = ''
      if (kernelId) { try { await start() } catch { /* état d'erreur déjà affiché */ } }
      return
    }
    state.value = 'starting'
    message.value = translate('notebook.jupyter.restarting')
    try {
      await api(`${base()}/${kernelId}/restart`, { method: 'POST', silent: true })
      state.value = 'idle'
      message.value = ''
    } catch (e) {
      setError(errorMessage(e))
    }
  }

  /** Ferme la connexion et arrête le noyau distant (changement de moteur, fermeture). */
  function shutdown() {
    closing = true
    epoch++
    abortPending(translate('notebook.jupyter.stopped'))
    chain = Promise.resolve()
    ws?.close()
    ws = null
    connectP = null
    if (kernelId) fetch(`${base()}/${kernelId}`, { method: 'DELETE', credentials: 'include', keepalive: true }).catch(() => {})
    kernelId = null
    state.value = 'off'
    message.value = ''
  }

  onBeforeUnmount(shutdown)
  return { state, message, kernelName, run, restart, interrupt, start, shutdown }
}
