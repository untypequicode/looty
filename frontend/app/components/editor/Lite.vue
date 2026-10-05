<script setup lang="ts">
import { useEditorSpellcheck } from '~/composables/usePrefs'
import { useEditor, EditorContent, BubbleMenu } from '@tiptap/vue-3'
import StarterKit from '@tiptap/starter-kit'
import Placeholder from '@tiptap/extension-placeholder'
import Link from '@tiptap/extension-link'
import Underline from '@tiptap/extension-underline'
import TaskList from '@tiptap/extension-task-list'
import TaskItem from '@tiptap/extension-task-item'
import Image from '@tiptap/extension-image'
import { Bold, Italic, Strikethrough, Code, Link2, List, ListChecks } from 'lucide-vue-next'
import { mentionExtension } from '~/editor/mention'
import { useWorkspace } from '~/composables/useWorkspace'
import { uploadFile } from '~/composables/api'

/**
 * Éditeur riche léger (descriptions de tâches, commentaires) : raccourcis
 * Markdown, liens, listes, cases à cocher, @mentions, collage/dépôt d'images.
 */
const model = defineModel<string>({ default: '' })
const props = withDefaults(defineProps<{
  placeholder?: string
  editable?: boolean
  autofocus?: boolean
  minimal?: boolean
  submitOnEnter?: boolean
}>(), { editable: true })
const emit = defineEmits<{ (e: 'blur'): void; (e: 'submit'): void; (e: 'mention', id: string): void }>()

const { members } = useWorkspace()

async function insertImage(file: File, pos?: number) {
  const r = await uploadFile(file)
  const node = editor.value?.schema.nodes.image.create({ src: r.url, alt: r.name })
  if (!node || !editor.value) return
  const tr = editor.value.state.tr
  if (pos != null) tr.insert(pos, node)
  else tr.replaceSelectionWith(node)
  editor.value.view.dispatch(tr)
}

const editor = useEditor({
  content: model.value,
  editable: props.editable,
  autofocus: props.autofocus ? 'end' : false,
  extensions: [
    StarterKit.configure({ heading: { levels: [2, 3] } }),
    Placeholder.configure({ placeholder: props.placeholder ?? '' }),
    Link.configure({ openOnClick: !props.editable, autolink: true, HTMLAttributes: { rel: 'noopener noreferrer nofollow', target: '_blank' } }),
    Underline,
    TaskList,
    TaskItem.configure({ nested: true }),
    Image.configure({ HTMLAttributes: { loading: 'lazy' } }),
    mentionExtension(() => members.data.value ?? [], (id) => emit('mention', id)),
  ],
  editorProps: {
    attributes: { class: 'prose prose-sm lite-editor' },
    handleKeyDown(_view, event) {
      if (props.submitOnEnter && event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
        emit('submit')
        return true
      }
      return false
    },
    handlePaste(view, event) {
      const file = [...(event.clipboardData?.files ?? [])].find((f) => f.type.startsWith('image/'))
      if (!file) return false
      insertImage(file)
      return true
    },
    handleDrop(view, event, _slice, moved) {
      const file = !moved ? [...(event.dataTransfer?.files ?? [])].find((f) => f.type.startsWith('image/')) : undefined
      if (!file) return false
      const pos = view.posAtCoords({ left: event.clientX, top: event.clientY })?.pos
      insertImage(file, pos)
      return true
    },
  },
  onUpdate: ({ editor: e }) => {
    model.value = e.isEmpty ? '' : e.getHTML()
  },
  onBlur: () => emit('blur'),
})
useEditorSpellcheck(editor)

watch(model, (v) => {
  if (!editor.value) return
  const cur = editor.value.isEmpty ? '' : editor.value.getHTML()
  if (v !== cur) editor.value.commands.setContent(v || '', false)
})
watch(() => props.editable, (v) => editor.value?.setEditable(v))
onBeforeUnmount(() => editor.value?.destroy())

function setLink() {
  const prev = editor.value?.getAttributes('link').href
  const url = window.prompt('URL', prev ?? 'https://')
  if (url === null) return
  if (!url) editor.value?.chain().focus().unsetLink().run()
  else if (/^(https?:|mailto:)/i.test(url)) editor.value?.chain().focus().extendMarkRange('link').setLink({ href: url }).run()
}

defineExpose({ focus: () => editor.value?.commands.focus('end'), clear: () => editor.value?.commands.clearContent(true), editor })
</script>

<template>
  <div class="lite" :class="{ editable }">
    <BubbleMenu v-if="editor && editable && !minimal" :editor="editor" :tippy-options="{ duration: 120, zIndex: 1400 }" class="bubble">
      <button :class="{ on: editor.isActive('bold') }" @click="editor.chain().focus().toggleBold().run()"><Bold :size="14" /></button>
      <button :class="{ on: editor.isActive('italic') }" @click="editor.chain().focus().toggleItalic().run()"><Italic :size="14" /></button>
      <button :class="{ on: editor.isActive('strike') }" @click="editor.chain().focus().toggleStrike().run()"><Strikethrough :size="14" /></button>
      <button :class="{ on: editor.isActive('code') }" @click="editor.chain().focus().toggleCode().run()"><Code :size="14" /></button>
      <button :class="{ on: editor.isActive('link') }" @click="setLink"><Link2 :size="14" /></button>
      <span class="sep" />
      <button :class="{ on: editor.isActive('bulletList') }" @click="editor.chain().focus().toggleBulletList().run()"><List :size="14" /></button>
      <button :class="{ on: editor.isActive('taskList') }" @click="editor.chain().focus().toggleTaskList().run()"><ListChecks :size="14" /></button>
    </BubbleMenu>
    <EditorContent :editor="editor" />
  </div>
</template>

<style scoped>
.lite :deep(.ProseMirror) { outline: none; min-height: 1.6em; }
.lite :deep(.ProseMirror p.is-editor-empty:first-child::before) {
  content: attr(data-placeholder);
  color: var(--text-3);
  float: left;
  height: 0;
  pointer-events: none;
}
.lite :deep(img) { max-height: 420px; object-fit: contain; }
.lite :deep(img.ProseMirror-selectednode) { outline: 2px solid var(--accent); }
.bubble {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 3px;
  background: var(--bg-elevated);
  border-radius: var(--r-md);
  box-shadow: var(--shadow-lg);
}
.bubble button { width: 28px; height: 28px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-2); }
.bubble button:hover { background: var(--bg-hover); color: var(--text); }
.bubble button.on { color: var(--accent-text); background: var(--accent-soft); }
.sep { width: 1px; height: 16px; background: var(--border-strong); margin: 0 2px; }
</style>
