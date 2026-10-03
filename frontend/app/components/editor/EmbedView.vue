<script setup lang="ts">
import { NodeViewWrapper, nodeViewProps } from '@tiptap/vue-3'
import { ExternalLink, Trash2 } from 'lucide-vue-next'
import { resolveEmbed } from '~/editor/embeds'

/** Intégration iframe (YouTube, Figma, Loom...) limitée aux fournisseurs autorisés. */
const props = defineProps(nodeViewProps)
const info = computed(() => resolveEmbed(props.node.attrs.src))
</script>

<template>
  <NodeViewWrapper class="emb" :class="{ selected }" data-drag-handle>
    <div v-if="info" class="emb-frame" :style="{ aspectRatio: info.ratio }">
      <iframe
        :src="info.embedUrl"
        :title="info.provider"
        loading="lazy"
        allowfullscreen
        referrerpolicy="strict-origin-when-cross-origin"
        sandbox="allow-scripts allow-same-origin allow-popups allow-presentation allow-forms"
        allow="fullscreen; picture-in-picture; clipboard-write"
      />
    </div>
    <div class="emb-bar" contenteditable="false">
      <span class="emb-provider">{{ info?.provider ?? $t('embeds.link') }}</span>
      <a :href="node.attrs.src" target="_blank" rel="noopener noreferrer" class="emb-url truncate">{{ node.attrs.src }}</a>
      <a :href="node.attrs.src" target="_blank" rel="noopener noreferrer" class="emb-btn"><ExternalLink :size="13" /></a>
      <button v-if="editor.isEditable" class="emb-btn" @click="deleteNode()"><Trash2 :size="13" /></button>
    </div>
  </NodeViewWrapper>
</template>

<style scoped>
.emb { margin: 1em 0; border-radius: var(--r-lg); overflow: hidden; background: var(--bg-column); box-shadow: var(--shadow-card); }
.emb.selected { box-shadow: 0 0 0 1.5px var(--border-focus); }
.emb-frame { width: 100%; background: #000; }
.emb-frame iframe { width: 100%; height: 100%; border: 0; display: block; }
.emb-bar { display: flex; align-items: center; gap: 8px; height: 34px; padding: 0 6px 0 12px; font-size: var(--fs-sm); }
.emb-provider { font-weight: 600; flex: none; }
.emb-url { color: var(--text-3); flex: 1; min-width: 0; }
.emb-btn { width: 26px; height: 26px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-3); }
.emb-btn:hover { background: var(--bg-hover); color: var(--text); }
</style>
