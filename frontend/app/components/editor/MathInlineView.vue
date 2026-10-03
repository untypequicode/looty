<script setup lang="ts">
import { NodeViewWrapper, nodeViewProps } from '@tiptap/vue-3'
import katex from 'katex'
import 'katex/dist/katex.min.css'

const props = defineProps(nodeViewProps)
const editing = ref(false)
const draft = ref(props.node.attrs.latex as string)
const input = ref<HTMLInputElement | null>(null)
watch(() => props.node.attrs.latex, (v) => { if (!editing.value) draft.value = v })
watch(() => props.selected, (s) => { if (s && props.editor.isEditable) start() })

const html = computed(() => {
  try { return katex.renderToString(draft.value || '', { displayMode: false, throwOnError: true, trust: false }) } catch { return '' }
})
function start() {
  if (!props.editor.isEditable) return
  editing.value = true
  nextTick(() => input.value?.focus())
}
function commit() {
  editing.value = false
  if (!draft.value.trim()) {
    props.deleteNode()
    return
  }
  if (draft.value !== props.node.attrs.latex) props.updateAttributes({ latex: draft.value })
}
function onKey(e: KeyboardEvent) {
  if (e.key === 'Enter' || e.key === 'Escape') {
    e.preventDefault()
    commit()
    const pos = (props.getPos() as number) + props.node.nodeSize
    props.editor.chain().focus().setTextSelection(pos).run()
  }
}
</script>

<template>
  <NodeViewWrapper as="span" class="math-inline" :class="{ selected }">
    <span v-if="editing" class="mi-edit" contenteditable="false">
      <span class="mi-dollar">$</span><input ref="input" v-model="draft" spellcheck="false" :size="Math.max(4, draft.length)" @keydown.stop="onKey" @blur="commit" /><span class="mi-dollar">$</span>
    </span>
    <span v-else-if="html" class="mi-render" @click="start" v-html="html" />
    <span v-else class="mi-render mi-error" @click="start">{{ draft || '$…$' }}</span>
  </NodeViewWrapper>
</template>

<style scoped>
.math-inline { display: inline-block; border-radius: 4px; padding: 0 2px; }
.math-inline.selected { box-shadow: 0 0 0 1.5px var(--border-focus); }
.mi-render { cursor: pointer; }
.mi-render:hover { background: var(--bg-hover); border-radius: 4px; }
.mi-error { color: var(--red); font-family: var(--font-mono); font-size: 0.85em; }
.mi-edit { display: inline-flex; align-items: center; background: var(--bg-code); border-radius: 4px; padding: 0 4px; font-family: var(--font-mono); font-size: 0.85em; }
.mi-edit input { border: none; outline: none; background: transparent; font: inherit; color: var(--text); }
.mi-dollar { color: var(--text-3); }
</style>
