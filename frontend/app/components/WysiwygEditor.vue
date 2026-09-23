<template>
  <div class="wysiwyg-editor">
    <bubble-menu v-if="editor" :editor="editor" :tippy-options="{ duration: 100 }" class="bubble-menu">
      <button type="button" @click.prevent="editor.chain().focus().toggleBold().run()" :class="{ 'is-active': editor.isActive('bold') }"><b>B</b></button>
      <button type="button" @click.prevent="editor.chain().focus().toggleItalic().run()" :class="{ 'is-active': editor.isActive('italic') }"><i>I</i></button>
      <button type="button" @click.prevent="editor.chain().focus().toggleStrike().run()" :class="{ 'is-active': editor.isActive('strike') }"><s>S</s></button>
      <div class="divider"></div>
      <button type="button" @click.prevent="editor.chain().focus().toggleHeading({ level: 1 }).run()" :class="{ 'is-active': editor.isActive('heading', { level: 1 }) }">H1</button>
      <button type="button" @click.prevent="editor.chain().focus().toggleHeading({ level: 2 }).run()" :class="{ 'is-active': editor.isActive('heading', { level: 2 }) }">H2</button>
      <div class="divider"></div>
      <button type="button" @click.prevent="editor.chain().focus().insertContent({ type: 'mathInline', attrs: { latex: '' } }).run()">$</button>
    </bubble-menu>

    <editor-content :editor="editor" class="editor-content-area" />
  </div>
</template>

<script setup>
import { useEditor, EditorContent } from '@tiptap/vue-3'
import { BubbleMenu } from '@tiptap/vue-3/menus'
import StarterKit from '@tiptap/starter-kit'
import Placeholder from '@tiptap/extension-placeholder'
import { CustomMathExtension } from './CustomMathExtension'
import { CustomMathInlineExtension } from './CustomMathInlineExtension'
import { CommandsExtension, getSuggestionItems, renderItems } from './CommandsExtension'
import { watch, onBeforeUnmount } from 'vue'
import 'katex/dist/katex.min.css'

const props = defineProps({
  modelValue: {
    type: String,
    default: '',
  },
})

const emit = defineEmits(['update:modelValue'])

const editor = useEditor({
  content: props.modelValue,
  extensions: [
    StarterKit,
    Placeholder.configure({
      placeholder: 'Tapez / pour les commandes ou commencez à écrire...',
    }),
    CustomMathExtension,
    CustomMathInlineExtension,
    CommandsExtension.configure({
      suggestion: {
        items: getSuggestionItems,
        render: renderItems,
      },
    }),
  ],
  onUpdate: () => {
    emit('update:modelValue', editor.value.getHTML())
  },
})

watch(() => props.modelValue, (value) => {
  const isSame = editor.value.getHTML() === value
  if (isSame) return
  editor.value.commands.setContent(value, false)
})

onBeforeUnmount(() => {
  if (editor.value) editor.value.destroy()
})
</script>

<style scoped>
.wysiwyg-editor {
  border: 1px solid #3f3f46;
  border-radius: 6px;
  overflow: hidden;
  background: #1e1e22;
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
  padding: 16px;
  min-height: 150px;
  color: #eee;
  outline: none;
}

:deep(.ProseMirror) {
  outline: none;
  min-height: 150px;
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
</style>
