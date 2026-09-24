<template>
  <div class="wysiwyg-editor">
    <bubble-menu v-if="editor" :editor="editor" :tippy-options="{ duration: 100 }" :should-show="({ editor, from, to }) => from !== to || editor.isActive('table')" class="bubble-menu">
      <button type="button" @click.prevent="editor.chain().focus().toggleBold().run()" :class="{ 'is-active': editor.isActive('bold') }"><b>B</b></button>
      <button type="button" @click.prevent="editor.chain().focus().toggleItalic().run()" :class="{ 'is-active': editor.isActive('italic') }"><i>I</i></button>
      <button type="button" @click.prevent="editor.chain().focus().toggleUnderline().run()" :class="{ 'is-active': editor.isActive('underline') }"><u>U</u></button>
      <button type="button" @click.prevent="editor.chain().focus().toggleStrike().run()" :class="{ 'is-active': editor.isActive('strike') }"><s>S</s></button>
      <div class="divider"></div>
      <button type="button" @click.prevent="editor.chain().focus().toggleHeading({ level: 1 }).run()" :class="{ 'is-active': editor.isActive('heading', { level: 1 }) }">H1</button>
      <button type="button" @click.prevent="editor.chain().focus().toggleHeading({ level: 2 }).run()" :class="{ 'is-active': editor.isActive('heading', { level: 2 }) }">H2</button>
      <div class="divider"></div>
      <button type="button" @click.prevent="editor.chain().focus().toggleCode().run()" :class="{ 'is-active': editor.isActive('code') }">&lt;&gt;</button>
      <button type="button" @click.prevent="editor.chain().focus().insertContent({ type: 'mathInline', attrs: { latex: '' } }).run()">$</button>
      
      <template v-if="editor.isActive('table')">
        <div class="divider"></div>
        <button type="button" @click.prevent="editor.chain().focus().addColumnBefore().run()" title="Col Avant">+C</button>
        <button type="button" @click.prevent="editor.chain().focus().addColumnAfter().run()" title="Col Après">C+</button>
        <button type="button" @click.prevent="editor.chain().focus().deleteColumn().run()" title="Supprimer Col">-C</button>
        <button type="button" @click.prevent="editor.chain().focus().addRowBefore().run()" title="Ligne Avant">+L</button>
        <button type="button" @click.prevent="editor.chain().focus().addRowAfter().run()" title="Ligne Après">L+</button>
        <button type="button" @click.prevent="editor.chain().focus().deleteRow().run()" title="Supprimer Ligne">-L</button>
        <button type="button" @click.prevent="editor.chain().focus().deleteTable().run()" title="Supprimer Tableau">🗑</button>
      </template>
    </bubble-menu>

    <editor-content :editor="editor" class="editor-content-area" />
  </div>
</template>

<script setup>
import { useEditor, EditorContent, BubbleMenu } from '@tiptap/vue-3'
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
import MentionSuggestion from './MentionSuggestion'
import { watch, onBeforeUnmount, markRaw, onMounted } from 'vue'
import 'katex/dist/katex.min.css'

const props = defineProps({
  modelValue: { type: String, default: '' },
  documentId: { type: String, default: null },
  currentUser: { type: Object, default: () => ({ name: 'Anonyme' }) },
  editable: { type: Boolean, default: true }
})

const emit = defineEmits(['update:modelValue', 'update:collaborators'])

const colors = ['#f56565', '#ed8936', '#ecc94b', '#48bb78', '#38b2ac', '#4299e1', '#667eea', '#9f7aea', '#ed64a6']
const userColor = colors[Math.floor(Math.random() * colors.length)]

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
    suggestion: MentionSuggestion,
  }),
  CommandsExtension.configure({
    suggestion: { items: getSuggestionItems, render: renderItems },
  }),
]

if (props.documentId) {
  ydoc = markRaw(new Y.Doc())

  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  
  const isLocal = window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1'
  const wsUrl = isLocal 
    ? `ws://127.0.0.1:8082`
    : `${protocol}//${window.location.host}/collab`

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
}

// CRUCIAL: Do not pass 'content' AT ALL if using Collaboration in Tiptap 3,
// otherwise it tries to resolve positions before the document is ready.
if (!props.documentId) {
  editorOptions.content = props.modelValue
}

const editor = useEditor(editorOptions)

if (provider) {
  provider.on('synced', () => {
    if (editor.value && editor.value.isEmpty && props.modelValue && props.modelValue !== '<p></p>') {
      editor.value.commands.setContent(props.modelValue)
    }
  })
}

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
</script>

<style scoped>
.wysiwyg-editor {
  overflow: hidden;
  background: transparent;
  position: relative;
}

.bubble-menu, .floating-menu {
  display: flex;
  background-color: #27282b;
  padding: 4px;
  border-radius: 6px;
  border: 1px solid #3f3f46;
  box-shadow: 0 4px 6px rgba(0, 0, 0, 0.3);
}

.bubble-menu button, .floating-menu button {
  background: transparent;
  border: none;
  color: #a1a1aa;
  padding: 4px 8px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 0.9rem;
  transition: all 0.2s;
}

.bubble-menu button:hover, .floating-menu button:hover {
  background: #3f3f46;
  color: #fff;
}

.bubble-menu button.is-active {
  background: #4f46e5;
  color: #fff;
}

.divider {
  width: 1px;
  background: #3f3f46;
  margin: 0 4px;
}

.editor-content-area {
  min-height: 150px;
  color: #eee;
  outline: none;
}

:deep(.ProseMirror) {
  outline: none;
  min-height: 150px;
  padding-bottom: 50vh;
}
:deep(.ProseMirror p.is-editor-empty:first-child::before) {
  color: #6b7280;
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
  background: #111113;
  border-radius: 6px;
  border: 1px solid #1e1f23;
  padding: 1rem;
  color: #e2e8f0;
  font-family: monospace;
  overflow-x: auto;
  margin: 1rem 0;
}
:deep(code) {
  background: #17181c;
  padding: 2px 4px;
  border-radius: 4px;
  font-size: 0.9em;
  color: #fca5a5;
}
:deep(pre code) {
  background: none;
  padding: 0;
  color: inherit;
}
:deep(blockquote) {
  border-left: 3px solid #5e6ad2;
  margin-left: 0;
  padding-left: 1rem;
  color: #8a8f98;
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
  border: 1px solid #3f3f46;
  box-sizing: border-box;
  min-width: 1em;
  padding: 6px 8px;
  position: relative;
  vertical-align: top;
}
:deep(.ProseMirror table th) {
  background-color: #1e1f23;
  font-weight: bold;
  text-align: left;
}
:deep(.ProseMirror table .column-resize-handle) {
  background-color: #5e6ad2;
  bottom: -2px;
  pointer-events: none;
  position: absolute;
  right: -2px;
  top: 0;
  width: 4px;
}
:deep(.ProseMirror table .selectedCell:after) {
  background: rgba(94, 106, 210, 0.2);
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
</style>
