<script setup lang="ts">
import { NodeViewWrapper, nodeViewProps } from '@tiptap/vue-3'
import katex from 'katex'
import 'katex/dist/katex.min.css'
import { Sigma, Package, AlertTriangle } from 'lucide-vue-next'
import { renderMath } from '~/editor/latex'
import { useNodeScope } from '~/editor/mathScope'
import { useLatexField } from '~/editor/latexInput'
import { nodeSource, useSourceField } from '~/editor/source'

const props = defineProps(nodeViewProps)
const editing = ref(!nodeSource(toRaw(props.node)) && props.editor.isEditable)
const area = ref<HTMLTextAreaElement | null>(null)
// Chaque frappe modifie directement la source (texte du nœud) : saisie collaborative.
const field = useSourceField(area, props as any)
const draft = field.draft
const carets = field.carets
const present = field.present
// Source changée par quelqu'un d'autre : le champ grandit avec elle.
watch(draft, () => nextTick(autosize))
watch(() => props.selected, (s) => { if (s && props.editor.isEditable) start() })

// Macros du document : \newcommand, \usepackage… des formules précédentes et de celle-ci.
const scope = useNodeScope(props as any)
const result = computed(() => renderMath(katex, draft.value ?? '', scope.value.scope, true))
const html = computed(() => result.value.html)
const error = computed(() => result.value.error)
const defines = computed(() => result.value.scope.defines)
const packages = computed(() => result.value.scope.packages)
const unknownPackages = computed(() => packages.value.filter((p) => p.status === 'unknown').map((p) => p.name))
/** Formule ne contenant que des déclarations : affichée comme un préambule. */
const preambleOnly = computed(() => !!draft.value?.trim() && !result.value.scope.parsed.rest && !error.value)

function start() {
  if (!props.editor.isEditable) return
  editing.value = true
  nextTick(() => { area.value?.focus(); autosize() })
}
function commit() {
  editing.value = false
}
const latex = useLatexField(area, () => Object.keys(result.value.scope.macros ?? {}))
function onKey(e: KeyboardEvent) {
  if (field.onHistoryKey(e)) return
  if (latex.onKeydown(e)) { autosize(); return }
  if (e.key === 'Escape' || (e.key === 'Enter' && (e.metaKey || e.ctrlKey))) {
    e.preventDefault()
    commit()
    const pos = (props.getPos() as number) + toRaw(props.node).nodeSize
    props.editor.chain().focus().setTextSelection(Math.min(pos, props.editor.state.doc.content.size)).run()
  }
}
function autosize() {
  if (!area.value) return
  area.value.style.height = 'auto'
  area.value.style.height = `${area.value.scrollHeight}px`
}
</script>

<template>
  <NodeViewWrapper class="math-block" :class="{ selected, editing, 'has-who': present.length }" :style="present.length ? { '--who': present[0]!.color } : undefined" data-drag-handle>
    <EditorSourceWho :people="carets.length ? [] : present" />
    <div v-if="preambleOnly" class="math-preamble" @click="start">
      <Sigma :size="13" />
      <span class="mp-label">{{ $t('editor.mathPreamble') }}</span>
      <code v-for="d in defines" :key="d">{{ d }}</code>
      <span v-for="p in packages" :key="p.name" class="mp-pkg" :class="p.status" :title="p.status === 'unknown' ? $t('editor.mathPackageUnknown', { name: p.name }) : ''"><Package :size="11" /> {{ p.name }}</span>
    </div>
    <div v-else class="math-render" :class="{ empty: !draft }" @click="start">
      <div v-if="draft && html" v-html="html" />
      <span v-else-if="!draft" class="math-placeholder">{{ $t('editor.mathPlaceholder') }}</span>
      <span v-else class="math-error">{{ error }}</span>
    </div>
    <div v-if="editing" class="math-edit" contenteditable="false">
      <div class="math-field">
        <textarea ref="area" v-model="draft" spellcheck="false" placeholder="\int_0^1 x^2\,dx" @input="autosize(); latex.refresh()" @click="latex.refresh()" @keydown.stop="onKey" @beforeinput="field.onBeforeInput" @blur="latex.close(); commit()" />
        <EditorLatexSuggest :items="latex.items.value" :active="latex.active.value" @pick="(c) => { latex.pick(c); autosize() }" />
        <EditorSourceCursors :carets="carets" />
      </div>
      <div v-if="unknownPackages.length" class="math-note warn"><AlertTriangle :size="12" /> {{ $t('editor.mathPackageUnknown', { name: unknownPackages.join(', ') }) }}</div>
      <div v-else-if="!preambleOnly && defines.length" class="math-note">{{ $t('editor.mathDefines', { names: defines.join(', ') }) }}</div>
      <div class="math-hint">LaTeX · <UiKbd :keys="['mod', 'enter']" /> {{ $t('editor.toValidate') }} · {{ $t('editor.mathScopeHint') }}</div>
    </div>
  </NodeViewWrapper>
</template>

<style scoped>
.math-block { position: relative; margin: 0.8em 0; border-radius: var(--r-md); transition: background var(--dur-fast), box-shadow var(--dur-fast); }
.math-block.selected, .math-block.editing { box-shadow: 0 0 0 1.5px var(--border-focus); }
.math-block.has-who:not(.editing) { box-shadow: 0 0 0 1.5px var(--who); }
.math-render { padding: 6px 12px; cursor: pointer; overflow-x: auto; min-height: 40px; display: flex; align-items: center; justify-content: center; border-radius: var(--r-md); }
.math-render:hover { background: var(--bg-hover); }
.math-placeholder { color: var(--text-3); font-size: var(--fs-md); }
.math-error { color: var(--red); font-size: var(--fs-sm); font-family: var(--font-mono); }
.math-preamble { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; padding: 6px 10px; border-radius: var(--r-md); color: var(--text-3); font-size: var(--fs-sm); cursor: pointer; background: var(--bg-code); }
.math-preamble:hover { color: var(--text-2); }
.math-preamble code { font-family: var(--font-mono); font-size: 12px; color: var(--text-2); padding: 0 4px; border-radius: 3px; background: var(--bg-hover); }
.mp-label { font-weight: 500; }
.mp-pkg { display: inline-flex; align-items: center; gap: 3px; font-size: 12px; }
.mp-pkg.unknown { color: var(--orange, #e2a03f); }
.math-field { position: relative; }
.math-edit { border-top: 1px solid var(--border); padding: 8px 12px; }
.math-edit textarea {
  width: 100%; border: none; outline: none; resize: none; background: transparent; overflow: hidden;
  font-family: var(--font-mono); font-size: 13px; line-height: 1.5; color: var(--text);
}
.math-note { display: flex; align-items: center; gap: 6px; font-size: var(--fs-xs); color: var(--text-3); margin-bottom: 4px; }
.math-note.warn { color: var(--orange, #e2a03f); }
.math-hint { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; font-size: var(--fs-xs); color: var(--text-3); }
</style>
