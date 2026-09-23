<template>
  <node-view-wrapper class="math-block">
    <div v-if="isEditing" class="math-edit" @keydown.stop="onKeyDown">
      <textarea 
        v-model="latex" 
        @blur="isEditing = false"
        ref="textareaRef"
        placeholder="Tapez votre formule LaTeX ici..."
        :rows="Math.max(3, (latex || '').split('\n').length)"
        class="modern-input"
        style="font-family: monospace; width: 100%; margin-bottom: 0; resize: vertical;"
      ></textarea>
    </div>
    <div v-else class="math-render" @click="enableEdit" title="Cliquez pour modifier">
      <div v-if="latex" v-html="renderedMath" class="katex-display-container"></div>
      <div v-else class="empty-math">Cliquez pour ajouter une formule LaTeX</div>
    </div>
  </node-view-wrapper>
</template>

<script setup>
import { nodeViewProps, NodeViewWrapper } from '@tiptap/vue-3'
import { ref, computed, nextTick, watch } from 'vue'
import katex from 'katex'

const props = defineProps(nodeViewProps)

const isEditing = ref(false)
const textareaRef = ref(null)

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
      displayMode: true,
      throwOnError: false
    })
  } catch (e) {
    return `<span style="color: red;">Erreur LaTeX: ${e.message}</span>`
  }
})

async function enableEdit() {
  isEditing.value = true
  await nextTick()
  if (textareaRef.value) {
    textareaRef.value.focus()
  }
}

function onKeyDown(e) {
  if (e.key === 'Escape' || (e.key === 'Enter' && !e.shiftKey)) {
    e.preventDefault()
    isEditing.value = false
    props.editor.commands.focus()
    return
  }
  
  if (e.key === 'ArrowUp' && textareaRef.value && textareaRef.value.selectionStart === 0) {
    isEditing.value = false
    props.editor.commands.focus(Math.max(0, props.getPos() - 1))
  }
  
  if (e.key === 'ArrowDown' && textareaRef.value && textareaRef.value.selectionStart === latex.value.length) {
    isEditing.value = false
    const posAfter = props.getPos() + props.node.nodeSize
    if (posAfter >= props.editor.state.doc.content.size) {
      props.editor.commands.insertContentAt(posAfter, '<p></p>')
    }
    props.editor.commands.focus(posAfter + 1)
  }
}
</script>

<style scoped>
.math-block {
  margin: 1rem 0;
  padding: 0.5rem;
  border: 1px solid transparent;
  border-radius: 4px;
  transition: border-color 0.2s;
}
.math-block:hover {
  border-color: #3f3f46;
}
.math-render {
  cursor: pointer;
  padding: 1rem;
  background: #18191c;
  border-radius: 6px;
  text-align: center;
  border: 1px solid transparent;
  transition: border-color 0.2s;
}
.math-render:hover {
  border-color: #3f3f46;
}
textarea.modern-input {
  background: #1e1e22;
  color: #eee;
  border: 1px solid #4f46e5;
  outline: none;
  resize: vertical;
}
textarea.modern-input:focus {
  box-shadow: 0 0 0 2px rgba(79, 70, 229, 0.2);
}
.empty-math {
  color: #8a8f98;
  font-style: italic;
}
.katex-display-container {
  overflow-x: auto;
}
</style>
