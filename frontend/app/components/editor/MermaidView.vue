<script setup lang="ts">
import { NodeViewWrapper, nodeViewProps } from '@tiptap/vue-3'
import { Code2, Eye, Columns2, Maximize2 } from 'lucide-vue-next'
import { useDebounceFn } from '@vueuse/core'
import { useTheme } from '~/composables/useTheme'
import { nodeSource, useSourceField } from '~/editor/source'
import { renderMermaid } from '~/editor/mermaid'
import { diagramStyleSettings, resolveStyle } from '~/editor/diagramStyles'

/** Diagramme Mermaid : aperçu SVG + éditeur côte à côte (mode strict, sans HTML arbitraire). */
const props = defineProps(nodeViewProps)
const { mode } = useTheme()
const area = ref<HTMLTextAreaElement | null>(null)
// Le code est le texte du nœud : chaque frappe est fusionnée avec celles des autres.
const field = useSourceField(area, props as any)
const code = field.draft
const carets = field.carets
const present = field.present
const svg = ref('')
const error = ref('')
const full = ref(false)
const view = ref<'preview' | 'split' | 'code'>(nodeSource(toRaw(props.node)) ? 'preview' : 'split')
let seq = 0

const isDark = () => document.documentElement.classList.contains('dark') || !document.documentElement.classList.contains('light')
const dark = ref(false)

// Style du bloc (`null` : celui de l'espace, qui peut changer à tout moment).
const styleId = computed<string | null>({
  get: () => props.node.attrs.diagramStyle ?? null,
  set: (v) => props.updateAttributes({ diagramStyle: v }),
})
const style = computed(() => resolveStyle(styleId.value, diagramStyleSettings.value))
const background = computed(() => (dark.value ? style.value.dark : style.value.light).background)

async function render() {
  dark.value = isDark()
  const id = `mmd-${Math.random().toString(36).slice(2)}-${++seq}`
  if (!code.value.trim()) { svg.value = ''; error.value = ''; return }
  try {
    const out = await renderMermaid(id, code.value, dark.value, style.value)
    svg.value = out.svg
    error.value = ''
  } catch (e: any) {
    error.value = String(e?.message ?? e).split('\n').slice(0, 3).join('\n')
    document.getElementById(`d${id}`)?.remove()
  }
}
const debounced = useDebounceFn(render, 350)
watch(code, () => debounced())
watch([mode, style], () => render())
onMounted(render)

function onKey(e: KeyboardEvent) {
  e.stopPropagation()
  field.onHistoryKey(e)
}
</script>

<template>
  <NodeViewWrapper class="mmd" :class="{ selected, 'has-who': present.length }" :style="present.length ? { '--who': present[0]!.color } : undefined" data-drag-handle>
    <div class="mmd-bar" contenteditable="false">
      <span class="mmd-title">{{ $t('diagrams.diagram') }}</span>
      <span v-if="present.length && !carets.length" class="mmd-who"><i v-for="p in present" :key="p.key" :style="{ background: p.color }">{{ p.name }}</i></span>
      <span class="spacer" />
      <EditorDiagramStylePicker v-model="styleId" :editable="editor.isEditable" />
      <button v-if="svg" class="mmd-btn" :title="$t('viewer.fullscreen')" @click="full = true"><Maximize2 :size="14" /></button>
      <UiSegmented
        v-if="editor.isEditable"
        v-model="view"
        :options="[{ value: 'preview', icon: Eye, title: $t('embeds.preview') }, { value: 'split', icon: Columns2, title: $t('embeds.split') }, { value: 'code', icon: Code2, title: $t('embeds.code') }]"
      />
    </div>
    <div class="mmd-body" :class="view" contenteditable="false">
      <div v-if="editor.isEditable && view !== 'preview'" class="mmd-code-wrap">
        <textarea ref="area" v-model="code" class="mmd-code" spellcheck="false" :placeholder="$t('embeds.mermaidPlaceholder')" @keydown="onKey" @beforeinput="field.onBeforeInput" />
        <EditorSourceCursors :carets="carets" />
      </div>
      <div v-if="view !== 'code'" class="mmd-preview" :style="{ background }" @dblclick="svg && (full = true)">
        <div v-if="svg" class="mmd-svg" v-html="svg" />
        <pre v-else-if="error" class="mmd-error">{{ error }}</pre>
        <span v-else class="faint">{{ $t('embeds.mermaidEmpty') }}</span>
      </div>
    </div>
    <EditorDiagramViewer v-model:open="full" :svg="svg" :background="background" :title="$t('diagrams.diagram')" />
  </NodeViewWrapper>
</template>

<style scoped>
.mmd { margin: 1em 0; border-radius: var(--r-lg); background: var(--bg-column); box-shadow: var(--shadow-card); overflow: hidden; }
.mmd.selected { box-shadow: 0 0 0 1.5px var(--border-focus); }
.mmd.has-who { box-shadow: 0 0 0 1.5px var(--who); }
.mmd-who { display: inline-flex; gap: 3px; }
.mmd-who i { padding: 0 6px; border-radius: 4px; color: #fff; font: 600 10px/15px var(--font-sans, system-ui, sans-serif); font-style: normal; }
.mmd-code-wrap { position: relative; display: grid; min-width: 0; border-right: 1px solid var(--border); }
.mmd-bar { display: flex; align-items: center; gap: 6px; height: 36px; padding: 0 8px 0 12px; border-bottom: 1px solid var(--border); background: var(--bg-panel); }
.mmd-title { font-size: var(--fs-xs); font-weight: 600; color: var(--text-3); text-transform: uppercase; letter-spacing: 0.04em; }
.mmd-btn { display: inline-grid; place-items: center; width: 26px; height: 26px; border-radius: var(--r-sm); color: var(--text-2); }
.mmd-btn:hover { background: var(--bg-hover); color: var(--text); }
.mmd-body { display: grid; min-height: 120px; }
.mmd-body.split { grid-template-columns: 1fr 1fr; }
.mmd-code {
  border: none; outline: none; resize: vertical; background: var(--bg-code); color: var(--text); min-height: 180px; padding: 12px 14px;
  font-family: var(--font-mono); font-size: 12.5px; line-height: 1.55; width: 100%;
}
.mmd-preview { display: flex; align-items: center; justify-content: center; min-width: 0; padding: 16px; overflow: auto; }
/*
 * Le SVG de Mermaid a `width="100%"` et sa taille naturelle en `max-width` : dans un parent
 * qui prend toute la largeur, il garde sa taille et ne rétrécit que s'il est plus large
 * (un parent ajusté à son contenu le réduisait à la taille par défaut d'un SVG).
 */
.mmd-svg { width: 100%; display: flex; justify-content: center; }
.mmd-svg :deep(svg) { height: auto; }
.mmd-error { margin: 0; color: var(--red); font-size: var(--fs-sm); white-space: pre-wrap; font-family: var(--font-mono); }
@media (max-width: 639px) { .mmd-body.split { grid-template-columns: 1fr; } .mmd-code-wrap { border-right: none; border-bottom: 1px solid var(--border); } }
</style>
