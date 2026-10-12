<script setup lang="ts">
import { NodeViewWrapper, nodeViewProps } from '@tiptap/vue-3'
import katex from 'katex'
import 'katex/dist/katex.min.css'
import { renderMath } from '~/editor/latex'
import { useNodeScope } from '~/editor/mathScope'
import { useLatexField } from '~/editor/latexInput'
import { useSourceField } from '~/editor/source'

const props = defineProps(nodeViewProps)
const editing = ref(false)
const input = ref<HTMLInputElement | null>(null)
// Chaque frappe modifie directement la source (texte du nœud) : saisie collaborative.
const field = useSourceField(input, props as any)
const draft = field.draft
const carets = field.carets
const present = field.present
watch(() => props.selected, (s) => { if (s && props.editor.isEditable) start() })

// Macros du document (\newcommand… déclarées dans les formules précédentes).
const scope = useNodeScope(props as any)
const result = computed(() => renderMath(katex, draft.value ?? '', scope.value.scope, false))
const html = computed(() => result.value.html)
/** Formule ne contenant que des déclarations : un discret « ƒ » signale le préambule. */
const preambleOnly = computed(() => !!draft.value?.trim() && !result.value.scope.parsed.rest && !result.value.error)
const definesTitle = computed(() => [...result.value.scope.defines, ...result.value.scope.packages.map((p) => p.name)].join(', '))

function start() {
  if (!props.editor.isEditable) return
  editing.value = true
  nextTick(() => input.value?.focus())
}
function commit() {
  if (!editing.value) return
  editing.value = false
  // Formule vidée : elle disparaît (sauf si quelqu'un d'autre est en train de la remplir).
  if (!field.source.value.trim()) props.deleteNode()
}
const latex = useLatexField(input, () => Object.keys(result.value.scope.macros ?? {}))

/** Quitte la formule au clavier : curseur juste avant ou juste après, dans le texte. */
function leave(side: 'before' | 'after') {
  const at = props.getPos() as number
  // toRaw : sous le proxy réactif de Vue, ProseMirror ne reconnaît plus une feuille (taille 2 au lieu de 1).
  const pos = side === 'after' ? at + toRaw(props.node).nodeSize : at
  latex.close()
  // D'abord le curseur et le focus dans l'éditeur (le navigateur y place sa sélection),
  // ensuite seulement le champ disparaît : sinon la sélection du DOM, relue par
  // ProseMirror, se retrouve un caractère plus loin.
  const max = props.editor.state.doc.content.size
  props.editor.chain().setTextSelection(Math.max(0, Math.min(pos, max))).focus().run()
  commit()
}
function onKey(e: KeyboardEvent) {
  if (field.onHistoryKey(e)) return
  if (latex.onKeydown(e)) return
  const f = input.value
  if (e.key === 'Enter' || e.key === 'Escape' || e.key === 'Tab') {
    e.preventDefault()
    leave(e.key === 'Tab' && e.shiftKey ? 'before' : 'after')
  } else if (f && e.key === 'ArrowRight' && f.selectionStart === f.value.length && f.selectionEnd === f.value.length) {
    e.preventDefault()
    leave('after')
  } else if (f && e.key === 'ArrowLeft' && f.selectionStart === 0 && f.selectionEnd === 0) {
    e.preventDefault()
    leave('before')
  }
}
</script>

<template>
  <NodeViewWrapper as="span" class="math-inline" :class="{ selected, 'has-who': present.length }" :style="present.length ? { '--who': present[0]!.color } : undefined">
    <EditorSourceWho :people="carets.length ? [] : present" />
    <span v-if="editing" class="mi-edit" contenteditable="false">
      <span class="mi-dollar">$</span><input ref="input" v-model="draft" spellcheck="false" :size="Math.max(4, draft.length)" @keydown.stop="onKey" @beforeinput="field.onBeforeInput" @input="latex.refresh()" @click="latex.refresh()" @blur="latex.close(); commit()" /><span class="mi-dollar">$</span>
      <EditorLatexSuggest :items="latex.items.value" :active="latex.active.value" @pick="latex.pick" />
      <EditorSourceCursors :carets="carets" />
    </span>
    <span v-else-if="preambleOnly" class="mi-render mi-preamble" :title="definesTitle" @click="start">ƒ</span>
    <span v-else-if="html" class="mi-render" @click="start" v-html="html" />
    <span v-else class="mi-render mi-error" :title="result.error" @click="start">{{ draft || '$…$' }}</span>
  </NodeViewWrapper>
</template>

<style scoped>
.math-inline { position: relative; display: inline-block; border-radius: 4px; padding: 0 2px; }
.math-inline.selected { box-shadow: 0 0 0 1.5px var(--border-focus); }
.math-inline.has-who { box-shadow: 0 0 0 1.5px var(--who); }
.mi-render { cursor: pointer; }
.mi-render:hover { background: var(--bg-hover); border-radius: 4px; }
.mi-error { color: var(--red); font-family: var(--font-mono); font-size: 0.85em; }
.mi-preamble { color: var(--text-3); font-style: italic; font-family: var(--font-mono); font-size: 0.85em; padding: 0 3px; background: var(--bg-code); border-radius: 4px; }
.mi-edit { position: relative; display: inline-flex; align-items: center; background: var(--bg-code); border-radius: 4px; padding: 0 4px; font-family: var(--font-mono); font-size: 0.85em; }
.mi-edit input { border: none; outline: none; background: transparent; font: inherit; color: var(--text); }
.mi-dollar { color: var(--text-3); }
</style>
