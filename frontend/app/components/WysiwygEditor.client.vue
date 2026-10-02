<template>
  <div class="wysiwyg-editor" :class="{ 'is-editable': editable }">
    <div class="editor-toolbar" v-if="editor && editable">
      <div class="toolbar-group">
        <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().toggleHeading({ level: 1 }).run()" :class="{ 'is-active': editor.isActive('heading', { level: 1 }) }" title="Titre 1">H1</button>
        <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().toggleHeading({ level: 2 }).run()" :class="{ 'is-active': editor.isActive('heading', { level: 2 }) }" title="Titre 2">H2</button>
        <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().toggleHeading({ level: 3 }).run()" :class="{ 'is-active': editor.isActive('heading', { level: 3 }) }" title="Titre 3">H3</button>
      </div>

      <div class="divider"></div>

      <div class="toolbar-group">
        <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().toggleBold().run()" :class="{ 'is-active': editor.isActive('bold') }" title="Gras"><b style="font-family: serif; font-size: 1.1rem;">B</b></button>
        <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().toggleItalic().run()" :class="{ 'is-active': editor.isActive('italic') }" title="Italique"><i style="font-family: serif; font-size: 1.1rem;">I</i></button>
        <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().toggleUnderline().run()" :class="{ 'is-active': editor.isActive('underline') }" title="Souligné"><u style="font-family: serif; font-size: 1.1rem;">U</u></button>
        <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().toggleStrike().run()" :class="{ 'is-active': editor.isActive('strike') }" title="Barré"><s style="font-family: serif; font-size: 1.1rem;">S</s></button>
      </div>

      <div class="divider"></div>

      <div class="toolbar-group">
        <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().toggleBulletList().run()" :class="{ 'is-active': editor.isActive('bulletList') }" title="Liste à puces"><span style="font-size: 1.2rem;">•</span></button>
        <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().toggleOrderedList().run()" :class="{ 'is-active': editor.isActive('orderedList') }" title="Liste numérotée">1.</button>
        <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().toggleTaskList().run()" :class="{ 'is-active': editor.isActive('taskList') }" title="Liste de tâches"><span style="font-size: 1.2rem;">☐</span></button>
        <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().toggleBlockquote().run()" :class="{ 'is-active': editor.isActive('blockquote') }" title="Citation">""</button>
      </div>

      <div class="divider"></div>

      <div class="toolbar-group">
        <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().toggleCodeBlock().run()" :class="{ 'is-active': editor.isActive('codeBlock') }" title="Code Source">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M10 20l4-16m4 4l4 8m-8-8l-4 8"/></svg>
        </button>
        <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().insertContent({ type: 'mathInline', attrs: { latex: '' } }).run()" title="Formule Mathématique">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M9 7h6m0 10v-3m-3 3h.01M9 17h.01M9 14h.01M12 14h.01M15 11h.01M12 11h.01M9 11h.01M7 21h10a2 2 0 002-2V5a2 2 0 00-2-2H7a2 2 0 00-2 2v14a2 2 0 002 2z"/></svg>
        </button>
        <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().setHorizontalRule().run()" title="Séparateur">—</button>
        <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run()" title="Tableau">▦</button>
      </div>

      <template v-if="editor.isActive('table')">
        <div class="divider"></div>
        <div class="toolbar-group">
          <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().addColumnBefore().run()" title="Colonne Avant">+C</button>
          <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().addColumnAfter().run()" title="Colonne Après">C+</button>
          <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().deleteColumn().run()" title="Supprimer Colonne">-C</button>
          <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().addRowBefore().run()" title="Ligne Avant">+L</button>
          <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().addRowAfter().run()" title="Ligne Après">L+</button>
          <button type="button" class="toolbar-btn" @click.prevent="editor.chain().focus().deleteRow().run()" title="Supprimer Ligne">-L</button>
          <button type="button" class="toolbar-btn danger" @click.prevent="editor.chain().focus().deleteTable().run()" title="Supprimer Tableau">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/></svg>
          </button>
        </div>
      </template>
    </div>

    <editor-content v-show="isSynced" :editor="editor" class="editor-content-area" />
    <div v-show="!isSynced" class="editor-loading">
      <div class="loading-spinner"></div>
      Chargement du document...
    </div>
  </div>
</template>

<script setup>

import { useEditor, EditorContent } from '@tiptap/vue-3'
import StarterKit from '@tiptap/starter-kit'
import Placeholder from '@tiptap/extension-placeholder'
import TaskList from '@tiptap/extension-task-list'
import TaskItem from '@tiptap/extension-task-item'
import Link from '@tiptap/extension-link'
import CodeBlock from '@tiptap/extension-code-block'
import Underline from '@tiptap/extension-underline'
import { ResizableImage } from './ResizableImage'
import { Collaboration } from '@tiptap/extension-collaboration'
import { CollaborationCursor } from '@tiptap/extension-collaboration-cursor'
import Table from '@tiptap/extension-table'
import TableRow from '@tiptap/extension-table-row'
import TableCell from '@tiptap/extension-table-cell'
import TableHeader from '@tiptap/extension-table-header'
import Mention from '@tiptap/extension-mention'
import * as Y from 'yjs'
import { HocuspocusProvider } from '@hocuspocus/provider'
import { CustomMathExtension } from './CustomMathExtension'
import { CustomMathInlineExtension } from './CustomMathInlineExtension'
import { CommandsExtension, getSuggestionItems, renderItems } from './CommandsExtension'
import { EmojiExtension, getEmojiItems, renderEmojiItems } from './EmojiExtension'
import MentionSuggestion from './MentionSuggestion'
import { TrailingNode } from './TrailingNode'
import GlobalDragHandle from 'tiptap-extension-global-drag-handle'
import Dropcursor from '@tiptap/extension-dropcursor'
import { watch, onBeforeUnmount, markRaw, onMounted, inject, ref } from 'vue'
import 'katex/dist/katex.min.css'
import { SearchExtension, searchPluginKey } from '../utils/tiptapSearch'
import { NoticeExtension } from './NoticeExtension'

const props = defineProps({
  modelValue: { type: String, default: '' },
  documentId: { type: String, default: null },
  currentUser: { type: Object, default: () => ({ name: 'Anonyme' }) },
  editable: { type: Boolean, default: true },
  workspaceMembers: { type: Array, default: () => [] },
})

const emit = defineEmits(['update:modelValue', 'update:collaborators', 'mention'])

const colors = ['#f56565', '#ed8936', '#ecc94b', '#48bb78', '#38b2ac', '#4299e1', '#667eea', '#9f7aea', '#ed64a6']
const userColor = colors[Math.floor(Math.random() * colors.length)]

const localSearchQuery = inject('localSearchQuery', ref(''))

let provider = null
let ydoc = null

const extensions = [
  StarterKit.configure({ codeBlock: false, history: false }),
  TaskList,
  TaskItem.configure({ nested: true }),
  Link.configure({ openOnClick: false }),
  CodeBlock,
  Underline,
  ResizableImage,
  Placeholder.configure({ placeholder: 'Tapez / pour les commandes ou commencez à écrire...' }),
  CustomMathExtension,
  CustomMathInlineExtension,
  Table.configure({ resizable: true }),
  TableRow,
  TableHeader,
  TableCell,
  Mention.configure({
    HTMLAttributes: {
      class: 'mention',
    },
    suggestion: MentionSuggestion(props.workspaceMembers, (userId) => {
      emit('mention', userId)
    }),
  }),
  CommandsExtension.configure({
    suggestion: { items: getSuggestionItems, render: renderItems },
  }),
  EmojiExtension.configure({
    suggestion: { items: getEmojiItems, render: renderEmojiItems },
  }),
  NoticeExtension,
  SearchExtension,
  TrailingNode,
  GlobalDragHandle.configure({
    dragHandleWidth: 20,
    scrollTreshold: 100,
  }),
]

if (props.documentId && typeof window !== 'undefined') {
  ydoc = markRaw(new Y.Doc())
  // Seed the document with a paragraph to prevent TipTap TextSelection crash on completely empty docs
  const xml = ydoc.getXmlFragment('default')
  if (xml.length === 0) {
    const p = new Y.XmlElement('paragraph')
    xml.insert(0, [p])
  }

  const protocol = typeof window !== 'undefined' && window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  
  const wsUrl = typeof window !== 'undefined' 
    ? `${protocol}//${window.location.host}/collab`
    : ''

  provider = markRaw(new HocuspocusProvider({
    url: wsUrl,
    name: props.documentId,
    document: ydoc,
    connect: false, // DON'T CONNECT YET
  }))

  extensions.push(
    Collaboration.configure({
      document: ydoc,
    }),
    CollaborationCursor.configure({
      provider,
      user: {
        name: props.currentUser?.name || 'Anonyme',
        color: userColor,
      },
    })
  )
}

const editorOptions = {
  extensions,
  editable: props.editable,
  onUpdate: () => {
    if (!props.documentId) {
      emit('update:modelValue', editor.value.getHTML())
    }
  },
  editorProps: {
    handleDrop: function(view, event, slice, moved) {
      if (!moved && event.dataTransfer && event.dataTransfer.files && event.dataTransfer.files[0]) {
        const file = event.dataTransfer.files[0]
        if (file.type.startsWith('image/')) {
          uploadAndInsertImage(file, view, event.clientX, event.clientY)
          return true
        }
      }
      return false
    },
    handlePaste: function(view, event, slice) {
      if (event.clipboardData && event.clipboardData.files && event.clipboardData.files[0]) {
        const file = event.clipboardData.files[0]
        if (file.type.startsWith('image/')) {
          uploadAndInsertImage(file, view)
          return true
        }
      }
      return false
    }
  }
}

async function uploadAndInsertImage(file, view, x, y) {
  const formData = new FormData()
  formData.append('file', file)
  try {
    const response = await fetch('/api/upload', {
      method: 'POST',
      body: formData
    })
    const data = await response.json()
    if (data.url) {
      const { schema } = view.state
      const node = schema.nodes.image.create({ src: data.url })
      if (x !== undefined && y !== undefined) {
        const pos = view.posAtCoords({ left: x, top: y })
        if (pos) {
          const tr = view.state.tr.insert(pos.pos, node)
          view.dispatch(tr)
          return
        }
      }
      const tr = view.state.tr.replaceSelectionWith(node)
      view.dispatch(tr)
    }
  } catch (err) {
    console.error('Upload failed', err)
  }
}

const isSynced = ref(!props.documentId) // True if no collab

// CRUCIAL: Pass an empty paragraph for Collaboration to prevent TextSelection crash on empty doc.
// If it's not collaborative (no documentId), pass the actual modelValue.
if (!props.documentId) {
  editorOptions.content = props.modelValue
} else {
  editorOptions.content = '<p></p>'
}

const editor = useEditor(editorOptions)

if (provider) {
  provider.on('synced', () => {
    isSynced.value = true
    if (editor.value && editor.value.isEmpty && props.modelValue && props.modelValue !== '<p></p>') {
      editor.value.commands.setContent(props.modelValue)
    }
  })
}

watch(localSearchQuery, (val) => {
  if (editor.value && editor.value.view) {
    editor.value.view.dispatch(editor.value.view.state.tr.setMeta(searchPluginKey, val))
  }
})

watch(() => props.modelValue, (value) => {
  if (!editor.value) return
  if (props.documentId) return
  const isSame = editor.value.getHTML() === value
  if (isSame) return
  editor.value.commands.setContent(value, false)
})

watch(() => props.editable, (value) => {
  if (editor.value) {
    editor.value.setEditable(value)
  }
})

onMounted(() => {
  if (provider) {
    provider.connect()
    
    provider.awareness.on('update', () => {
      const states = Array.from(provider.awareness.getStates().values())
      const users = states.map(state => state.user).filter(Boolean)
      // Filter out duplicate names
      const uniqueUsers = []
      const names = new Set()
      for (const u of users) {
        if (!names.has(u.name)) {
          names.add(u.name)
          uniqueUsers.push(u)
        }
      }
      emit('update:collaborators', uniqueUsers)
    })
  }
  
  // Set content immediately so we don't wait for the WebSocket sync if the editor is empty
  if (editor.value && editor.value.isEmpty && props.modelValue && props.modelValue !== '<p></p>') {
    editor.value.commands.setContent(props.modelValue, false)
  }
})

onBeforeUnmount(() => {
  if (provider) {
    provider.destroy()
  }
  if (editor.value) editor.value.destroy()
})

defineExpose({
  getHTML: () => {
    return editor.value ? editor.value.getHTML() : ''
  }
})
</script>

<style scoped>
.wysiwyg-editor {
  background: transparent;
  position: relative;
  display: flex;
  flex-direction: column;
  min-height: calc(100vh - 200px);
}
.wysiwyg-editor:focus-within {
  /* No border */
}

.editor-toolbar {
  position: fixed;
  bottom: 40px;
  left: 50%;
  transform: translateX(-50%) scale(0.9);
  z-index: 100;
  display: flex;
  align-items: center;
  flex-wrap: nowrap;
  gap: 4px;
  padding: 6px 12px;
  background-color: var(--bg-color-modal);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-float);
  opacity: 0.6;
  transition: all 0.3s cubic-bezier(0.2, 0.8, 0.2, 1);
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-width: none;
  -ms-overflow-style: none;
}
.editor-toolbar::-webkit-scrollbar {
  display: none;
}

.editor-toolbar::after {
  content: '';
  position: absolute;
  top: -20px;
  bottom: -20px;
  left: -20px;
  right: -20px;
  z-index: -1;
}

.editor-toolbar:hover {
  opacity: 1;
  transform: translateX(-50%) scale(1);
  box-shadow: var(--shadow-glass);
}

.toolbar-group {
  display: flex;
  align-items: center;
  gap: 2px;
}

.toolbar-btn {
  background: transparent;
  border: none;
  color: var(--text-color-3);
  padding: 6px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 28px;
  height: 28px;
  font-size: 0.9rem;
  transition: all var(--transition-fast);
}

.toolbar-btn:hover {
  background: var(--border-color);
  color: var(--text-color);
}

.toolbar-btn.is-active {
  background: var(--primary-color-glass);
  color: var(--primary-color);
}

.toolbar-btn.danger:hover {
  background: rgba(235, 87, 87, 0.1);
  color: var(--danger-color);
}

.divider {
  width: 1px;
  height: 18px;
  background: var(--border-color);
  margin: 0 4px;
}

.editor-content-area {
  flex: 1;
  padding: 16px 24px;
  color: var(--text-color);
  outline: none;
  font-size: 1.05rem;
  line-height: 1.6;
}

:deep(.ProseMirror) {
  outline: none;
  min-height: 150px;
}
:deep(.ProseMirror p.is-editor-empty:first-child::before) {
  color: var(--text-color-3);
  content: attr(data-placeholder);
  float: left;
  height: 0;
  pointer-events: none;
}
:deep(.ProseMirror p) {
  margin-top: 0;
  margin-bottom: 0.5rem;
}
/* Task Lists */
:deep(ul[data-type="taskList"]) {
  list-style: none;
  padding: 0;
  margin: 0.5rem 0;
}
:deep(ul[data-type="taskList"] li) {
  display: flex;
  align-items: flex-start;
  margin-bottom: 0.25rem;
}
:deep(ul[data-type="taskList"] li > label) {
  margin-right: 0.5rem;
  user-select: none;
}
:deep(ul[data-type="taskList"] li > div) {
  flex: 1;
}

/* Code Blocks */
:deep(pre) {
  background: var(--bg-color-2);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-color);
  padding: 1rem;
  color: var(--text-color);
  font-family: monospace;
  overflow-x: auto;
  margin: 1rem 0;
}
:deep(code) {
  background: var(--bg-color-hover);
  padding: 2px 4px;
  border-radius: 4px;
  font-size: 0.9em;
  color: var(--danger-color);
}
:deep(pre code) {
  background: none;
  padding: 0;
  color: inherit;
}
:deep(blockquote) {
  border-left: 3px solid var(--primary-color);
  margin-left: 0;
  padding-left: 1rem;
  color: var(--text-color-2);
  font-style: italic;
}

/* Tables */
:deep(.ProseMirror table) {
  border-collapse: collapse;
  margin: 0;
  overflow: hidden;
  table-layout: fixed;
  width: 100%;
}
:deep(.ProseMirror table td),
:deep(.ProseMirror table th) {
  border: 1px solid var(--border-color);
  box-sizing: border-box;
  min-width: 1em;
  padding: 6px 8px;
  position: relative;
  vertical-align: top;
}
:deep(.ProseMirror table th) {
  background-color: var(--bg-color-2);
  font-weight: bold;
  text-align: left;
}
:deep(.ProseMirror table .column-resize-handle) {
  background-color: var(--primary-color);
  bottom: -2px;
  pointer-events: none;
  position: absolute;
  right: -2px;
  top: 0;
  width: 4px;
}
:deep(.ProseMirror table .selectedCell:after) {
  background: var(--primary-color-glass);
  content: "";
  left: 0;
  right: 0;
  top: 0;
  bottom: 0;
  position: absolute;
  pointer-events: none;
}

</style>

<style>
/* CSS for Tiptap collaboration cursors - non scoped */
.collaboration-cursor__caret {
  border-left: 1px solid #0d0d0d;
  border-right: 1px solid #0d0d0d;
  margin-left: -1px;
  margin-right: -1px;
  pointer-events: none;
  position: relative;
  word-break: normal;
}
.collaboration-cursor__label {
  border-radius: 3px 3px 3px 0;
  color: #fff;
  font-size: 12px;
  font-style: normal;
  font-weight: 600;
  left: -1px;
  line-height: normal;
  padding: 0.1rem 0.3rem;
  position: absolute;
  top: -1.4em;
  user-select: none;
  white-space: nowrap;
}

/* Mention extension styles */
.mention {
  color: #5e6ad2;
  background-color: rgba(94, 106, 210, 0.1);
  border-radius: 0.4rem;
  padding: 0.1rem 0.3rem;
  box-decoration-break: clone;
}

/* Drag Handle */
:deep(.drag-handle) {
  position: fixed;
  z-index: 50;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border-radius: 4px;
  cursor: grab;
  background-image: url('data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="gray" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="9" cy="5" r="1"/><circle cx="9" cy="12" r="1"/><circle cx="9" cy="19" r="1"/><circle cx="15" cy="5" r="1"/><circle cx="15" cy="12" r="1"/><circle cx="15" cy="19" r="1"/></svg>');
  background-position: center;
  background-repeat: no-repeat;
  transition: background-color 0.2s;
}

:deep(.drag-handle:hover) {
  background-color: var(--bg-color-hover);
}
</style>

<style scoped>
.editor-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 48px;
  color: var(--text-color-3);
  font-size: 1.1rem;
}
.loading-spinner {
  width: 32px;
  height: 32px;
  border: 3px solid var(--border-color);
  border-top-color: var(--primary-color);
  border-radius: 50%;
  animation: spin 1s linear infinite;
  margin-bottom: 16px;
}
@keyframes spin {
  to { transform: rotate(360deg); }
}

/* Responsive design for the editor toolbar */
@media (max-width: 768px) {
  .editor-toolbar {
    width: 95vw;
    max-width: 95vw;
    overflow-x: auto;
    overflow-y: hidden;
    white-space: nowrap;
    justify-content: flex-start;
    -webkit-overflow-scrolling: touch;
    padding: 8px;
    bottom: 20px;
    border-radius: var(--radius-lg);
  }
  .editor-toolbar::-webkit-scrollbar {
    display: none;
  }
  .toolbar-btn {
    min-width: 32px;
    height: 32px;
  }
}
</style>

<style>
/* Global style to guarantee the highlight is applied regardless of scoping */
.search-highlight {
  background-color: rgba(255, 212, 0, 0.7) !important;
  color: #000 !important;
  border-radius: 2px;
  box-shadow: 0 0 0 2px rgba(255, 212, 0, 0.4);
}

.notice-block {
  padding: 1rem 1.5rem;
  margin: 1.5rem 0;
  border-radius: var(--radius-md);
  border-left: 4px solid;
  position: relative;
}

.notice-block::before {
  content: '';
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-weight: 600;
  margin-bottom: 0.5rem;
  font-size: 1.05rem;
}

.notice-block p:last-child {
  margin-bottom: 0;
}

.notice-block p:first-of-type {
  margin-top: 0;
}

/* Info Notice */
.notice-info {
  background-color: rgba(56, 189, 248, 0.1);
  border-color: #38bdf8;
  color: var(--text-color);
}
.notice-info::before {
  content: 'ℹ️ Info notice';
  color: #38bdf8;
}

/* Success Notice */
.notice-success {
  background-color: rgba(74, 222, 128, 0.1);
  border-color: #4ade80;
  color: var(--text-color);
}
.notice-success::before {
  content: '✅ Success notice';
  color: #4ade80;
}

/* Warning Notice */
.notice-warning {
  background-color: rgba(250, 204, 21, 0.1);
  border-color: #facc15;
  color: var(--text-color);
}
.notice-warning::before {
  content: '⚠️ Warning notice';
  color: #facc15;
}

/* Tip Notice */
.notice-tip {
  background-color: rgba(167, 139, 250, 0.1);
  border-color: #a78bfa;
  color: var(--text-color);
}
.notice-tip::before {
  content: '💡 Tip notice';
  color: #a78bfa;
}
</style>
