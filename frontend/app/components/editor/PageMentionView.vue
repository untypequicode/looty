<script setup lang="ts">
import { NodeViewWrapper, nodeViewProps } from '@tiptap/vue-3'
import { FileText, FileX } from 'lucide-vue-next'
import { useWorkspace } from '~/composables/useWorkspace'

/** Mention de page : titre et icône à jour ; page supprimée ou inaccessible barrée. */
const props = defineProps(nodeViewProps)
const { pages } = useWorkspace()
const page = computed(() => (pages.data.value ?? []).find((p) => p.id === props.node.attrs.id))
const known = computed(() => !!pages.data.value)
const missing = computed(() => known.value && !page.value)
const label = computed(() => page.value?.title || props.node.attrs.label || '…')
const emoji = computed(() => (page.value?.icon && !page.value.icon.startsWith('http') ? page.value.icon : null))

function open(e: MouseEvent) {
  if (missing.value || !props.node.attrs.href) return
  e.preventDefault()
  if (e.ctrlKey || e.metaKey) window.open(props.node.attrs.href, '_blank')
  else navigateTo(props.node.attrs.href)
}
</script>

<template>
  <NodeViewWrapper as="span" class="page-mention-wrap">
    <a :href="node.attrs.href" class="page-mention" :class="{ missing, selected }" contenteditable="false" data-type="page-mention" :data-page-id="node.attrs.id" @click="open">
      <span v-if="emoji" class="pm-emoji">{{ emoji }}</span>
      <component :is="missing ? FileX : FileText" v-else :size="13" class="pm-icon" />
      <span class="pm-label">{{ label }}</span>
    </a>
  </NodeViewWrapper>
</template>

<style scoped>
.page-mention { display: inline-flex; align-items: center; gap: 3px; padding: 0 4px 0 3px; border-radius: 4px; color: var(--accent-text, var(--accent)); text-decoration: none; background: var(--accent-soft); cursor: pointer; font-weight: 500; line-height: 1.4; }
.page-mention:hover { background: color-mix(in srgb, var(--accent) 22%, transparent); }
.page-mention.selected { box-shadow: 0 0 0 1.5px var(--border-focus); }
.page-mention.missing { color: var(--text-3); background: var(--bg-code); text-decoration: line-through; cursor: default; }
.pm-icon { flex-shrink: 0; opacity: .8; }
.pm-emoji { font-size: .95em; }
</style>
