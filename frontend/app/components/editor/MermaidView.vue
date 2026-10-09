<script setup lang="ts">
import { NodeViewWrapper, nodeViewProps } from '@tiptap/vue-3'
import { Code2, Eye, Columns2 } from 'lucide-vue-next'
import { useDebounceFn } from '@vueuse/core'
import { useTheme } from '~/composables/useTheme'
import { nodeSource, useSourceField } from '~/editor/source'

/** Diagramme Mermaid : aperçu SVG + éditeur côte à côte (mode strict, sans HTML arbitraire). */
const props = defineProps(nodeViewProps)
const { mode } = useTheme()
const area = ref<HTMLTextAreaElement | null>(null)
// Le code est le texte du nœud : chaque frappe est fusionnée avec celles des autres.
const field = useSourceField(area, props as any)
const code = field.draft
const svg = ref('')
const error = ref('')
const view = ref<'preview' | 'split' | 'code'>(nodeSource(toRaw(props.node)) ? 'preview' : 'split')
let seq = 0

const isDark = () => document.documentElement.classList.contains('dark') || !document.documentElement.classList.contains('light')

async function render() {
  const id = `mmd-${Math.random().toString(36).slice(2)}-${++seq}`
  if (!code.value.trim()) { svg.value = ''; error.value = ''; return }
  try {
    const mermaid = (await import('mermaid')).default
    mermaid.initialize({ startOnLoad: false, securityLevel: 'strict', theme: isDark() ? 'dark' : 'neutral', fontFamily: 'Inter, sans-serif' })
    const out = await mermaid.render(id, code.value)
    svg.value = out.svg
    error.value = ''
  } catch (e: any) {
    error.value = String(e?.message ?? e).split('\n').slice(0, 3).join('\n')
    document.getElementById(`d${id}`)?.remove()
  }
}
const debounced = useDebounceFn(render, 350)
watch(code, () => debounced())
watch(mode, () => render())
onMounted(render)

function onKey(e: KeyboardEvent) {
  e.stopPropagation()
  field.onHistoryKey(e)
}
</script>

<template>
  <NodeViewWrapper class="mmd" :class="{ selected }" data-drag-handle>
    <div class="mmd-bar" contenteditable="false">
      <span class="mmd-title">Mermaid</span>
      <span class="spacer" />
      <UiSegmented
        v-if="editor.isEditable"
        v-model="view"
        :options="[{ value: 'preview', icon: Eye, title: $t('embeds.preview') }, { value: 'split', icon: Columns2, title: $t('embeds.split') }, { value: 'code', icon: Code2, title: $t('embeds.code') }]"
      />
    </div>
    <div class="mmd-body" :class="view" contenteditable="false">
      <textarea v-if="editor.isEditable && view !== 'preview'" ref="area" v-model="code" class="mmd-code" spellcheck="false" placeholder="graph TD&#10;  A[Début] --> B{Choix}" @keydown="onKey" @beforeinput="field.onBeforeInput" />
      <div v-if="view !== 'code'" class="mmd-preview">
        <div v-if="svg" class="mmd-svg" v-html="svg" />
        <pre v-else-if="error" class="mmd-error">{{ error }}</pre>
        <span v-else class="faint">{{ $t('embeds.mermaidEmpty') }}</span>
      </div>
    </div>
  </NodeViewWrapper>
</template>

<style scoped>
.mmd { margin: 1em 0; border-radius: var(--r-lg); background: var(--bg-column); box-shadow: var(--shadow-card); overflow: hidden; }
.mmd.selected { box-shadow: 0 0 0 1.5px var(--border-focus); }
.mmd-bar { display: flex; align-items: center; gap: 8px; height: 36px; padding: 0 8px 0 12px; border-bottom: 1px solid var(--border); background: var(--bg-panel); }
.mmd-title { font-size: var(--fs-xs); font-weight: 600; color: var(--text-3); text-transform: uppercase; letter-spacing: 0.04em; }
.mmd-body { display: grid; min-height: 120px; }
.mmd-body.split { grid-template-columns: 1fr 1fr; }
.mmd-code {
  border: none; outline: none; resize: vertical; background: var(--bg-code); color: var(--text); min-height: 180px; padding: 12px 14px;
  font-family: var(--font-mono); font-size: 12.5px; line-height: 1.55; border-right: 1px solid var(--border);
}
.mmd-preview { display: flex; align-items: center; justify-content: center; padding: 16px; overflow: auto; }
.mmd-svg { max-width: 100%; }
.mmd-svg :deep(svg) { max-width: 100%; height: auto; }
.mmd-error { margin: 0; color: var(--red); font-size: var(--fs-sm); white-space: pre-wrap; font-family: var(--font-mono); }
@media (max-width: 639px) { .mmd-body.split { grid-template-columns: 1fr; } .mmd-code { border-right: none; border-bottom: 1px solid var(--border); } }
</style>
