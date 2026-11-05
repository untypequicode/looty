<script setup lang="ts">
import hljs from 'highlight.js/lib/core'
import pythonLang from 'highlight.js/lib/languages/python'
import 'katex/dist/katex.min.css'
import type { NbOutput } from '~/composables/useNotebookKernel'
import { renderMarkdown, type NbAttachments } from '~/editor/notebookMarkdown'
import { useI18n } from '~/composables/useI18n'

/**
 * Notebook en lecture seule (lien public, mode lecture) : le .ipynb tel qu'il est enregistré,
 * texte rendu, code coloré, sorties enregistrées. Ni noyau, ni temps réel.
 */
const props = defineProps<{
  /** Contenu du fichier .ipynb. */
  text: string
  /** Chemin relatif (image, lien) → adresse, ou null. */
  resolve?: ((href: string) => string | null) | null
}>()

const { t } = useI18n()
if (!hljs.getLanguage('python')) hljs.registerLanguage('python', pythonLang)
const join = (v: unknown) => (Array.isArray(v) ? v.join('') : String(v ?? ''))
const escape = (s: string) => s.replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]!)

const parsed = computed(() => {
  try { return JSON.parse(props.text) as { cells?: any[]; metadata?: any } } catch { return null }
})
const language = computed(() => String(parsed.value?.metadata?.language_info?.name ?? parsed.value?.metadata?.kernelspec?.language ?? 'python').toLowerCase())
const cells = computed(() => (parsed.value?.cells ?? []).filter((c) => c && typeof c === 'object').map((c, i) => {
  const source = join(c.source)
  const type: 'code' | 'markdown' | 'raw' = c.cell_type === 'markdown' ? 'markdown' : c.cell_type === 'raw' ? 'raw' : 'code'
  if (type === 'markdown') return { key: `${i}`, type, html: renderMarkdown(source, { attachments: (c.attachments ?? null) as NbAttachments | null, resolve: props.resolve }) }
  if (type === 'raw') return { key: `${i}`, type, code: escape(source) }
  const code = language.value === 'python' ? hljs.highlight(source, { language: 'python', ignoreIllegals: true }).value : escape(source)
  return { key: `${i}`, type, code, count: typeof c.execution_count === 'number' ? c.execution_count : null, outputs: (Array.isArray(c.outputs) ? c.outputs : []) as NbOutput[] }
}))
</script>

<template>
  <div class="nbv" data-testid="nb-viewer">
    <p v-if="!parsed" class="nbv-bad">{{ t('notebook.unreadable') }}</p>
    <div v-for="c in cells" :key="c.key" class="nbv-cell" :class="c.type">
      <div v-if="c.type === 'markdown'" class="nbv-md prose" v-html="c.html" />
      <template v-else>
        <div class="nbv-in">
          <span class="nbv-prompt">{{ c.type === 'code' ? `[${c.count ?? ' '}]` : '' }}</span>
          <pre class="nbv-code"><code class="hljs" v-html="c.code" /></pre>
        </div>
        <div v-if="c.type === 'code' && c.outputs?.length" class="nbv-out"><NotebookOutput :outputs="c.outputs" /></div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.nbv { display: flex; flex-direction: column; gap: 10px; }
.nbv-bad { color: var(--text-3); }
.nbv-md { font-size: inherit; line-height: inherit; }
.nbv-md :deep(img) { max-width: 100%; height: auto; }
.nbv-md :deep(table) { border-collapse: collapse; }
.nbv-md :deep(th), .nbv-md :deep(td) { padding: 3px 10px; border: 1px solid var(--border); }
.nbv-in { display: grid; grid-template-columns: 44px minmax(0, 1fr); gap: 6px; align-items: start; }
.nbv-prompt { padding-top: 9px; font: 11px var(--font-mono, monospace); color: var(--text-3); text-align: right; }
.nbv-code { margin: 0; padding: 8px 12px; border-radius: var(--r-md); background: var(--bg-panel); box-shadow: inset 0 0 0 1px var(--border); overflow-x: auto; font: 13px/1.55 var(--font-mono, ui-monospace, monospace); }
.nbv-code code { background: none; padding: 0; white-space: pre; }
.nbv-out { padding-left: 50px; overflow-x: auto; }
.nbv-cell.raw .nbv-code { color: var(--text-2); }
</style>
