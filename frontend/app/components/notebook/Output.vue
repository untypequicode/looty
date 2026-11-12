<script setup lang="ts">
import type { NbOutput } from '~/composables/useNotebookKernel'
import { sanitizeOutputHtml, stripAnsi } from '~/editor/notebookMarkdown'
import { useI18n } from '~/composables/useI18n'

/** Sorties d'une cellule : texte, erreurs, tableaux (HTML nettoyé), images, formules. */
const props = defineProps<{ outputs: NbOutput[] }>()
const { t } = useI18n()
const text = (v: unknown) => (Array.isArray(v) ? v.join('') : String(v ?? ''))
const items = computed(() => props.outputs.map((o, i) => {
  const key = `${i}`
  if (o.output_type === 'stream') return { key, kind: o.name === 'stderr' ? 'stderr' : 'stream', text: text(o.text) }
  // Bibliothèque sans version navigateur (pur Python / WebAssembly) : explication localisée, erreur brute en détail.
  if (o.output_type === 'error' && o.ename === 'WasmUnavailableError') return { key, kind: 'error', note: t('notebook.deps.wasm'), text: stripAnsi(String(o.evalue ?? '')) }
  if (o.output_type === 'error') return { key, kind: 'error', text: stripAnsi((o.traceback?.length ? o.traceback.join('\n') : `${o.ename}: ${o.evalue}`)) }
  const d = o.data ?? {}
  if (d['text/html']) return { key, kind: 'html', html: sanitizeOutputHtml(text(d['text/html'])) }
  if (d['image/png']) return { key, kind: 'image', src: `data:image/png;base64,${text(d['image/png']).replace(/\s/g, '')}` }
  if (d['image/jpeg']) return { key, kind: 'image', src: `data:image/jpeg;base64,${text(d['image/jpeg']).replace(/\s/g, '')}` }
  if (d['image/svg+xml']) return { key, kind: 'image', src: `data:image/svg+xml;utf8,${encodeURIComponent(text(d['image/svg+xml']))}` }
  return { key, kind: o.output_type === 'execute_result' ? 'result' : 'stream', text: text(d['text/plain']) }
}))
</script>

<template>
  <div class="nbo">
    <template v-for="it in items" :key="it.key">
      <img v-if="it.kind === 'image'" class="nbo-img" :src="it.src" alt="" />
      <div v-else-if="it.kind === 'html'" class="nbo-html" v-html="it.html" />
      <div v-else-if="it.note" class="nbo-wasm" data-testid="nb-wasm-note">
        <p>{{ it.note }}</p>
        <pre class="nbo-text stderr">{{ it.text }}</pre>
      </div>
      <pre v-else class="nbo-text" :class="it.kind">{{ it.text }}</pre>
    </template>
  </div>
</template>

<style scoped>
.nbo { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.nbo-text { margin: 0; padding: 4px 0; font-family: var(--font-mono, ui-monospace, monospace); font-size: 12.5px; line-height: 1.5; white-space: pre-wrap; word-break: break-word; color: var(--text); }
.nbo-text.stderr { color: var(--orange, #e2a03f); }
.nbo-text.error { color: var(--red); background: color-mix(in srgb, var(--red) 8%, transparent); padding: 8px 10px; border-radius: var(--r-sm); }
.nbo-text.result { color: var(--text); }
.nbo-wasm { padding: 8px 10px; border-radius: var(--r-sm); background: color-mix(in srgb, var(--red) 8%, transparent); }
.nbo-wasm p { margin: 0 0 4px; font-size: 13px; font-weight: 500; color: var(--red); }
.nbo-img { max-width: 100%; height: auto; align-self: flex-start; background: #fff; border-radius: var(--r-sm); }
.nbo-html { overflow-x: auto; font-size: 13px; }
.nbo-html :deep(table) { border-collapse: collapse; font-size: 12.5px; }
.nbo-html :deep(th), .nbo-html :deep(td) { padding: 3px 10px; border: 1px solid var(--border); text-align: right; }
.nbo-html :deep(th) { background: var(--bg-hover); font-weight: 600; }
.nbo-html :deep(tbody tr:nth-child(odd)) { background: color-mix(in srgb, var(--bg-hover) 50%, transparent); }
</style>
