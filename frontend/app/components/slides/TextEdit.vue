<script setup lang="ts">
import type * as Y from 'yjs'
import { Editor, EditorContent } from '@tiptap/vue-3'
import StarterKit from '@tiptap/starter-kit'
import { Collaboration } from '@tiptap/extension-collaboration'
import { CollaborationCaret } from '@tiptap/extension-collaboration-caret'

/**
 * Édition du texte d'une zone de texte : TipTap lié au fragment Yjs de l'élément (même schéma que
 * les pages, sans l'historique de l'éditeur : l'annulation de la frappe suit celle de Yjs). Avec un
 * `provider`, les curseurs et sélections des autres personnes qui écrivent dans la même zone sont affichés
 * (comme dans les pages) ; `readonly` : vue en direct d'une zone qu'une autre personne est en train de modifier.
 */
const props = defineProps<{ fragment: Y.XmlFragment; textStyle: Record<string, string | undefined>; provider?: unknown; user?: { name: string; color: string }; readonly?: boolean }>()
const emit = defineEmits<{ (e: 'done'): void; (e: 'editor', ed: Editor | null): void }>()
const editor = new Editor({
  extensions: [
    StarterKit.configure({ undoRedo: false, heading: { levels: [1, 2, 3] }, codeBlock: false, link: { openOnClick: false, HTMLAttributes: { rel: 'noopener noreferrer nofollow' } } }),
    Collaboration.configure({ fragment: props.fragment }),
    ...(props.provider ? [CollaborationCaret.configure({ provider: props.provider as any, user: props.user ?? { name: '?', color: '#7170ff' } })] : []),
  ],
  editable: !props.readonly,
  editorProps: { attributes: { class: 'slide-text-edit', 'aria-label': 'Text' } },
  autofocus: props.readonly ? false : 'end',
  onBlur: ({ event }) => {
    if (props.readonly) return
    const to = (event as FocusEvent | undefined)?.relatedTarget as HTMLElement | null
    if (to?.closest?.('[data-slides-props]')) return
    emit('done')
  },
})
onMounted(() => { if (!props.readonly) emit('editor', editor) })
onBeforeUnmount(() => { if (!props.readonly) emit('editor', null); editor.destroy() })
function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') { e.stopPropagation(); emit('done') }
}
</script>

<template>
  <div class="te slide-text" :class="{ live: readonly }" :style="textStyle" @keydown="onKey" @pointerdown.stop="!readonly">
    <EditorContent :editor="editor" class="te-c" />
  </div>
</template>

<style scoped>
.te { width: 100%; height: 100%; line-height: 1.2; outline: 2px solid var(--accent); outline-offset: 0; cursor: text; }
.te.live { outline: none; pointer-events: none; cursor: inherit; }
.te :deep(.collaboration-carets__caret) { position: relative; margin-left: -1px; margin-right: -1px; border-left: 0.07em solid; border-right: 0.07em solid; border-color: currentcolor; word-break: normal; pointer-events: none; }
.te :deep(.collaboration-carets__label) { position: absolute; top: -1.5em; left: -0.07em; padding: 0 0.35em; border-radius: 0.2em 0.2em 0.2em 0; font-size: 0.32em; font-weight: 600; font-style: normal; line-height: 1.6; color: #fff; white-space: nowrap; user-select: none; }
.te-c, .te-c :deep(.ProseMirror) { height: 100%; outline: none; overflow-wrap: anywhere; }
.te :deep(p), .te :deep(h1), .te :deep(h2), .te :deep(h3), .te :deep(ul), .te :deep(ol), .te :deep(blockquote) { margin: 0 0 0.35em; }
.te :deep(h1) { font-size: 1.6em; } .te :deep(h2) { font-size: 1.35em; } .te :deep(h3) { font-size: 1.15em; }
.te :deep(ul), .te :deep(ol) { padding-left: 1.4em; }
.te :deep(blockquote) { border-left: 0.12em solid var(--border-strong); padding-left: 0.6em; }
.te :deep(a) { color: var(--accent-text); text-decoration: underline; }
</style>
