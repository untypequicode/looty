<script setup lang="ts">
import * as Y from 'yjs'
import { HocuspocusProvider } from '@hocuspocus/provider'
import { Plus, Play, RotateCcw, Eraser, Type, Code2, CircleDot, Info, ListEnd, Square, Cpu, Server, ListTree, X } from 'lucide-vue-next'
import 'katex/dist/katex.min.css'
import { socketBase } from '~/composables/useRealtime'
import { useNotebookKernel, type NbOutput } from '~/composables/useNotebookKernel'
import { useJupyterKernel } from '~/composables/useJupyterKernel'
import type { KernelSpec } from '~/components/notebook/EngineDialog.vue'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { projectResolver, type NbAttachments } from '~/editor/notebookMarkdown'
import { planUploads, insertionText } from '~/editor/notebookFiles'
import { useProjectActions, type ProjectDetail } from '~/composables/useProjects'
import { api } from '~/composables/api'
import { toast } from '~/composables/useToast'
import { missingModule, packageFor, isProjectModule, mergeRequirements } from '../../../public/notebook-deps.js'
import { useLocalStorage } from '@vueuse/core'
import {
  cellJson, cellFields, clipboardText, parseClipboardCells, markdownHeadings, isSourceHidden, isOutputsHidden, withHidden, withTiming, executionMs,
  type NbCellJson,
} from '~/editor/notebookCells'

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

interface CellVM { key: string; id: string; ymap: Y.Map<any>; type: 'code' | 'markdown' | 'raw'; ytext: Y.Text; count: number | null; outputs: NbOutput[]; attachments: NbAttachments | null; meta: Record<string, any> }
const cells = shallowRef<CellVM[]>([])
function parseOutputs(v: unknown): NbOutput[] { try { const o = JSON.parse(String(v ?? '[]')); return Array.isArray(o) ? o : [] } catch { return [] } }
function parseMeta(v: unknown): Record<string, any> { try { const o = JSON.parse(String(v ?? '{}')); return o && typeof o === 'object' ? o : {} } catch { return {} } }
function parseAttachments(v: unknown): NbAttachments | null { if (typeof v !== 'string') return null; try { const o = JSON.parse(v); return o && typeof o === 'object' ? o : null } catch { return null } }
function sync() {
  const seen = new Map<string, number>()
  cells.value = ycells.toArray().map((m) => {
    const id = String(m.get('id') ?? '')
    const n = (seen.get(id) ?? 0) + 1
    seen.set(id, n)
    const type = (m.get('type') as CellVM['type']) || 'code'
    return { key: `${id}:${n}:${type}`, id, ymap: m, type, ytext: m.get('source') as Y.Text, count: (m.get('count') as number | null) ?? null, outputs: parseOutputs(m.get('outputs')), attachments: parseAttachments(m.get('attachments')), meta: parseMeta(m.get('meta')) }
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
/** Cellule dont l'éditeur a le focus (mode édition) ; aucune : mode commande. */
const editingId = ref<string | null>(null)
const cellRefs = new Map<string, any>()
const root = ref<HTMLElement | null>(null)
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
/** Copie d'une cellule (texte, sorties, métadonnées, pièces jointes) ; même identifiant si `keepId`. */
function copyCell(src: Y.Map<any>, keepId = false) {
  const extra: Record<string, unknown> = { type: src.get('type') || 'code', outputs: src.get('outputs') ?? '[]', count: src.get('count') ?? null, meta: src.get('meta') ?? '{}' }
  if (keepId) extra.id = src.get('id')
  if (typeof src.get('attachments') === 'string') extra.attachments = src.get('attachments')
  return makeCell('code', String(src.get('source')?.toString() ?? ''), extra)
}
/** Cellule nbformat (presse-papiers, cellule supprimée) → nouvelle cellule du document. */
function cellFromJson(j: NbCellJson, keepId = false) {
  const { source, ...fields } = cellFields(j)
  const m = makeCell('code', source, fields)
  if (keepId && j.id) m.set('id', String(j.id).slice(0, 64))
  return m
}
const indexOf = (id: string | null) => cells.value.findIndex((c) => c.id === id)
/** Où insérer : sous la cellule sélectionnée, sinon à la fin. */
const selectedIndex = () => {
  const i = indexOf(selectedId.value)
  return i < 0 ? cells.value.length : i + 1
}
function select(id: string | null, scroll = false) {
  selectedId.value = id
  if (scroll && id) nextTick(() => root.value?.querySelector(`[data-cell="${CSS.escape(id)}"]`)?.scrollIntoView({ block: 'nearest' }))
}
/** Mode commande : le focus revient au notebook (raccourcis clavier). */
function commandMode() {
  editingId.value = null
  nextTick(() => root.value?.focus({ preventScroll: true }))
}
function edit(id: string) { select(id); nextTick(() => cellRefs.get(id)?.focus()) }
function addAt(index: number, type: 'code' | 'markdown', focus = true) {
  if (ro.value) return
  const m = makeCell(type, '')
  ydoc.transact(() => ycells.insert(Math.max(0, Math.min(index, ycells.length)), [m]))
  if (focus) edit(m.get('id'))
  else select(m.get('id'), true)
}

// Cellules supprimées par cette personne (Z en mode commande) : pile locale, jamais partagée.
interface Deleted { cell: NbCellJson; index: number; prev: string | null; next: string | null }
const deleted: Deleted[] = []
function deleteCells(index: number, count = 1) {
  const list = cells.value.slice(index, index + count)
  if (!list.length) return
  for (const [k, c] of list.entries()) deleted.push({ cell: cellJson((key) => c.ymap.get(key)), index: index + k, prev: cells.value[index - 1]?.id ?? null, next: cells.value[index + count]?.id ?? null })
  if (deleted.length > 50) deleted.splice(0, deleted.length - 50)
  const after = cells.value[index + count]?.id ?? cells.value[index - 1]?.id ?? null
  ydoc.transact(() => {
    ycells.delete(index, list.length)
    if (!ycells.length) ycells.push([makeCell('code', '')])
  })
  if (list.some((c) => c.id === selectedId.value)) select(after ?? cells.value[0]?.id ?? null)
}
async function removeAt(index: number) {
  if (ro.value) return
  const c = cells.value[index]
  if (!c) return
  if (c.ytext.length > 80 && !(await confirmDialog({ title: t('notebook.deleteTitle'), message: t('notebook.deleteHint'), danger: true, confirmLabel: t('common.delete') }))) return
  deleteCells(index)
}
/** Rétablit la dernière cellule supprimée par cette personne, à sa place ou au plus près. */
function undoDelete() {
  const d = deleted.pop()
  if (!d || ro.value) return
  const prev = indexOf(d.prev)
  const next = indexOf(d.next)
  const at = prev >= 0 ? prev + 1 : next >= 0 ? next : Math.min(d.index, cells.value.length)
  const m = cellFromJson(d.cell, !cells.value.some((c) => c.id === d.cell.id))
  ydoc.transact(() => ycells.insert(at, [m]))
  select(m.get('id'), true)
}
/** Déplace une cellule (Y.Array ne déplace pas : copie réinsérée dans une transaction). */
function moveTo(from: number, to: number) {
  const c = cells.value[from]
  if (ro.value || !c || to < 0 || to >= cells.value.length || to === from) return
  const copy = copyCell(c.ymap, true)
  ydoc.transact(() => { ycells.delete(from, 1); ycells.insert(to, [copy]) })
  selectedId.value = c.id
}
function moveAt(index: number, dir: -1 | 1) { moveTo(index, index + dir) }
function retype(c: CellVM, type: 'code' | 'markdown') { if (!ro.value && c.type !== type) ydoc.transact(() => c.ymap.set('type', type)) }
function clearOutput(c: CellVM) {
  if (ro.value) return
  ydoc.transact(() => { c.ymap.set('outputs', '[]'); c.ymap.set('count', null) })
  delete local[c.id]
}
function clearAll() { for (const c of cells.value) if (c.type === 'code') clearOutput(c) }
function setMeta(c: CellVM, next: Record<string, unknown>) { if (!ro.value && c.ymap.doc) ydoc.transact(() => c.ymap.set('meta', JSON.stringify(next))) }
function toggleHidden(c: CellVM, key: 'source_hidden' | 'outputs_hidden') {
  setMeta(c, withHidden(c.meta, key, !(key === 'source_hidden' ? isSourceHidden(c.meta) : isOutputsHidden(c.meta))))
}

// --- Presse-papiers des cellules : JSON nbformat (liste de cellules) ---
/** Dernière copie, si le presse-papiers système n'est pas lisible (permission refusée). */
let clipboard: NbCellJson[] = []
const selectedCells = () => cells.value.filter((c) => c.id === selectedId.value)
function copyCells(e?: ClipboardEvent) {
  const list = selectedCells().map((c) => cellJson((k) => c.ymap.get(k)))
  if (!list.length) return false
  clipboard = list
  const text = clipboardText(list)
  if (e?.clipboardData) { e.clipboardData.setData('text/plain', text); e.preventDefault() } else navigator.clipboard?.writeText(text).catch(() => {})
  return true
}
function cutCells(e?: ClipboardEvent) {
  if (ro.value || !copyCells(e)) return
  const i = indexOf(selectedId.value)
  if (i >= 0) deleteCells(i)
}
/** Colle des cellules sous la sélection (nouveaux identifiants). */
function insertCells(list: NbCellJson[]) {
  if (ro.value || !list.length) return
  const at = selectedIndex()
  const maps = list.map((j) => cellFromJson(j))
  ydoc.transact(() => ycells.insert(Math.min(at, ycells.length), maps))
  select(maps[maps.length - 1]!.get('id'), true)
}
async function pasteCells(e?: ClipboardEvent) {
  if (ro.value) return
  let text: string | null
  if (e) {
    text = e.clipboardData?.getData('text/plain') ?? ''
    e.preventDefault()
  } else {
    // Presse-papiers illisible (permission refusée) : dernière copie faite ici.
    text = await (navigator.clipboard?.readText() ?? Promise.reject(new Error('no clipboard'))).catch(() => null)
    if (text === null) { insertCells(clipboard); return }
  }
  const list = parseClipboardCells(text)
  if (list) insertCells(list)
}
function duplicate(index: number) {
  const c = cells.value[index]
  if (ro.value || !c) return
  const m = copyCell(c.ymap)
  ydoc.transact(() => ycells.insert(index + 1, [m]))
  select(m.get('id'), true)
}

// --- Scinder et fusionner ---
function splitAt(index: number, pos: number) {
  const c = cells.value[index]
  if (ro.value || !c) return
  const text = c.ytext.toString()
  const after = text.slice(pos).replace(/^\n/, '')
  const m = makeCell(c.type === 'markdown' ? 'markdown' : 'code', after, { type: c.type })
  ydoc.transact(() => {
    c.ytext.delete(pos, text.length - pos)
    // Ligne vide laissée par la coupure : retirée.
    const before = c.ytext.toString()
    if (before.endsWith('\n')) c.ytext.delete(before.length - 1, 1)
    ycells.insert(index + 1, [m])
  })
  edit(m.get('id'))
}
/** Fusionne la cellule avec la suivante (sorties effacées, comme JupyterLab). */
function mergeBelow(index: number) {
  const c = cells.value[index]
  const n = cells.value[index + 1]
  if (ro.value || !c || !n) return
  const extra = n.ytext.toString()
  const attachments = c.attachments || n.attachments ? JSON.stringify({ ...(n.attachments ?? {}), ...(c.attachments ?? {}) }) : null
  ydoc.transact(() => {
    c.ytext.insert(c.ytext.length, (c.ytext.length && extra ? '\n' : '') + extra)
    if (attachments) c.ymap.set('attachments', attachments)
    if (c.type === 'code') { c.ymap.set('outputs', '[]'); c.ymap.set('count', null) }
    ycells.delete(index + 1, 1)
  })
  delete local[c.id]
  select(c.id, true)
}

// --- Réordonner par glisser-déposer (poignée dans la marge) ---
const dragId = ref<string | null>(null)
const dropAt = ref<{ id: string; after: boolean } | null>(null)
function dropCell() {
  const from = indexOf(dragId.value)
  const target = dropAt.value
  dragId.value = null
  dropAt.value = null
  if (from < 0 || !target) return
  let to = indexOf(target.id) + (target.after ? 1 : 0)
  if (to > from) to--
  moveTo(from, to)
}

// --- Mode commande (comme Jupyter) : actif quand aucune cellule n'est en édition ---
let lastD = 0
function onKey(e: KeyboardEvent) {
  if (editingId.value || e.defaultPrevented) return
  const tg = e.target as HTMLElement | null
  if (tg?.closest('input, textarea, select, [contenteditable="true"], .cm-editor, .dialog')) return
  if (e.ctrlKey || e.metaKey || e.altKey) return
  const i = indexOf(selectedId.value)
  const c = cells.value[i]
  // Verrouillage des majuscules : « A » vaut « a » ; Maj+M reste « M ».
  const key = e.key.length === 1 && !e.shiftKey ? e.key.toLowerCase() : e.key
  const go = (d: number) => { const n = cells.value[Math.max(0, Math.min(cells.value.length - 1, (i < 0 ? 0 : i + d)))]; if (n) select(n.id, true) }
  let done = true
  if (key === 'ArrowDown' || key === 'j') go(1)
  else if (key === 'ArrowUp' || key === 'k') go(-1)
  else if (key === 'Enter' && c) { if (ro.value && c.type === 'markdown') done = false; else edit(c.id) }
  else if (key === 'M' && e.shiftKey) mergeBelow(i)
  else if (e.shiftKey) done = false
  else if (key === 'a') addAt(i < 0 ? 0 : i, 'code', false)
  else if (key === 'b') addAt(i + 1, 'code', false)
  else if (key === 'm' && c) retype(c, 'markdown')
  else if (key === 'y' && c) retype(c, 'code')
  else if (key === 'c') copyCells()
  else if (key === 'x') cutCells()
  else if (key === 'v') void pasteCells()
  else if (key === 'z') undoDelete()
  else if (key === 'd' && i >= 0 && !ro.value) {
    if (Date.now() - lastD < 600) { lastD = 0; deleteCells(i) } else lastD = Date.now()
  }
  else done = false
  if (done) e.preventDefault()
}
/** Ctrl+C / Ctrl+X / Ctrl+V en mode commande : cellules entières. */
function onClipboard(e: ClipboardEvent) {
  if (editingId.value || !selectedId.value) return
  const tg = e.target as HTMLElement | null
  if (tg?.closest('input, textarea, [contenteditable="true"], .cm-editor')) return
  if (e.type === 'copy') copyCells(e)
  else if (e.type === 'cut') cutCells(e)
  else if (e.type === 'paste') void pasteCells(e)
}

// --- Sommaire : titres des cellules Markdown ---
const tocOpen = useLocalStorage('looty.notebook.toc', false)
const toc = computed(() => cells.value.flatMap((c) => (c.type === 'markdown' ? markdownHeadings(c.ytext.toString()).map((h, n) => ({ ...h, cell: c.id, n, key: `${c.id}:${n}` })) : [])))
// Les titres suivent le texte des cellules (frappes non suivies par sync) : recalcul après une pause.
let tocTimer: ReturnType<typeof setTimeout> | undefined
ycells.observeDeep((events) => { if (tocOpen.value && events.some((e) => e.target instanceof Y.Text)) { clearTimeout(tocTimer); tocTimer = setTimeout(sync, 400) } })
function jumpTo(h: { cell: string; n: number }) {
  select(h.cell)
  nextTick(() => {
    const el = root.value?.querySelector(`[data-cell="${CSS.escape(h.cell)}"]`)
    const heading = el?.querySelectorAll('.cell-md :is(h1, h2, h3, h4, h5, h6)')[h.n]
    ;(heading ?? el)?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  })
}

// --- Fichiers du projet dans les cellules (ADR 0006) ---
const projectActions = useProjectActions(computed(() => props.projectId))
const filesByPath = computed(() => new Map((props.projectFiles ?? []).filter((f) => !f.is_dir).map((f) => [f.path, f])))
/** Images et liens relatifs des cellules Markdown : fichiers du projet. */
const resolveLink = computed(() => projectResolver(props.path, filesByPath.value.keys(), ([p]) => {
  const f = p ? filesByPath.value.get(p) : undefined
  return f ? `/api/projects/${props.projectId}/files/${f.id}/raw?v=${f.size}` : null
}))
/** Fichiers collés ou glissés sur une cellule : envoyés dans le projet, texte à insérer en retour. */
async function uploadForCell(c: CellVM, files: File[], pasted: boolean): Promise<string | null> {
  if (ro.value) return null
  const kind = c.type === 'code' ? 'code' : 'markdown'
  const planned = planUploads(files, { notebookPath: props.path, cell: kind, pasted, existing: filesByPath.value.keys() })
  try {
    for (const folder of new Set(planned.map((p) => p.folder))) {
      await projectActions.upload(planned.filter((p) => p.folder === folder).map((p) => new File([p.file], p.name, { type: p.file.type })), folder)
    }
  } catch { return null /* toast par api() */ }
  return insertionText(planned, kind, props.path)
}

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
const kernelFileOk = (f: { path: string; size: number; is_dir: boolean }) => !f.is_dir && !/\.ipynb$/i.test(f.path) && f.size <= 5_000_000
/** Fichiers du projet vus par le code, relus avant chaque exécution (version enregistrée, voir kernelRead). */
const kernelFiles = new Map<string, string>()
const REQUIREMENTS = 'requirements.txt'
/** Écritures de requirements.txt l'une après l'autre (lecture puis fusion puis envoi). */
let reqChain: Promise<unknown> = Promise.resolve()
/** Liste du projet : relue avant chaque exécution ; requête partagée si déjà en vol, et une seule pour tout « Tout exécuter ». */
let listing: { pending: boolean; p: Promise<ProjectDetail> } | null = null
let listingBatch = false
function projectListing(fresh = false) {
  if (!fresh && listing && (listing.pending || listingBatch)) return listing.p
  const p = api<ProjectDetail>(`/api/projects/${props.projectId}`, { silent: true })
  const entry = { pending: true, p }
  listing = entry
  p.then(() => { entry.pending = false }, () => { if (listing === entry) listing = null })
  return p
}
/** Ajoute des paquets à requirements.txt du projet (fichier ordinaire, créé ou modifié par l'API des fichiers). */
async function writeRequirements(specs: string[], retry: boolean): Promise<void> {
  const detail = await projectListing(true)
  const file = detail.files.find((f) => !f.is_dir && f.path === REQUIREMENTS)
  if (!file) {
    const { text, added } = mergeRequirements('', specs)
    if (!added.length) return
    try {
      await api(`/api/projects/${props.projectId}/files`, { method: 'POST', body: { path: REQUIREMENTS, is_dir: false, content: text }, silent: true })
    } catch (e) {
      // Fichier créé entre-temps par quelqu'un d'autre : on relit et on fusionne une fois.
      if (retry) return writeRequirements(specs, false)
      throw e
    }
    return
  }
  const r = await fetch(`/api/projects/${props.projectId}/files/${file.id}/raw`, { credentials: 'same-origin' })
  if (!r.ok) throw new Error(`requirements.txt: ${r.status}`)
  const { text, added } = mergeRequirements(await r.text(), specs)
  if (!added.length) return
  await api(`/api/projects/${props.projectId}/files/${file.id}`, { method: 'PUT', body: { content: text }, silent: true })
}
function addRequirements(specs: string[]) {
  if (ro.value || !specs.length) return Promise.resolve()
  const job = reqChain.then(async () => {
    try {
      await writeRequirements(specs, true)
    } catch {
      toast.error(t('notebook.deps.saveFailed', { pkg: specs.join(', ') }), { duration: 8000 })
    } finally {
      listing = null
    }
    await projectActions.refresh().catch(() => {})
  })
  reqChain = job.catch(() => {})
  return job.catch(() => {})
}
const pyodide = useNotebookKernel({
  list: async () => {
    const detail = await projectListing()
    kernelFiles.clear()
    const out: { path: string; key: string; size: number }[] = []
    for (const f of detail.files.filter(kernelFileOk).slice(0, 200)) {
      kernelFiles.set(f.path, f.id)
      out.push({ path: f.path, key: `${f.id}:${f.size}:${f.updated_at}`, size: f.size })
    }
    return out
  },
  read: async (path) => {
    const id = kernelFiles.get(path)
    if (!id) return null
    const r = await fetch(`/api/projects/${props.projectId}/files/${id}/raw`, { credentials: 'same-origin' })
    return r.ok ? r.arrayBuffer() : null
  },
  onInstalled: addRequirements,
})
const jupyter = useJupyterKernel({ projectId: () => props.projectId, kernel: () => nbKernelName.value })
// Moteur changé (ici ou par quelqu'un d'autre) : le noyau distant est arrêté, rien ne bascule en silence.
watch([engine, nbKernelName], ([e], [was, wasKernel]) => {
  if (was === 'jupyter' && (e !== 'jupyter' || nbKernelName.value !== wasKernel)) { jupyter.shutdown(); running.value = null; queued.value = new Set() }
})
const kernel = computed(() => (remote.value ? jupyter : pyodide))
/** Sorties affichées pendant le calcul (et, en lecture seule, gardées sur cet appareil). */
const local = reactive<Record<string, { outputs: NbOutput[]; count: number | null; ms?: number | null }>>({})
const running = ref<string | null>(null)
const queued = ref<Set<string>>(new Set())
const shown = (c: CellVM) => local[c.id]?.outputs ?? c.outputs
const shownCount = (c: CellVM) => (local[c.id] ? local[c.id]!.count : c.count)
/** Durée de la dernière exécution (pendant le calcul : aucune). */
const shownMs = (c: CellVM) => (running.value === c.id || queued.value.has(c.id) ? null : local[c.id] ? local[c.id]!.ms ?? null : executionMs(c.meta))
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
  let started: Date | null = null
  const res: { ok: boolean; outputs: NbOutput[]; count?: number | null } = await kernel.value.run(code, (o) => { started ??= new Date(); running.value = c.id; queued.value.delete(c.id); queued.value = new Set(queued.value); local[c.id] = { outputs: o, count: null } })
  running.value = null
  queued.value.delete(c.id)
  queued.value = new Set(queued.value)
  const count = res.count ?? nextCount()
  const outputs = res.outputs.map((o) => (o.output_type === 'execute_result' ? { ...o, execution_count: count } : o))
  const timing = withTiming(parseMeta(c.ymap.get('meta')), started ?? new Date(), new Date())
  if (!ro.value && c.ymap.doc) {
    ydoc.transact(() => { c.ymap.set('outputs', JSON.stringify(capped(outputs))); c.ymap.set('count', count); c.ymap.set('meta', JSON.stringify(timing)) })
    delete local[c.id]
  } else {
    local[c.id] = { outputs, count, ms: executionMs(timing) }
  }
  return res.ok
}
/** Bibliothèque manquante d'une cellule (Pyodide, modification permise) : { module, pkg } pour le bouton « Installer ». */
function missingOf(c: CellVM): { module: string; pkg: string } | null {
  if (c.type !== 'code' || remote.value || ro.value) return null
  for (const o of shown(c)) {
    const module = missingModule(o)
    if (module && !isProjectModule(module, [...filesByPath.value.keys()])) return { module, pkg: packageFor(module) }
  }
  return null
}
const installing = ref<string | null>(null)
/** « Installer » : micropip, puis requirements.txt (via le noyau), puis la cellule est relancée. */
async function installMissing(c: CellVM) {
  const miss = missingOf(c)
  if (!miss || installing.value) return
  installing.value = c.id
  try {
    const res = await pyodide.install([miss.pkg])
    if (res.ok) { await runCell(c); return }
    const kind = res.wasm ? 'WasmUnavailableError' : 'PipError'
    const err: NbOutput = { output_type: 'error', ename: kind, evalue: res.error ?? '', traceback: [`${kind}: ${res.error ?? ''}`] }
    const outputs = [...c.outputs.filter((o) => o.output_type !== 'error'), err]
    if (c.ymap.doc) ydoc.transact(() => c.ymap.set('outputs', JSON.stringify(capped(outputs))))
  } finally {
    installing.value = null
  }
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
  listing = null
  listingBatch = true
  try {
    for (const c of list) {
      const ok = await runCell(c)
      if (!ok) { queued.value = new Set(); break }
    }
  } finally {
    listingBatch = false
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

onBeforeUnmount(() => { clearTimeout(tocTimer); provider.destroy(); ydoc.destroy() })
defineExpose({ getText: () => '', goToLine: (_n: number) => {}, focus: () => {} })
</script>

<template>
  <div class="nb" :class="{ ro }">
    <div class="nb-bar">
      <UiTooltip :text="t('notebook.toc')"><UiButton size="xs" variant="ghost" :icon="ListTree" :aria-label="t('notebook.toc')" :aria-pressed="tocOpen" :class="{ on: tocOpen }" data-testid="nb-toc-toggle" @click="tocOpen = !tocOpen" /></UiTooltip>
      <span class="nb-sep" />
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
      <UiTooltip :text="t('notebook.shortcutsHint')"><span class="nb-mode" :class="{ edit: !!editingId }" data-testid="nb-mode">{{ editingId ? t('notebook.modeEdit') : t('notebook.modeCommand') }}</span></UiTooltip>
      <UiButton size="xs" variant="ghost" class="nb-engine" :icon="remote ? Server : Cpu" :title="t('notebook.jupyter.title')" data-testid="nb-engine" @click="engineOpen = true">{{ remote ? t('notebook.jupyter.remoteShort') : t('notebook.jupyter.pyodideShort') }}</UiButton>
      <UiTooltip :text="remote ? t('notebook.jupyter.remoteHint') : t('notebook.pyodideHint')"><Info :size="13" class="nb-info" /></UiTooltip>
      <span class="nb-kernel" :class="kernelState" role="status" data-testid="nb-kernel" :title="statusText"><CircleDot :size="11" /> {{ statusText }}</span>
    </div>
    <div class="nb-body">
      <aside v-if="tocOpen" class="nb-toc" :aria-label="t('notebook.toc')" data-testid="nb-toc">
        <header><b>{{ t('notebook.toc') }}</b><UiButton size="xs" variant="ghost" :icon="X" :aria-label="t('common.close')" @click="tocOpen = false" /></header>
        <button v-for="h in toc" :key="h.key" class="nb-toc-item" :class="{ on: h.cell === selectedId }" :style="{ paddingLeft: `${8 + (h.level - 1) * 12}px` }" @click="jumpTo(h)">{{ h.text }}</button>
        <p v-if="!toc.length" class="nb-toc-empty">{{ t('notebook.tocEmpty') }}</p>
      </aside>
      <div
        ref="root" class="nb-scroll" tabindex="-1" @click.self="selectedId = null" @keydown="onKey" @copy="onClipboard" @cut="onClipboard" @paste="onClipboard"
        @focusin="(e: FocusEvent) => { const el = (e.target as HTMLElement).closest('.cm-editor') ? (e.target as HTMLElement).closest('[data-cell]') : null; editingId = el?.getAttribute('data-cell') ?? null }"
        @focusout="(e: FocusEvent) => { if (!(e.relatedTarget as HTMLElement | null)?.closest?.('.cm-editor')) editingId = null }"
      >
        <div v-if="!synced" class="nb-loading"><UiSpinner /></div>
        <template v-else>
          <NotebookCell
            v-for="(c, i) in cells" :key="c.key" :ref="(el: any) => { if (el) cellRefs.set(c.id, el); else cellRefs.delete(c.id) }"
            :id="c.id" :type="c.type" :ytext="c.ytext" :attachments="c.attachments" :count="shownCount(c)" :outputs="shown(c)" :running="running === c.id" :queued="queued.has(c.id) && running !== c.id"
            :selected="selectedId === c.id" :read-only="ro" :awareness="provider.awareness"
            :resolve="resolveLink" :on-files="ro ? null : (fl: File[], pasted: boolean) => uploadForCell(c, fl, pasted)"
            :source-hidden="isSourceHidden(c.meta)" :outputs-hidden="isOutputsHidden(c.meta)" :duration-ms="c.type === 'code' ? shownMs(c) : null"
            :missing="missingOf(c)" :installing="installing === c.id" @install="installMissing(c)"
            :dragging="dragId === c.id" :drop-edge="dropAt?.id === c.id ? (dropAt.after ? 'after' : 'before') : null"
            @select="select(c.id)" @run="(m) => runAt(i, m)" @add="(w, ty) => addAt(w === 'above' ? i : i + 1, ty)" @remove="removeAt(i)" @move="(d) => moveAt(i, d)" @clear="clearOutput(c)" @retype="(ty) => retype(c, ty)"
            @duplicate="duplicate(i)" @split="(pos) => splitAt(i, pos)" @escape="commandMode" @toggle="(k) => toggleHidden(c, k)"
            @drag-start="dragId = c.id" @drag-over="(after) => { if (dragId) dropAt = { id: c.id, after } }" @drag-drop="dropCell" @drag-end="dragId = null; dropAt = null"
          />
          <div v-if="!ro" class="nb-end">
            <UiButton size="sm" variant="ghost" :icon="Plus" @click="addAt(cells.length, 'code')">{{ t('notebook.addCode') }}</UiButton>
            <UiButton size="sm" variant="ghost" :icon="ListEnd" @click="addAt(cells.length, 'markdown')">{{ t('notebook.addText') }}</UiButton>
          </div>
        </template>
      </div>
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
.nb-body { flex: 1; min-height: 0; display: flex; }
.nb-scroll { flex: 1; min-width: 0; min-height: 0; overflow-y: auto; padding: 18px 12px 80px 0; display: flex; flex-direction: column; gap: 6px; outline: none; }
.nb-toc { flex: none; width: 230px; overflow-y: auto; padding: 8px 6px; border-right: 1px solid var(--border); background: var(--bg-panel); }
.nb-toc header { display: flex; align-items: center; justify-content: space-between; padding: 0 2px 6px 8px; font-size: var(--fs-sm); }
.nb-toc-item { display: block; width: 100%; padding: 4px 8px; border-radius: var(--r-sm); text-align: left; font-size: var(--fs-sm); color: var(--text-2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.nb-toc-item:hover { background: var(--bg-hover); color: var(--text); }
.nb-toc-item.on { color: var(--text); background: var(--bg-active); }
.nb-toc-empty { margin: 4px 8px; font-size: var(--fs-xs); color: var(--text-3); }
.nb-bar :deep(.on) { background: var(--bg-active); }
.nb-mode { margin-right: 6px; padding: 1px 8px; border-radius: var(--r-full); font-size: var(--fs-xs); color: var(--text-3); background: var(--bg-hover); cursor: help; }
.nb-mode.edit { color: var(--accent-text, var(--accent)); background: var(--accent-soft); }
.nb-loading { display: grid; place-items: center; padding: 40px; }
.nb-end { display: flex; gap: 6px; padding-left: 68px; margin-top: 8px; }
</style>
