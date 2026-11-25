<script setup lang="ts">
import type * as Y from 'yjs'
import { Editor, EditorContent } from '@tiptap/vue-3'
import StarterKit from '@tiptap/starter-kit'
import { Collaboration } from '@tiptap/extension-collaboration'

/**
 * Édition du texte d'une zone de texte : TipTap lié au fragment Yjs de l'élément (même schéma que
 * les pages, sans l'historique de l'éditeur : l'annulation de la frappe suit celle de Yjs).
 */
const props = defineProps<{ fragment: Y.XmlFragment; textStyle: Record<string, string | undefined> }>()
const emit = defineEmits<{ (e: 'done'): void; (e: 'editor', ed: Editor | null): void }>()
const editor = new Editor({
  extensions: [
    StarterKit.configure({ undoRedo: false, heading: { levels: [1, 2, 3] }, codeBlock: false, link: { openOnClick: false, HTMLAttributes: { rel: 'noopener noreferrer nofollow' } } }),
    Collaboration.configure({ fragment: props.fragment }),
  ],
  editorProps: { attributes: { class: 'slide-text-edit', 'aria-label': 'Text' } },
  autofocus: 'end',
  onBlur: ({ event }) => {
    const to = (event as FocusEvent | undefined)?.relatedTarget as HTMLElement | null
    if (to?.closest?.('[data-slides-props]')) return
    emit('done')
  },
})
onMounted(() => emit('editor', editor))
onBeforeUnmount(() => { emit('editor', null); editor.destroy() })
function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') { e.stopPropagation(); emit('done') }
}
</script>

<template>
  <div class="te slide-text" :style="textStyle" @keydown="onKey" @pointerdown.stop>
    <EditorContent :editor="editor" class="te-c" />
  </div>
</template>

<style scoped>
.te { width: 100%; height: 100%; line-height: 1.2; outline: 2px solid var(--accent); outline-offset: 0; cursor: text; }
.te-c, .te-c :deep(.ProseMirror) { height: 100%; outline: none; overflow-wrap: anywhere; }
.te :deep(p), .te :deep(h1), .te :deep(h2), .te :deep(h3), .te :deep(ul), .te :deep(ol), .te :deep(blockquote) { margin: 0 0 0.35em; }
.te :deep(h1) { font-size: 1.6em; } .te :deep(h2) { font-size: 1.35em; } .te :deep(h3) { font-size: 1.15em; }
.te :deep(ul), .te :deep(ol) { padding-left: 1.4em; }
.te :deep(blockquote) { border-left: 0.12em solid var(--border-strong); padding-left: 0.6em; }
.te :deep(a) { color: var(--accent-text); text-decoration: underline; }
</style>
