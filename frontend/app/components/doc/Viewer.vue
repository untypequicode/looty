<script setup lang="ts">
import { useEditor, EditorContent } from '@tiptap/vue-3'
import { readonlyExtensions } from '~/editor/extensions'

/**
 * Rendu en lecture seule d'un contenu HTML d'éditeur. Passer par TipTap (et non v-html)
 * garantit que seul le schéma connu est rendu : aucun script ni attribut arbitraire.
 */
const props = withDefaults(defineProps<{ html?: string | null; size?: 'sm' | 'md' }>(), { size: 'md' })

const editor = useEditor({
  editable: false,
  content: props.html || '',
  extensions: readonlyExtensions(),
})
watch(() => props.html, (h) => {
  if (editor.value && h !== editor.value.getHTML()) editor.value.commands.setContent(h || '', false)
})
onBeforeUnmount(() => editor.value?.destroy())
</script>

<template>
  <EditorContent :editor="editor" class="prose doc-viewer" :class="{ 'prose-sm': size === 'sm' }" />
</template>

<style scoped>
.doc-viewer :deep(.ProseMirror) { outline: none; }
</style>
