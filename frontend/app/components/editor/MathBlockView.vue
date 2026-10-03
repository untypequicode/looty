<script setup lang="ts">
import { NodeViewWrapper, nodeViewProps } from '@tiptap/vue-3'
import katex from 'katex'
import 'katex/dist/katex.min.css'

const props = defineProps(nodeViewProps)
const editing = ref(!props.node.attrs.latex)
const draft = ref(props.node.attrs.latex as string)
const area = ref<HTMLTextAreaElement | null>(null)

watch(() => props.node.attrs.latex, (v) => { if (!editing.value) draft.value = v })
watch(() => props.selected, (s) => { if (s && props.editor.isEditable) start() })

const html = computed(() => {
  // KaTeX échappe l'entrée et désactive \href / \url par défaut (trust: false).
  try {
    return katex.renderToString(draft.value || '\\;', { displayMode: true, throwOnError: true, trust: false })
  } catch (e: any) {
    return ''
  }
})
const error = computed(() => {
  try { katex.renderToString(draft.value || '', { throwOnError: true }); return '' } catch (e: any) { return String(e.message ?? e) }
})

function start() {
  if (!props.editor.isEditable) return
  editing.value = true
  nextTick(() => { area.value?.focus(); autosize() })
}
function commit() {
  editing.value = false
  if (draft.value !== props.node.attrs.latex) props.updateAttributes({ latex: draft.value })
}
function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape' || (e.key === 'Enter' && (e.metaKey || e.ctrlKey))) {
    e.preventDefault()
    commit()
    const pos = (props.getPos() as number) + props.node.nodeSize
    props.editor.chain().focus().setTextSelection(Math.min(pos, props.editor.state.doc.content.size)).run()
  }
}
function autosize() {
  if (!area.value) return
  area.value.style.height = 'auto'
  area.value.style.height = `${area.value.scrollHeight}px`
}
</script>

<template>
  <NodeViewWrapper class="math-block" :class="{ selected, editing }" data-drag-handle>
    <div class="math-render" :class="{ empty: !draft }" @click="start">
      <div v-if="draft && html" v-html="html" />
      <span v-else-if="!draft" class="math-placeholder">{{ $t('editor.mathPlaceholder') }}</span>
      <span v-else class="math-error">{{ error }}</span>
    </div>
    <div v-if="editing" class="math-edit" contenteditable="false">
      <textarea ref="area" v-model="draft" spellcheck="false" placeholder="\int_0^1 x^2\,dx" @input="autosize" @keydown.stop="onKey" @blur="commit" />
      <div class="math-hint">LaTeX · <UiKbd :keys="['mod', 'enter']" /> {{ $t('editor.toValidate') }}</div>
    </div>
  </NodeViewWrapper>
</template>

<style scoped>
.math-block { margin: 0.8em 0; border-radius: var(--r-md); transition: background var(--dur-fast), box-shadow var(--dur-fast); }
.math-block.selected, .math-block.editing { box-shadow: 0 0 0 1.5px var(--border-focus); }
.math-render { padding: 6px 12px; cursor: pointer; overflow-x: auto; min-height: 40px; display: flex; align-items: center; justify-content: center; border-radius: var(--r-md); }
.math-render:hover { background: var(--bg-hover); }
.math-placeholder { color: var(--text-3); font-size: var(--fs-md); }
.math-error { color: var(--red); font-size: var(--fs-sm); font-family: var(--font-mono); }
.math-edit { border-top: 1px solid var(--border); padding: 8px 12px; }
.math-edit textarea {
  width: 100%; border: none; outline: none; resize: none; background: transparent; overflow: hidden;
  font-family: var(--font-mono); font-size: 13px; line-height: 1.5; color: var(--text);
}
.math-hint { display: flex; align-items: center; gap: 6px; font-size: var(--fs-xs); color: var(--text-3); }
</style>
