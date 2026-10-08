<script setup lang="ts">
import * as Y from 'yjs'
import { HocuspocusProvider } from '@hocuspocus/provider'
import { EditorState, Compartment } from '@codemirror/state'
import {
  EditorView, keymap, lineNumbers, highlightActiveLine, highlightActiveLineGutter, highlightSpecialChars, drawSelection, dropCursor,
  rectangularSelection, crosshairCursor,
} from '@codemirror/view'
import { defaultKeymap, indentWithTab } from '@codemirror/commands'
import { foldGutter, foldKeymap, indentOnInput, syntaxHighlighting, defaultHighlightStyle, bracketMatching } from '@codemirror/language'
import { closeBrackets, closeBracketsKeymap, completionKeymap } from '@codemirror/autocomplete'
import { searchKeymap, highlightSelectionMatches } from '@codemirror/search'
import { yCollab, yUndoManagerKeymap } from 'y-codemirror.next'
import { languageFor, completionFor } from '~/editor/codeLang'
import { socketBase } from '~/composables/useRealtime'

/**
 * Éditeur de code partagé : fichier texte d'un projet, édité à plusieurs en direct
 * (document Yjs « file:<id> » du serveur de collaboration), coloration selon
 * l'extension, curseurs des autres personnes.
 */
const props = defineProps<{ fileId: string; path: string; readOnly?: boolean; user: { name: string; color: string } }>()
const emit = defineEmits<{
  (e: 'status', s: 'connecting' | 'synced' | 'offline' | 'denied'): void
  (e: 'change', text: string): void
  (e: 'collaborators', users: { name: string; color: string }[]): void
}>()

const host = ref<HTMLElement | null>(null)
const config = useRuntimeConfig()
const ydoc = new Y.Doc()
const ytext = ydoc.getText('content')
const collabUrl = (config.public.collabUrl as string) || `${socketBase()}/collab`
const provider = new HocuspocusProvider({ url: collabUrl, name: `file:${props.fileId}`, document: ydoc })
provider.awareness?.setLocalStateField('user', { name: props.user.name, color: props.user.color, colorLight: `${props.user.color}33` })
const undoManager = new Y.UndoManager(ytext)
const editable = new Compartment()
let view: EditorView | null = null

const theme = EditorView.theme({
  '&': { height: '100%', fontSize: '13px', backgroundColor: 'var(--bg)', color: 'var(--text)' },
  '.cm-scroller': { fontFamily: 'var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace)', lineHeight: '1.55' },
  '.cm-gutters': { backgroundColor: 'var(--bg)', color: 'var(--text-3)', border: 'none' },
  '.cm-activeLineGutter': { backgroundColor: 'var(--bg-hover)' },
  '.cm-activeLine': { backgroundColor: 'color-mix(in srgb, var(--bg-hover) 60%, transparent)' },
  '&.cm-focused': { outline: 'none' },
  '.cm-selectionBackground, &.cm-focused .cm-selectionBackground, ::selection': { backgroundColor: 'color-mix(in srgb, var(--accent) 22%, transparent) !important' },
  '.cm-cursor': { borderLeftColor: 'var(--text)' },
  '.cm-tooltip': { backgroundColor: 'var(--bg-elevated)', border: '1px solid var(--border)', borderRadius: '6px' },
  '.cm-tooltip-autocomplete ul li[aria-selected]': { backgroundColor: 'var(--accent-soft)', color: 'var(--text)' },
  '.cm-ySelectionInfo': { fontFamily: 'var(--font)', fontSize: '10.5px', padding: '1px 4px', borderRadius: '3px', opacity: '1', top: '-1.4em' },
})

onMounted(() => {
  emit('status', 'connecting')
  view = new EditorView({
    parent: host.value!,
    state: EditorState.create({
      doc: ytext.toString(),
      extensions: [
        lineNumbers(), highlightActiveLineGutter(), highlightSpecialChars(), foldGutter(), drawSelection(), dropCursor(),
        EditorState.allowMultipleSelections.of(true), indentOnInput(), syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
        bracketMatching(), closeBrackets(), rectangularSelection(), crosshairCursor(), highlightActiveLine(), highlightSelectionMatches(),
        EditorView.lineWrapping,
        keymap.of([...closeBracketsKeymap, ...defaultKeymap, ...searchKeymap, ...foldKeymap, ...completionKeymap, ...yUndoManagerKeymap, indentWithTab]),
        ...languageFor(props.path),
        completionFor(props.path),
        yCollab(ytext, provider.awareness, { undoManager }),
        editable.of([EditorState.readOnly.of(!!props.readOnly), EditorView.editable.of(!props.readOnly)]),
        theme,
        EditorView.updateListener.of((u) => { if (u.docChanged) emit('change', u.state.doc.toString()) }),
      ],
    }),
  })
})

provider.on('synced', () => { emit('status', 'synced'); emit('change', ytext.toString()) })
provider.on('authenticationFailed', () => emit('status', 'denied'))
provider.on('status', ({ status }: { status: string }) => { if (status === 'disconnected') emit('status', 'offline'); else if (status === 'connected' && provider.isSynced) emit('status', 'synced') })
provider.on('awarenessUpdate', ({ states }: { states: { clientId?: number; user?: { name: string; color: string } }[] }) => {
  emit('collaborators', states.map((s) => s.user).filter((u): u is { name: string; color: string } => !!u && u.name !== props.user.name))
})
watch(() => props.readOnly, (ro) => view?.dispatch({ effects: editable.reconfigure([EditorState.readOnly.of(!!ro), EditorView.editable.of(!ro)]) }))

onBeforeUnmount(() => {
  view?.destroy()
  provider.destroy()
  ydoc.destroy()
})

/** Va à une ligne (erreur LaTeX) et place le curseur au début. */
function goToLine(line: number) {
  if (!view) return
  const l = view.state.doc.line(Math.min(Math.max(1, line), view.state.doc.lines))
  view.dispatch({ selection: { anchor: l.from }, scrollIntoView: true })
  view.focus()
}
defineExpose({ getText: () => ytext.toString(), goToLine, focus: () => view?.focus() })
</script>

<template>
  <div ref="host" class="code-ed" />
</template>

<style scoped>
.code-ed { height: 100%; min-height: 0; overflow: hidden; }
.code-ed :deep(.cm-editor) { height: 100%; }
</style>
