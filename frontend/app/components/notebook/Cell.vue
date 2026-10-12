<script setup lang="ts">
import * as Y from 'yjs'
import { Play, Loader2, ArrowUp, ArrowDown, Trash2, Eraser, Type, Code2, Plus } from 'lucide-vue-next'
import { EditorState, Prec } from '@codemirror/state'
import { EditorView, keymap, drawSelection, highlightSpecialChars } from '@codemirror/view'
import { defaultKeymap, indentWithTab } from '@codemirror/commands'
import { syntaxHighlighting, defaultHighlightStyle, bracketMatching, indentOnInput } from '@codemirror/language'
import { closeBrackets, closeBracketsKeymap, completionKeymap } from '@codemirror/autocomplete'
import { python } from '@codemirror/lang-python'
import { markdown } from '@codemirror/lang-markdown'
import { yCollab } from 'y-codemirror.next'
import type { NbOutput } from '~/composables/useNotebookKernel'
import { renderMarkdown } from '~/editor/notebookMarkdown'
import { useI18n } from '~/composables/useI18n'

/**
 * Une cellule de notebook : le texte est un Y.Text partagé (édition à plusieurs, curseurs),
 * les sorties viennent de la dernière exécution (de n'importe qui).
 */
const props = defineProps<{
  id: string
  type: 'code' | 'markdown' | 'raw'
  ytext: Y.Text
  count: number | null
  outputs: NbOutput[]
  running: boolean
  queued: boolean
  selected: boolean
  readOnly: boolean
  awareness: any
}>()
const emit = defineEmits<{
  select: []
  run: [mode: 'stay' | 'next' | 'below']
  add: [where: 'above' | 'below', type: 'code' | 'markdown']
  remove: []
  move: [dir: -1 | 1]
  clear: []
  retype: [type: 'code' | 'markdown']
}>()
const { t } = useI18n()

const host = ref<HTMLElement | null>(null)
const editing = ref(props.type !== 'markdown')
const src = ref(props.ytext.toString())
const html = computed(() => (props.type === 'markdown' ? renderMarkdown(src.value) : ''))
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
          { key: 'Escape', run: (v) => { v.contentDOM.blur(); if (props.type === 'markdown' && src.value.trim()) editing.value = false; return true } },
        ])),
        keymap.of([...closeBracketsKeymap, ...defaultKeymap, ...completionKeymap, indentWithTab]),
        ...(props.type === 'code' ? [python()] : props.type === 'markdown' ? [markdown()] : []),
        yCollab(props.ytext, props.awareness, { undoManager: undo }),
        EditorState.readOnly.of(props.readOnly), EditorView.editable.of(!props.readOnly),
        theme,
        EditorView.domEventHandlers({ focus: () => { emit('select') } }),
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
defineExpose({ focus: () => { if (props.type === 'markdown') editing.value = true; nextTick(() => view?.focus()) } })

const prompt = computed(() => (props.running ? '*' : props.queued ? '…' : props.count ?? ' '))
const hasOutputs = computed(() => props.outputs.length > 0)
</script>

<template>
  <div class="cell" :class="[type, { selected, running }]" :data-cell="id" @click="emit('select')" @focusout="onFocusOut">
    <div class="cell-gutter">
      <button v-if="type === 'code'" class="cell-run" :disabled="running" :aria-label="t('notebook.run')" :title="`${t('notebook.run')} (Shift+Enter)`" @click.stop="emit('run', 'stay')">
        <Loader2 v-if="running || queued" :size="14" class="spin" /><Play v-else :size="14" />
      </button>
      <span v-if="type === 'code'" class="cell-prompt">[{{ prompt }}]</span>
    </div>
    <div class="cell-main">
      <div v-if="editing" class="cell-input" :class="{ md: type === 'markdown' }"><div ref="host" class="cell-editor" /></div>
      <div v-else class="cell-md prose" @dblclick="startEdit" v-html="html" />
      <div v-if="type === 'code' && hasOutputs" class="cell-out"><NotebookOutput :outputs="outputs" /></div>
    </div>
    <div v-if="!readOnly" class="cell-tools" @click.stop>
      <button :aria-label="t('notebook.addAbove')" :title="t('notebook.addAbove')" @click="emit('add', 'above', 'code')"><Plus :size="12" /><ArrowUp :size="11" /></button>
      <button :aria-label="t('notebook.addBelow')" :title="t('notebook.addBelow')" @click="emit('add', 'below', 'code')"><Plus :size="12" /><ArrowDown :size="11" /></button>
      <button :aria-label="t('notebook.moveUp')" :title="t('notebook.moveUp')" @click="emit('move', -1)"><ArrowUp :size="13" /></button>
      <button :aria-label="t('notebook.moveDown')" :title="t('notebook.moveDown')" @click="emit('move', 1)"><ArrowDown :size="13" /></button>
      <button v-if="type !== 'raw'" :aria-label="type === 'code' ? t('notebook.toText') : t('notebook.toCode')" :title="type === 'code' ? t('notebook.toText') : t('notebook.toCode')" @click="emit('retype', type === 'code' ? 'markdown' : 'code')"><component :is="type === 'code' ? Type : Code2" :size="13" /></button>
      <button v-if="type === 'code' && hasOutputs" :aria-label="t('notebook.clearOutput')" :title="t('notebook.clearOutput')" @click="emit('clear')"><Eraser :size="13" /></button>
      <button class="danger" :aria-label="t('common.delete')" :title="t('common.delete')" @click="emit('remove')"><Trash2 :size="13" /></button>
    </div>
  </div>
</template>

<style scoped>
.cell { position: relative; display: grid; grid-template-columns: 62px minmax(0, 1fr); gap: 6px; padding: 4px 8px 4px 0; border-left: 3px solid transparent; border-radius: 0 var(--r-sm) var(--r-sm) 0; }
.cell.selected { border-left-color: var(--accent); background: color-mix(in srgb, var(--accent-soft) 40%, transparent); }
.cell-gutter { display: flex; flex-direction: column; align-items: flex-end; gap: 2px; padding: 8px 0 0 0; font-family: var(--font-mono, monospace); font-size: 11px; color: var(--text-3); user-select: none; }
.cell-run { display: grid; place-items: center; width: 24px; height: 22px; border-radius: var(--r-sm); color: var(--text-2); opacity: .55; }
.cell:hover .cell-run, .cell.selected .cell-run, .cell.running .cell-run { opacity: 1; }
.cell-run:hover { background: var(--bg-hover); color: var(--accent); }
.cell-prompt { padding-right: 4px; }
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
.cell-out { padding: 2px 4px 2px 10px; }
.cell-tools { position: absolute; top: -12px; right: 10px; z-index: 4; display: none; gap: 1px; padding: 2px; border-radius: var(--r-md); background: var(--bg-elevated); box-shadow: 0 0 0 1px var(--border), var(--shadow-sm, 0 1px 4px rgba(0,0,0,.12)); }
.cell:hover .cell-tools, .cell.selected .cell-tools, .cell:focus-within .cell-tools { display: inline-flex; }
.cell-tools button { display: inline-flex; align-items: center; justify-content: center; gap: 1px; min-width: 24px; height: 22px; padding: 0 3px; border-radius: var(--r-sm); color: var(--text-2); }
.cell-tools button:hover { background: var(--bg-hover); color: var(--text); }
.cell-tools button.danger:hover { color: var(--red); background: var(--red-soft, var(--bg-hover)); }
.spin { animation: nb-spin 1s linear infinite; }
@keyframes nb-spin { to { transform: rotate(360deg); } }
</style>
