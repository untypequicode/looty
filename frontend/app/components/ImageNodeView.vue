<template>
  <node-view-wrapper class="image-resizer">
    <div 
      class="image-container" 
      :style="{ width: node.attrs.width }"
      :class="{ 'is-selected': selected }"
    >
      <img :src="node.attrs.src" :alt="node.attrs.alt" :title="node.attrs.title" />
      <div 
        v-if="editor.isEditable"
        class="resize-handle"
        @mousedown="startResize"
      ></div>
    </div>
  </node-view-wrapper>
</template>

<script setup>
import { ref } from 'vue'
import { NodeViewWrapper, nodeViewProps } from '@tiptap/vue-3'

const props = defineProps(nodeViewProps)
const isResizing = ref(false)
let startX = 0
let startWidth = 0

const startResize = (event) => {
  event.preventDefault()
  isResizing.value = true
  startX = event.clientX
  
  const container = event.target.closest('.image-container')
  startWidth = container.offsetWidth
  
  document.addEventListener('mousemove', onMouseMove)
  document.addEventListener('mouseup', onMouseUp)
}

const onMouseMove = (event) => {
  if (!isResizing.value) return
  
  const dx = event.clientX - startX
  const newWidth = Math.max(100, startWidth + dx) // min width 100px
  
  props.updateAttributes({
    width: `${newWidth}px`,
  })
}

const onMouseUp = () => {
  isResizing.value = false
  document.removeEventListener('mousemove', onMouseMove)
  document.removeEventListener('mouseup', onMouseUp)
}
</script>

<style scoped>
.image-resizer {
  display: flex;
  justify-content: center;
  margin: 1rem 0;
}

.image-container {
  position: relative;
  display: inline-block;
  max-width: 100%;
}

.image-container img {
  display: block;
  width: 100%;
  height: auto;
  border-radius: 6px;
  user-select: none;
  -webkit-user-drag: none;
}

.image-container.is-selected img {
  outline: 2px solid #5e6ad2;
}

.resize-handle {
  position: absolute;
  right: -5px;
  bottom: -5px;
  width: 15px;
  height: 15px;
  background-color: #5e6ad2;
  border: 2px solid #fff;
  border-radius: 50%;
  cursor: nwse-resize;
  z-index: 10;
  opacity: 0;
  transition: opacity 0.2s;
}

.image-container:hover .resize-handle,
.image-container.is-selected .resize-handle {
  opacity: 1;
}
</style>
