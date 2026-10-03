<script setup lang="ts">
import { NodeViewWrapper, nodeViewProps } from '@tiptap/vue-3'
import { AlignLeft, AlignCenter, Maximize2, Trash2, Download } from 'lucide-vue-next'

const props = defineProps(nodeViewProps)
const box = ref<HTMLElement | null>(null)
const resizing = ref(false)
const align = computed(() => props.node.attrs.align ?? 'center')

/** Redimensionnement par la poignée, en % de la largeur du document. */
function startResize(e: PointerEvent, side: 'left' | 'right') {
  e.preventDefault()
  e.stopPropagation()
  const el = box.value
  const parent = el?.closest('.ProseMirror') as HTMLElement | null
  if (!el || !parent) return
  resizing.value = true
  const startX = e.clientX
  const startW = el.offsetWidth
  const max = parent.clientWidth
  ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
  const move = (ev: PointerEvent) => {
    const dx = (ev.clientX - startX) * (side === 'right' ? 1 : -1) * (align.value === 'center' ? 2 : 1)
    const w = Math.min(100, Math.max(15, ((startW + dx) / max) * 100))
    el.style.width = `${w}%`
  }
  const up = () => {
    resizing.value = false
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', up)
    props.updateAttributes({ width: el.style.width })
  }
  window.addEventListener('pointermove', move)
  window.addEventListener('pointerup', up)
}
</script>

<template>
  <NodeViewWrapper class="img-node" :class="[`align-${align}`, { selected, resizing }]">
    <div ref="box" class="img-box" :style="{ width: node.attrs.width || undefined }" data-drag-handle>
      <img :src="node.attrs.src" :alt="node.attrs.alt || ''" :title="node.attrs.title || undefined" draggable="false" loading="lazy" />
      <template v-if="editor.isEditable">
        <span class="handle left" @pointerdown="startResize($event, 'left')" />
        <span class="handle right" @pointerdown="startResize($event, 'right')" />
        <div class="img-tools" contenteditable="false">
          <button :class="{ on: align === 'left' }" :title="$t('editor.alignLeft')" @click="updateAttributes({ align: 'left' })"><AlignLeft :size="14" /></button>
          <button :class="{ on: align === 'center' }" :title="$t('editor.alignCenter')" @click="updateAttributes({ align: 'center' })"><AlignCenter :size="14" /></button>
          <button :title="$t('editor.fullWidth')" @click="updateAttributes({ width: '100%' })"><Maximize2 :size="14" /></button>
          <a :href="node.attrs.src" download :title="$t('common.download')"><Download :size="14" /></a>
          <button :title="$t('common.delete')" @click="deleteNode()"><Trash2 :size="14" /></button>
        </div>
      </template>
    </div>
  </NodeViewWrapper>
</template>

<style scoped>
.img-node { display: flex; margin: 1em 0; }
.img-node.align-center { justify-content: center; }
.img-node.align-left { justify-content: flex-start; }
.img-box { position: relative; max-width: 100%; line-height: 0; }
.img-box img { width: 100%; height: auto; border-radius: var(--r-md); }
.img-node.selected img { outline: 2px solid var(--accent); outline-offset: 2px; }
.handle {
  position: absolute; top: 50%; width: 6px; height: 44px; margin-top: -22px; border-radius: 4px;
  background: var(--bg-elevated); box-shadow: 0 0 0 1px var(--border-strong), var(--shadow-md); cursor: ew-resize; opacity: 0; transition: opacity var(--dur-fast);
}
.handle.left { left: 8px; }
.handle.right { right: 8px; }
.img-box:hover .handle, .img-node.selected .handle, .img-node.resizing .handle { opacity: 1; }
.img-tools {
  position: absolute; top: 8px; right: 8px; display: flex; gap: 2px; padding: 3px; line-height: 1;
  border-radius: var(--r-md); background: var(--bg-elevated); box-shadow: var(--shadow-lg); opacity: 0; transition: opacity var(--dur-fast);
}
.img-box:hover .img-tools, .img-node.selected .img-tools { opacity: 1; }
.img-tools button, .img-tools a { width: 26px; height: 26px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-2); }
.img-tools button:hover, .img-tools a:hover { background: var(--bg-hover); color: var(--text); }
.img-tools .on { color: var(--accent-text); background: var(--accent-soft); }
</style>
