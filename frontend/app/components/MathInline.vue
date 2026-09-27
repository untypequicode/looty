<template>
  <node-view-wrapper as="span" class="math-inline" style="display: inline-block;">
    <span v-if="isEditing" class="math-inline-edit" @keydown.stop="onKeyDown">
      <input 
        v-model="latex" 
        @blur="isEditing = false"
        ref="inputRef"
        type="text"
        class="modern-input inline-input"
        placeholder="LaTeX"
      />
    </span>
    <span v-else class="math-inline-render" @click="enableEdit" title="Cliquez pour modifier">
      <span v-if="latex" v-html="renderedMath"></span>
      <span v-else class="empty-math">$...$</span>
    </span>
  </node-view-wrapper>
</template>

<script setup>
import { nodeViewProps, NodeViewWrapper } from '@tiptap/vue-3'
import { ref, computed, nextTick, watch } from 'vue'
import katex from 'katex'

const props = defineProps(nodeViewProps)

const isEditing = ref(false)
const inputRef = ref(null)

watch(() => props.selected, (isSelected) => {
  if (isSelected) {
    enableEdit()
  }
})

const latex = computed({
  get: () => props.node.attrs.latex,
  set: (val) => {
    props.updateAttributes({ latex: val })
  }
})

const renderedMath = computed(() => {
  try {
    return katex.renderToString(latex.value || '', {
      displayMode: false,
      throwOnError: false
    })
  } catch (e) {
    return `<span style="color: red;">Erreur LaTeX</span>`
  }
})

async function enableEdit() {
  isEditing.value = true
  await nextTick()
  if (inputRef.value) {
    inputRef.value.focus()
  }
}

function onKeyDown(e) {
  if (e.key === 'Escape' || e.key === 'Enter') {
    e.preventDefault()
    isEditing.value = false
    props.editor.commands.focus()
    return
  }
  
  if (e.key === 'ArrowLeft' && inputRef.value && inputRef.value.selectionStart === 0) {
    isEditing.value = false
    props.editor.commands.focus(props.getPos() - 1)
  }
  
  if (e.key === 'ArrowRight' && inputRef.value && inputRef.value.selectionStart === latex.value.length) {
    isEditing.value = false
    props.editor.commands.focus(props.getPos() + 2) // +2 to skip the node itself
  }
}
</script>

<style scoped>
.math-inline {
  cursor: pointer;
  margin: 0 2px;
}
.math-inline-render {
  padding: 0 4px;
  background: var(--bg-color-2);
  border-radius: 4px;
  border: 1px solid transparent;
  transition: border-color 0.2s;
  color: var(--text-color);
}
.math-inline-render:hover {
  border-color: var(--border-color);
}
.inline-input {
  width: 150px;
  padding: 2px 6px;
  font-family: monospace;
  font-size: 0.9rem;
  margin: 0;
  display: inline-block;
  height: auto;
  background: var(--bg-color-main);
  color: var(--text-color);
  border: 1px solid var(--primary-color);
  outline: none;
}
.inline-input:focus {
  box-shadow: 0 0 0 2px rgba(79, 70, 229, 0.2);
}
.empty-math {
  color: var(--text-color-3);
  font-style: italic;
}
</style>
