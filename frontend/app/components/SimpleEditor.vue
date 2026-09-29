<template>
  <div class="wysiwyg-editor">
    <bubble-menu v-if="editor && editable" :editor="editor" :tippy-options="{ duration: 100 }" class="bubble-menu">
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
import { CustomMathExtension } from './CustomMathExtension'
import { CustomMathInlineExtension } from './CustomMathInlineExtension'
import { watch, onBeforeUnmount } from 'vue'
import 'katex/dist/katex.min.css'

const props = defineProps({
  modelValue: {
    type: String,
    default: '',
  },
  placeholderText: {
    type: String,
    default: 'Commencez à écrire...',
  },
  editable: {
    type: Boolean,
    default: true
  }
})

const emit = defineEmits(['update:modelValue', 'blur'])

const editor = useEditor({
  content: props.modelValue,
  editable: props.editable,
  extensions: [
    StarterKit.configure({
      codeBlock: false,
    }),
    TaskList,
    TaskItem.configure({
      nested: true,
    }),
    Link.configure({
      openOnClick: false,
    }),
    CodeBlock,
    Underline,
    Placeholder.configure({
      placeholder: props.placeholderText,
    }),
    CustomMathExtension,
    CustomMathInlineExtension,
  ],
  onUpdate: () => {
    emit('update:modelValue', editor.value.getHTML())
  },
  onBlur: () => {
    emit('blur')
  },
})

watch(() => props.modelValue, (value) => {
  const isSame = editor.value.getHTML() === value
  if (isSame) return
  editor.value.commands.setContent(value, false)
})

watch(() => props.editable, (value) => {
  if (editor.value) {
    editor.value.setEditable(value)
  }
})

onBeforeUnmount(() => {
  if (editor.value) editor.value.destroy()
})
</script>

<style scoped>
.wysiwyg-editor {
  border: 1px solid var(--border-color);
  border-radius: 6px;
  overflow: hidden;
  background: var(--bg-color-main);
  position: relative;
}

.bubble-menu, .floating-menu {
  display: flex;
  background-color: var(--bg-color-modal);
  padding: 4px;
  border-radius: 6px;
  border: 1px solid var(--border-color);
  box-shadow: 0 4px 6px rgba(0, 0, 0, 0.3);
}

.bubble-menu button, .floating-menu button {
  background: transparent;
  border: none;
  color: var(--text-color-3);
  padding: 4px 8px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 0.9rem;
  transition: all 0.2s;
}

.bubble-menu button:hover, .floating-menu button:hover {
  background: var(--bg-color-hover);
  color: var(--text-color);
}

.bubble-menu button.is-active {
  background: var(--primary-color);
  color: #fff;
}

.divider {
  width: 1px;
  background: var(--border-color);
  margin: 0 4px;
}

.editor-content-area {
  padding: 16px;
  min-height: 150px;
  color: var(--text-color);
  outline: none;
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
  background: var(--bg-color-modal);
  border-radius: 6px;
  border: 1px solid var(--border-color);
  padding: 1rem;
  color: var(--text-color-2);
  font-family: monospace;
  overflow-x: auto;
  margin: 1rem 0;
}
:deep(code) {
  background: var(--bg-color-2);
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
  color: var(--text-color-3);
  font-style: italic;
}
</style>
