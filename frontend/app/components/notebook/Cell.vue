<script setup lang="ts">
import * as Y from 'yjs'
import { Play, Loader2, ArrowUp, ArrowDown, Trash2, Eraser, Type, Code2, Plus, Copy, GripVertical, ChevronDown, ChevronRight, MoreHorizontal, Download } from 'lucide-vue-next'
import { EditorState, Prec } from '@codemirror/state'
import { EditorView, keymap, drawSelection, highlightSpecialChars } from '@codemirror/view'
import { defaultKeymap, indentWithTab } from '@codemirror/commands'
import { syntaxHighlighting, defaultHighlightStyle, bracketMatching, indentOnInput } from '@codemirror/language'
import { closeBrackets, closeBracketsKeymap, completionKeymap } from '@codemirror/autocomplete'
import { python } from '@codemirror/lang-python'
import { markdown } from '@codemirror/lang-markdown'
import { yCollab } from 'y-codemirror.next'
import type { NbOutput } from '~/composables/useNotebookKernel'
import { renderMarkdown, type NbAttachments } from '~/editor/notebookMarkdown'
import { hasFiles, markNotebookDrop } from '~/editor/notebookFiles'
import { useI18n } from '~/composables/useI18n'
import { formatDuration } from '~/editor/notebookCells'

/**
 * Une cellule de notebook : le texte est un Y.Text partagé (édition à plusieurs, curseurs),
 * les sorties viennent de la dernière exécution (de n'importe qui).
 */
const props = defineProps<{
  id: string
  type: 'code' | 'markdown' | 'raw'
  ytext: Y.Text
  attachments?: NbAttachments | null
  count: number | null
  outputs: NbOutput[]
  running: boolean
  queued: boolean
  selected: boolean
  readOnly: boolean
  awareness: any
  /** Chemin relatif (image, lien) → adresse du fichier du projet. */
  resolve?: ((href: string) => string | null) | null
  /** Envoi des fichiers collés ou glissés ; renvoie le texte à insérer. */
  onFiles?: ((files: File[], pasted: boolean) => Promise<string | null>) | null
  /** Entrée / sortie repliées (metadata.jupyter, comme JupyterLab). */
  sourceHidden?: boolean
  outputsHidden?: boolean
  /** Durée de la dernière exécution. */
  durationMs?: number | null
  /** Bibliothèque Python manquante (erreur ModuleNotFoundError) que le moteur peut installer. */
  missing?: { module: string; pkg: string } | null
  installing?: boolean
  dragging?: boolean
  dropEdge?: 'before' | 'after' | null
}>()
const emit = defineEmits<{
  select: []
  run: [mode: 'stay' | 'next' | 'below']
  add: [where: 'above' | 'below', type: 'code' | 'markdown']
  remove: []
  move: [dir: -1 | 1]
  clear: []
  retype: [type: 'code' | 'markdown']
  duplicate: []
  install: []
  split: [pos: number]
  escape: []
  toggle: [key: 'source_hidden' | 'outputs_hidden']
  dragStart: []
  dragOver: [after: boolean]
  dragDrop: []
  dragEnd: []
}>()
const { t } = useI18n()

const host = ref<HTMLElement | null>(null)
const editing = ref(props.type !== 'markdown')
const src = ref(props.ytext.toString())
const html = computed(() => (props.type === 'markdown' ? renderMarkdown(src.value, { attachments: props.attachments, resolve: props.resolve }) : ''))
const onText = () => { src.value = props.ytext.toString() }
props.ytext.observe(onText)
let view: EditorView | null = null
let undo: Y.UndoManager | null = null

const theme = EditorView.theme({
  '&': { fontSize: '13px', backgroundColor: 'transparent', color: 'var(--text)' },
  '.cm-scroller': { fontFamily: 'var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace)', lineHeight: '1.55' },
  '.cm-content': { padding: '8px 0', caretColor: 'var(--text)' },
  '&.cm-focused': { outline: 'none' },
  '.cm-selectionBackground, &.cm-focused .cm-selectionBackground, ::selection': { backgroundColor: 'color-mix(in srgb, var(--accent) 22%, transparent) !important' },
  '.cm-cursor': { borderLeftColor: 'var(--text)' },
  '.cm-tooltip': { backgroundColor: 'var(--bg-elevated)', border: '1px solid var(--border)', borderRadius: '6px' },
  '.cm-ySelectionInfo': { fontFamily: 'var(--font)', fontSize: '10.5px', padding: '1px 4px', borderRadius: '3px', opacity: '1', top: '-1.4em' },
})

function mount() {
  if (view || !host.value) return
  undo = new Y.UndoManager(props.ytext)
  const run = (mode: 'stay' | 'next' | 'below') => () => { emit('run', mode); return true }
  view = new EditorView({
    parent: host.value,
    state: EditorState.create({
      doc: props.ytext.toString(),
      extensions: [
        highlightSpecialChars(), drawSelection(), indentOnInput(), syntaxHighlighting(defaultHighlightStyle, { fallback: true }), bracketMatching(), closeBrackets(),
        EditorView.lineWrapping,
        Prec.highest(keymap.of([
          { key: 'Shift-Enter', run: run('next') }, { key: 'Mod-Enter', run: run('stay') }, { key: 'Alt-Enter', run: run('below') },
          { key: 'Escape', run: (v) => { v.contentDOM.blur(); if (props.type === 'markdown' && src.value.trim()) editing.value = false; emit('escape'); return true } },
        ])),
        keymap.of([...closeBracketsKeymap, ...defaultKeymap, ...completionKeymap, indentWithTab]),
        ...(props.type === 'code' ? [python()] : props.type === 'markdown' ? [markdown()] : []),
        yCollab(props.ytext, props.awareness, { undoManager: undo }),
        EditorState.readOnly.of(props.readOnly), EditorView.editable.of(!props.readOnly),
        theme,
        EditorView.domEventHandlers({
          focus: () => { emit('select') },
          // Ctrl+Maj+- : scinde la cellule au curseur (touche « - » du pavé numérique aussi).
          keydown: (e, v) => {
            if (props.readOnly || !(e.ctrlKey || e.metaKey) || !e.shiftKey || !(e.code === 'Minus' || e.code === 'NumpadSubtract' || e.key === '-' || e.key === '_')) return false
            e.preventDefault()
            emit('split', v.state.selection.main.head)
            return true
          },
        }),
      ],
    }),
  })
}
function unmount() { view?.destroy(); view = null; undo?.destroy(); undo = null }

onMounted(() => { if (editing.value) nextTick(mount) })
watch(editing, (e) => { if (e) nextTick(() => { mount(); view?.focus() }); else unmount() })
watch(() => props.readOnly, () => { unmount(); if (editing.value) nextTick(mount) })
onBeforeUnmount(() => { props.ytext.unobserve(onText); unmount() })

function startEdit() { if (!props.readOnly && props.type === 'markdown') editing.value = true }
function onFocusOut(e: FocusEvent) {
  // Texte rendu à nouveau quand on quitte une cellule Markdown non vide.
  if (props.type !== 'markdown' || !editing.value) return
  if (e.relatedTarget && (e.currentTarget as HTMLElement).contains(e.relatedTarget as Node)) return
  if (src.value.trim()) editing.value = false
}
// --- Fichiers collés ou glissés (ADR 0006) ---
const dropping = ref(false)
const acceptsFiles = () => !props.readOnly && !!props.onFiles && props.type !== 'raw'
const CELL_MIME = 'application/x-looty-cell'
const isCellDrag = (e: DragEvent) => Array.from(e.dataTransfer?.types ?? []).includes(CELL_MIME)
function onGripStart(e: DragEvent) {
  if (props.readOnly || !e.dataTransfer) return
  e.dataTransfer.setData(CELL_MIME, props.id)
  e.dataTransfer.effectAllowed = 'move'
  const el = (e.currentTarget as HTMLElement).closest('.cell')
  if (el) e.dataTransfer.setDragImage(el, 20, 20)
  emit('dragStart')
}
function onDragOver(e: DragEvent) {
  if (isCellDrag(e)) {
    e.preventDefault()
    markNotebookDrop(e)
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move'
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
    emit('dragOver', e.clientY > r.top + r.height / 2)
    return
  }
  if (!acceptsFiles() || !hasFiles(e.dataTransfer)) return
  e.preventDefault()
  markNotebookDrop(e)
  if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy'
  dropping.value = true
}
function onDrop(e: DragEvent) {
  dropping.value = false
  if (isCellDrag(e)) {
    e.preventDefault()
    e.stopPropagation()
    emit('dragDrop')
    return
  }
  if (!acceptsFiles() || !hasFiles(e.dataTransfer)) return
  // Avant l'éditeur (qui insérerait le texte des fichiers) et avant le dépôt global du projet.
  e.preventDefault()
  e.stopPropagation()
  markNotebookDrop(e)
  const at = view && editing.value ? view.posAtCoords({ x: e.clientX, y: e.clientY }) : null
  void insertFiles([...(e.dataTransfer?.files ?? [])], false, at)
}
function onPaste(e: ClipboardEvent) {
  if (!acceptsFiles()) return
  const files = [...(e.clipboardData?.files ?? [])]
  if (!files.length) return
  e.preventDefault()
  e.stopPropagation()
  void insertFiles(files, true, null)
}
/** Envoie les fichiers, puis insère le lien au curseur (à la fin d'une cellule rendue, qui passe en édition). */
async function insertFiles(files: File[], pasted: boolean, at: number | null) {
  if (!files.length || !props.onFiles) return
  let index = at ?? (view && editing.value ? view.state.selection.main.head : props.ytext.length)
  let prefix = ''
  if (!editing.value || !view) {
    index = props.ytext.length
    const cur = props.ytext.toString()
    prefix = cur && !cur.endsWith('\n') ? '\n\n' : ''
    editing.value = true
  }
  emit('select')
  const rel = Y.createRelativePositionFromTypeIndex(props.ytext, index)
  const text = await props.onFiles(files, pasted)
  if (!text || !props.ytext.doc) return
  const pos = Y.createAbsolutePositionFromRelativePosition(rel, props.ytext.doc)?.index ?? props.ytext.length
  props.ytext.doc.transact(() => props.ytext.insert(pos, prefix + text))
  await nextTick()
  if (view) {
    const end = Math.min(view.state.doc.length, pos + prefix.length + text.length)
    view.dispatch({ selection: { anchor: end } })
    view.focus()
  }
}
defineExpose({ focus: () => { if (props.type === 'markdown') editing.value = true; nextTick(() => view?.focus()) } })

// Sortie très longue : hauteur limitée (défilement), bouton pour la déplier.
const outEl = ref<HTMLElement | null>(null)
const outExpanded = ref(false)
const outOverflow = ref(false)
let ro: ResizeObserver | null = null
function measure() { const el = outEl.value; outOverflow.value = !!el && el.scrollHeight > el.clientHeight + 4 }
watch(outEl, (el) => {
  ro?.disconnect()
  if (!el || typeof ResizeObserver === 'undefined') return
  ro = new ResizeObserver(measure)
  ro.observe(el)
  if (el.firstElementChild) ro.observe(el.firstElementChild)
})
watch(() => props.outputs, () => nextTick(measure))
onBeforeUnmount(() => ro?.disconnect())
const duration = computed(() => (props.durationMs != null ? formatDuration(props.durationMs) : ''))
const firstLine = computed(() => src.value.split('\n').find((l) => l.trim())?.slice(0, 120) ?? '')

const prompt = computed(() => (props.running ? '*' : props.queued ? '…' : props.count ?? ' '))
const hasOutputs = computed(() => props.outputs.length > 0)
</script>

<template>
  <div
    class="cell" :class="[type, { selected, running, dropping, dragging, 'drop-before': dropEdge === 'before', 'drop-after': dropEdge === 'after' }]" :data-cell="id" @click="emit('select')" @focusout="onFocusOut"
    @dragover.capture="onDragOver" @dragleave="dropping = false" @drop.capture="onDrop" @paste.capture="onPaste"
  >
    <div class="cell-gutter">
      <span v-if="!readOnly" class="cell-grip" draggable="true" :title="t('notebook.dragHint')" :aria-label="t('notebook.dragHint')" @dragstart="onGripStart" @dragend="emit('dragEnd')" @click.stop><GripVertical :size="13" /></span>
      <button class="cell-fold" :aria-label="sourceHidden ? t('notebook.showInput') : t('notebook.hideInput')" :title="sourceHidden ? t('notebook.showInput') : t('notebook.hideInput')" :aria-expanded="!sourceHidden" @click.stop="emit('toggle', 'source_hidden')">
        <component :is="sourceHidden ? ChevronRight : ChevronDown" :size="12" />
      </button>
      <button v-if="type === 'code'" class="cell-run" :disabled="running" :aria-label="t('notebook.run')" :title="`${t('notebook.run')} (Shift+Enter)`" @click.stop="emit('select'); emit('run', 'stay')">
        <Loader2 v-if="running || queued" :size="14" class="spin" /><Play v-else :size="14" />
      </button>
      <span v-if="type === 'code'" class="cell-prompt">[{{ prompt }}]</span>
    </div>
    <div class="cell-main">
      <button v-if="sourceHidden" class="cell-hidden" :title="t('notebook.showInput')" @click.stop="emit('toggle', 'source_hidden')"><MoreHorizontal :size="14" /> <span class="truncate">{{ firstLine }}</span></button>
      <template v-else>
        <div v-if="editing" class="cell-input" :class="{ md: type === 'markdown' }"><div ref="host" class="cell-editor" /></div>
        <div v-else class="cell-md prose" @dblclick="startEdit" v-html="html" />
      </template>
      <template v-if="type === 'code' && hasOutputs">
        <button v-if="outputsHidden" class="cell-hidden" :title="t('notebook.showOutput')" @click.stop="emit('toggle', 'outputs_hidden')"><MoreHorizontal :size="14" /> {{ t('notebook.outputHidden') }}</button>
        <div v-else class="cell-out-wrap">
          <button class="cell-fold out" :aria-label="t('notebook.hideOutput')" :title="t('notebook.hideOutput')" @click.stop="emit('toggle', 'outputs_hidden')"><ChevronDown :size="12" /></button>
          <div ref="outEl" class="cell-out" :class="{ capped: !outExpanded }"><NotebookOutput :outputs="outputs" /></div>
          <button v-if="missing" class="cell-install" :disabled="installing" data-testid="nb-install" @click.stop="emit('install')">
            <Loader2 v-if="installing" :size="13" class="spin" /><Download v-else :size="13" /> {{ t('notebook.deps.install', { pkg: missing.pkg }) }}
          </button>
          <button v-if="outOverflow || outExpanded" class="cell-expand" @click.stop="outExpanded = !outExpanded">{{ outExpanded ? t('notebook.collapseOutput') : t('notebook.expandOutput') }}</button>
        </div>
      </template>
      <div v-if="type === 'code' && duration" class="cell-time" data-testid="cell-time">{{ t('notebook.duration', { time: duration }) }}</div>
    </div>
    <div v-if="!readOnly" class="cell-tools" @click.stop>
      <button :aria-label="t('notebook.addAbove')" :title="t('notebook.addAbove')" @click="emit('add', 'above', 'code')"><Plus :size="12" /><ArrowUp :size="11" /></button>
      <button :aria-label="t('notebook.addBelow')" :title="t('notebook.addBelow')" @click="emit('add', 'below', 'code')"><Plus :size="12" /><ArrowDown :size="11" /></button>
      <button :aria-label="t('notebook.moveUp')" :title="t('notebook.moveUp')" @click="emit('move', -1)"><ArrowUp :size="13" /></button>
      <button :aria-label="t('notebook.moveDown')" :title="t('notebook.moveDown')" @click="emit('move', 1)"><ArrowDown :size="13" /></button>
      <button :aria-label="t('notebook.duplicate')" :title="t('notebook.duplicate')" @click="emit('duplicate')"><Copy :size="13" /></button>
      <button v-if="type !== 'raw'" :aria-label="type === 'code' ? t('notebook.toText') : t('notebook.toCode')" :title="type === 'code' ? t('notebook.toText') : t('notebook.toCode')" @click="emit('retype', type === 'code' ? 'markdown' : 'code')"><component :is="type === 'code' ? Type : Code2" :size="13" /></button>
      <button v-if="type === 'code' && hasOutputs" :aria-label="t('notebook.clearOutput')" :title="t('notebook.clearOutput')" @click="emit('clear')"><Eraser :size="13" /></button>
      <button class="danger" :aria-label="t('common.delete')" :title="t('common.delete')" @click="emit('remove')"><Trash2 :size="13" /></button>
    </div>
  </div>
</template>

<style scoped>
.cell { position: relative; display: grid; grid-template-columns: 62px minmax(0, 1fr); gap: 6px; padding: 4px 8px 4px 0; border-left: 3px solid transparent; border-radius: 0 var(--r-sm) var(--r-sm) 0; }
.cell.dropping { box-shadow: inset 0 0 0 2px var(--accent); }
.cell-md :deep(img) { max-width: 100%; height: auto; }
.cell.selected { border-left-color: var(--accent); background: color-mix(in srgb, var(--accent-soft) 40%, transparent); }
.cell-gutter { display: flex; flex-wrap: wrap; justify-content: flex-end; align-content: flex-start; align-items: center; gap: 2px 0; padding: 6px 0 0 0; font-family: var(--font-mono, monospace); font-size: 11px; color: var(--text-3); user-select: none; }
.cell-run { display: grid; place-items: center; width: 24px; height: 22px; border-radius: var(--r-sm); color: var(--text-2); opacity: .55; }
.cell:hover .cell-run, .cell.selected .cell-run, .cell.running .cell-run { opacity: 1; }
.cell-run:hover { background: var(--bg-hover); color: var(--accent); }
.cell-prompt { width: 100%; padding-right: 4px; text-align: right; }
.cell-main { min-width: 0; display: flex; flex-direction: column; gap: 2px; }
.cell-input { border-radius: var(--r-md); background: var(--bg-panel); box-shadow: inset 0 0 0 1px var(--border); padding: 0 10px; min-width: 0; }
.cell.selected .cell-input { box-shadow: inset 0 0 0 1.5px var(--border-focus); }
.cell-input.md { background: transparent; }
.cell-editor { min-width: 0; }
.cell-md { padding: 6px 4px; min-height: 28px; cursor: text; }
.cell-md :deep(h1) { font-size: 1.6em; margin: .4em 0 .3em; }
.cell-md :deep(h2) { font-size: 1.3em; margin: .5em 0 .3em; }
.cell-md :deep(p) { margin: .4em 0; }
.cell-md :deep(code) { padding: 1px 5px; border-radius: 4px; background: var(--bg-code); font-size: .9em; }
.cell-md :deep(table) { border-collapse: collapse; }
.cell-md :deep(th), .cell-md :deep(td) { padding: 3px 10px; border: 1px solid var(--border); }
.cell-out-wrap { position: relative; }
.cell-out { padding: 2px 4px 2px 10px; }
.cell-out.capped { max-height: 420px; overflow-y: auto; }
.cell-expand { align-self: flex-start; margin: 2px 0 0 10px; padding: 1px 8px; border-radius: var(--r-sm); font-size: var(--fs-xs); color: var(--text-3); }
.cell-expand:hover { background: var(--bg-hover); color: var(--text); }
.cell-install { display: inline-flex; align-items: center; gap: 6px; align-self: flex-start; margin: 4px 0 0 10px; padding: 3px 10px; border-radius: var(--r-sm); border: 1px solid var(--border); background: var(--bg-hover); font-size: var(--fs-xs); font-weight: 500; color: var(--text); }
.cell-install:hover:not(:disabled) { border-color: var(--accent, var(--text-3)); }
.cell-install:disabled { opacity: .6; cursor: progress; }
.cell-hidden { display: flex; align-items: center; gap: 6px; min-width: 0; height: 26px; padding: 0 10px; border-radius: var(--r-md); background: var(--bg-panel); box-shadow: inset 0 0 0 1px var(--border); color: var(--text-3); font-family: var(--font-mono, monospace); font-size: 12px; text-align: left; }
.cell-hidden:hover { color: var(--text); }
.cell-time { padding: 0 4px 0 10px; font-size: 11px; color: var(--text-3); font-variant-numeric: tabular-nums; }
.cell-grip { display: grid; place-items: center; width: 16px; height: 18px; color: var(--text-3); cursor: grab; opacity: 0; }
.cell:hover .cell-grip, .cell.selected .cell-grip { opacity: .7; }
.cell-grip:hover { opacity: 1 !important; color: var(--text); }
.cell-fold { display: grid; place-items: center; width: 18px; height: 16px; border-radius: var(--r-sm); color: var(--text-3); opacity: 0; }
.cell:hover .cell-fold, .cell.selected .cell-fold { opacity: .8; }
.cell-fold:hover { background: var(--bg-hover); color: var(--text); }
.cell-fold.out { position: absolute; left: -14px; top: 4px; }
.cell.dragging { opacity: .45; }
.cell.drop-before { box-shadow: 0 -2px 0 0 var(--accent); }
.cell.drop-after { box-shadow: 0 2px 0 0 var(--accent); }
.cell-tools { position: absolute; top: -12px; right: 10px; z-index: 4; display: none; gap: 1px; padding: 2px; border-radius: var(--r-md); background: var(--bg-elevated); box-shadow: 0 0 0 1px var(--border), var(--shadow-sm, 0 1px 4px rgba(0,0,0,.12)); }
.cell:hover .cell-tools, .cell.selected .cell-tools, .cell:focus-within .cell-tools { display: inline-flex; }
.cell-tools button { display: inline-flex; align-items: center; justify-content: center; gap: 1px; min-width: 24px; height: 22px; padding: 0 3px; border-radius: var(--r-sm); color: var(--text-2); }
.cell-tools button:hover { background: var(--bg-hover); color: var(--text); }
.cell-tools button.danger:hover { color: var(--red); background: var(--red-soft, var(--bg-hover)); }
.spin { animation: nb-spin 1s linear infinite; }
@keyframes nb-spin { to { transform: rotate(360deg); } }
</style>
