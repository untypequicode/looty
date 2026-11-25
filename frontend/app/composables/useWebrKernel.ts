import { onBeforeUnmount, ref } from 'vue'
import { translate } from './useI18n'
import type { KernelEntry, KernelState, NbOutput } from './useNotebookKernel'
import { fromJedi, toCodePoints, type CompletionOut } from '~/editor/notebookComplete'
import { R_HARNESS, parseCompletion, parsePackages, queryR, rCompleteCode, runR, type RunOutcome, type WebRLike } from '~/editor/webrEngine'

/**
 * Noyau R d'un notebook : WebR (R compilé en WebAssembly) dans le navigateur, ADR 0018.
 * Un noyau par notebook ouvert et par personne, démarré au premier calcul seulement : WebR n'est
 * ni téléchargé ni lancé avant. Moteur et ressources sont servis par l'application elle-même
 * (`/webr/`) ; le canal PostMessage évite SharedArrayBuffer (donc tout en-tête COOP/COEP).
 * Mêmes méthodes que `useNotebookKernel` (Pyodide) pour que l'éditeur n'ait qu'un moteur « actif ».
 */
const HOME = '/home/web_user'
const PACKAGES_FILE = 'r-packages.txt'
const MAX_TOTAL = 25_000_000

/** PNG en base64 d'un graphique (ImageBitmap du périphérique canvas de WebR). */
async function bitmapToPng(img: unknown): Promise<string> {
  const bmp = img as ImageBitmap
  const canvas = new OffscreenCanvas(bmp.width, bmp.height)
  canvas.getContext('2d')!.drawImage(bmp, 0, 0)
  const blob = await canvas.convertToBlob({ type: 'image/png' })
  const bytes = new Uint8Array(await blob.arrayBuffer())
  let bin = ''
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000))
  return btoa(bin)
}

export function useWebrKernel(opts: {
  list?: () => Promise<KernelEntry[]>
  read?: (path: string) => Promise<ArrayBuffer | null>
  /** Fabrique de l'instance WebR (tests, ou autre source) ; par défaut le paquet `webr` chargé à la demande. */
  create?: () => Promise<any>
} = {}) {
  const state = ref<KernelState>('off')
  const message = ref('')
  let webr: any = null
  let readyP: Promise<void> | null = null
  /** Incrémenté à chaque redémarrage : une exécution d'une vie antérieure n'écrit plus rien. */
  let epoch = 0
  let known = new Map<string, string>()
  let packagesDone = false
  let warnings: NbOutput[] = []
  let chain: Promise<unknown> = Promise.resolve()
  const interrupts = new Set<(r: RunOutcome) => void>()

  async function create() {
    if (opts.create) return opts.create()
    // Chargé à la demande : le moteur n'est pas dans le paquet principal de l'application.
    const { WebR, ChannelType } = await import('webr')
    return new WebR({ baseUrl: `${location.origin}/webr/`, channelType: ChannelType.PostMessage, REnv: { R_HOME: '/usr/lib/R', R_ENABLE_JIT: '0' } })
  }

  function start() {
    if (readyP) return readyP
    state.value = 'starting'
    message.value = translate('notebook.webr.loading')
    const mine = epoch
    readyP = (async () => {
      try {
        const w = await create()
        await w.init()
        if (mine !== epoch) { try { w.close() } catch { /* déjà arrêté */ } throw new Error('restarted') }
        await w.evalRVoid(R_HARNESS)
        webr = w
        state.value = 'idle'
        message.value = ''
      } catch (e: any) {
        if (mine === epoch) { state.value = 'off'; message.value = String(e?.message ?? e); readyP = null }
        throw e
      }
    })()
    readyP.catch(() => {})
    return readyP
  }

  const dirs = new Set<string>()
  async function mkdirs(dir: string) {
    let at = ''
    for (const part of dir.split('/').filter(Boolean)) {
      at += '/' + part
      if (dirs.has(at)) continue
      try { await webr.FS.mkdir(at) } catch { /* existe déjà */ }
      dirs.add(at)
    }
  }

  /** Fichiers du projet (.csv, .R…) → dossier de travail de R ; seuls ceux qui ont changé sont renvoyés. */
  async function syncFiles() {
    if (!opts.list || !opts.read || !webr) return
    let list: KernelEntry[]
    try { list = await opts.list() } catch { return }
    const present = new Set(list.map((f) => f.path))
    const sizes = new Map(list.map((f) => [f.path, f.size]))
    let total = [...known.keys()].reduce((n, p) => n + (sizes.get(p) ?? 0), 0)
    for (const p of [...known.keys()].filter((p) => !present.has(p))) {
      known.delete(p)
      try { await webr.FS.unlink(`${HOME}/${p}`) } catch { /* déjà absent */ }
    }
    for (const f of list) {
      if (known.get(f.path) === f.key || total + f.size > MAX_TOTAL) continue
      try {
        const data = await opts.read(f.path)
        if (!data) continue
        total += data.byteLength
        const path = `${HOME}/${f.path.replace(/^\/+/, '')}`
        await mkdirs(path.slice(0, path.lastIndexOf('/')))
        await webr.FS.writeFile(path, new Uint8Array(data))
        known.set(f.path, f.key)
      } catch { /* fichier illisible : ignoré */ }
    }
    if (!packagesDone) await installListed()
  }

  /** Paquets de r-packages.txt (installation explicite, une fois par noyau) ; un échec est signalé sans bloquer. */
  async function installListed() {
    packagesDone = true
    let text = ''
    try { text = new TextDecoder().decode(await webr.FS.readFile(`${HOME}/${PACKAGES_FILE}`)) } catch { return }
    for (const pkg of parsePackages(text)) {
      message.value = translate('notebook.webr.installing', { pkg })
      try { await webr.installPackages(pkg, { quiet: true }) } catch (e: any) {
        warnings.push({ output_type: 'stream', name: 'stderr', text: `${translate('notebook.webr.installFailed', { pkg })}\n${String(e?.message ?? e).trim().split('\n').pop()}\n` })
      }
    }
  }

  /** Exécute une cellule ; `onUpdate` signale le début (puis les sorties à la fin : R ne rend la main qu'après). */
  function run(code: string, onUpdate: (o: NbOutput[]) => void): Promise<RunOutcome> {
    const mine = epoch
    const interrupted = (): RunOutcome => ({ ok: false, outputs: [{ output_type: 'error', ename: 'KeyboardInterrupt', evalue: translate('notebook.webr.interrupted'), traceback: [translate('notebook.webr.interrupted')] }] })
    const job = chain.then(async (): Promise<RunOutcome> => {
      if (mine !== epoch) return interrupted()
      try { await start() } catch (e: any) {
        if (mine !== epoch) return interrupted()
        const text = message.value || String(e?.message ?? e) || translate('notebook.webr.failed')
        return { ok: false, outputs: [{ output_type: 'error', ename: 'KernelError', evalue: text, traceback: [text] }] }
      }
      state.value = 'busy'
      const lead = () => { const w = warnings; warnings = []; return w }
      onUpdate([])
      try { await syncFiles() } catch { /* fichiers non disponibles : la cellule s'exécute quand même */ }
      message.value = ''
      const prefix = lead()
      let stop!: (r: RunOutcome) => void
      const result = await new Promise<RunOutcome>((resolve) => {
        stop = resolve
        interrupts.add(resolve)
        runR(webr as WebRLike, code, bitmapToPng).then(resolve)
      })
      interrupts.delete(stop)
      if (mine !== epoch) return interrupted()
      if (state.value === 'busy') state.value = 'idle'
      return { ok: result.ok, outputs: [...prefix, ...result.outputs] }
    })
    chain = job.catch(() => {})
    return job
  }

  /**
   * Requête hors cellule des panneaux R. Silencieuse (par défaut) : pas de démarrage de WebR
   * pour l'occasion (`null` s'il n'est pas prêt), ni changement d'état. Sinon : comme une cellule,
   * avec graphiques, sans numéro d'exécution.
   */
  async function query(code: string, o: { silent?: boolean; timeoutMs?: number; onUpdate?: (o: NbOutput[]) => void } = {}): Promise<RunOutcome | null> {
    const silent = o.silent ?? true
    if (silent && (!webr || state.value === 'off' || state.value === 'starting')) return null
    if (!silent) return run(code, o.onUpdate ?? (() => {}))
    const mine = epoch
    const job = chain.then(() => (mine === epoch && webr ? queryR(webr as WebRLike, code) : null))
    chain = job.catch(() => {})
    if (!o.timeoutMs) return job
    return Promise.race([job, new Promise<RunOutcome>((resolve) => setTimeout(() => resolve({ ok: false, outputs: [{ output_type: 'error', ename: 'Timeout', evalue: translate('notebook.r.panel.timeout'), traceback: [translate('notebook.r.panel.timeout')] }] }), o.timeoutMs))])
  }

  /** Complétions R (utils) de la ligne courante ; seulement quand WebR est prêt et libre. */
  async function complete(code: string, offset: number): Promise<CompletionOut | null> {
    if (!webr || state.value !== 'idle') return null
    const pos = toCodePoints(code, offset)
    const before = Array.from(code).slice(0, pos).join('')
    const line = before.slice(before.lastIndexOf('\n') + 1)
    if (!line.trim()) return null
    const r = await Promise.race([
      queryR(webr as WebRLike, rCompleteCode(line)).then((x) => x.outputs[0]?.text).catch(() => null),
      new Promise<null>((resolve) => setTimeout(() => resolve(null), 3000)),
    ])
    if (typeof r !== 'string' || !r) return null
    return fromJedi(code, pos, parseCompletion(r))
  }

  /** Arrête le calcul (ou remet à zéro) : R est relancé de zéro au prochain calcul. WebR n'a pas d'interruption sans SharedArrayBuffer. */
  function restart() {
    epoch++
    try { webr?.close() } catch { /* déjà arrêté */ }
    webr = null
    readyP = null
    known = new Map()
    dirs.clear()
    packagesDone = false
    warnings = []
    for (const done of interrupts) done({ ok: false, outputs: [{ output_type: 'error', ename: 'KeyboardInterrupt', evalue: translate('notebook.webr.interrupted'), traceback: [translate('notebook.webr.interrupted')] }] })
    interrupts.clear()
    chain = Promise.resolve()
    state.value = 'off'
    message.value = ''
  }

  onBeforeUnmount(() => { epoch++; try { webr?.close() } catch { /* déjà arrêté */ } webr = null })
  return { state, message, run, restart, start, complete, query }
}
