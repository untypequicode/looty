<script setup lang="ts">
import { NodeViewWrapper, nodeViewProps } from '@tiptap/vue-3'

defineProps(nodeViewProps)
</script>

<template>
  <NodeViewWrapper class="pdf-node" :class="{ selected }" data-drag-handle>
    <ViewerPdf :src="node.attrs.src" :name="node.attrs.name" height="620px" />
  </NodeViewWrapper>
</template>

<style scoped>
.pdf-node.selected :deep(.pdf) { box-shadow: 0 0 0 1.5px var(--border-focus); }
</style>
