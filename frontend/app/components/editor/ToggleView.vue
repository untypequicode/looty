<script setup lang="ts">
import { NodeViewWrapper, NodeViewContent, nodeViewProps } from '@tiptap/vue-3'
import { ChevronRight } from 'lucide-vue-next'

const props = defineProps(nodeViewProps)
// L'état ouvert/fermé est local à chaque lecteur s'il ne peut pas éditer.
const localOpen = ref(props.node.attrs.open)
watch(() => props.node.attrs.open, (v) => (localOpen.value = v))
function toggle() {
  localOpen.value = !localOpen.value
  if (props.editor.isEditable) props.updateAttributes({ open: localOpen.value })
}
</script>

<template>
  <NodeViewWrapper class="toggle-node" :class="{ closed: !localOpen }">
    <button class="toggle-btn" contenteditable="false" :aria-expanded="localOpen" @click="toggle"><ChevronRight :size="14" /></button>
    <NodeViewContent class="toggle-inner" />
  </NodeViewWrapper>
</template>

<style scoped>
.toggle-node { position: relative; padding-left: 26px; margin: 0.4em 0; }
.toggle-btn {
  position: absolute; left: 0; top: 2px; width: 22px; height: 22px; display: grid; place-items: center; border-radius: var(--r-xs);
  color: var(--text-3); transform: rotate(90deg); transition: transform var(--dur) var(--ease), background var(--dur-fast);
}
.toggle-node.closed .toggle-btn { transform: rotate(0deg); }
.toggle-btn:hover { background: var(--bg-hover); color: var(--text); }
.toggle-inner :deep(summary) { display: block; list-style: none; font-weight: 600; }
.toggle-inner :deep(summary)::-webkit-details-marker { display: none; }
.toggle-inner :deep(summary:empty)::before { content: attr(data-placeholder); color: var(--text-4); }
.toggle-node.closed .toggle-inner :deep([data-type='toggle-content']) { display: none; }
.toggle-inner :deep([data-type='toggle-content']) { margin-top: 4px; }
</style>
