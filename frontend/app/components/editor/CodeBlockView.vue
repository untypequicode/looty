<script setup lang="ts">
import { NodeViewWrapper, NodeViewContent, nodeViewProps } from '@tiptap/vue-3'
import { Copy, Check, ChevronDown } from 'lucide-vue-next'

const props = defineProps(nodeViewProps)
const copied = ref(false)
const open = ref(false)

const LANGS = ['plaintext', 'bash', 'c', 'cpp', 'csharp', 'css', 'diff', 'go', 'graphql', 'ini', 'java', 'javascript', 'json', 'kotlin', 'less', 'lua', 'makefile', 'markdown', 'objectivec', 'perl', 'php', 'python', 'r', 'ruby', 'rust', 'scss', 'shell', 'sql', 'swift', 'typescript', 'xml', 'yaml']
const items = LANGS.map((l) => ({ value: l, label: l }))
const lang = computed({
  get: () => props.node.attrs.language || 'plaintext',
  set: (v: string) => props.updateAttributes({ language: v === 'plaintext' ? null : v }),
})

async function copy() {
  await navigator.clipboard.writeText(props.node.textContent)
  copied.value = true
  setTimeout(() => (copied.value = false), 1400)
}
</script>

<template>
  <NodeViewWrapper class="code-node">
    <div class="code-bar" contenteditable="false">
      <UiCombobox v-if="editor.isEditable" v-model="lang" v-model:open="open" :items="items" :placeholder="$t('editor.language')" :width="200">
        <button class="code-lang">{{ lang }} <ChevronDown :size="12" /></button>
      </UiCombobox>
      <span v-else class="code-lang static">{{ lang }}</span>
      <button class="code-copy" :title="$t('common.copy')" @click="copy">
        <Check v-if="copied" :size="13" class="ok" /><Copy v-else :size="13" />
      </button>
    </div>
    <pre><NodeViewContent as="code" :class="`hljs language-${node.attrs.language || 'plaintext'}`" /></pre>
  </NodeViewWrapper>
</template>

<style scoped>
.code-node { position: relative; margin: 0.8em 0; }
.code-node pre { margin: 0 !important; padding-top: 34px !important; }
.code-bar { position: absolute; top: 6px; left: 8px; right: 8px; display: flex; justify-content: space-between; align-items: center; z-index: 1; }
.code-lang { display: inline-flex; align-items: center; gap: 4px; height: 22px; padding: 0 6px; border-radius: var(--r-xs); font-size: var(--fs-xs); color: var(--text-3); font-family: var(--font-mono); }
.code-lang:not(.static):hover { background: var(--bg-hover); color: var(--text); }
.code-copy { width: 24px; height: 24px; display: grid; place-items: center; border-radius: var(--r-xs); color: var(--text-3); opacity: 0; transition: opacity var(--dur-fast); }
.code-node:hover .code-copy { opacity: 1; }
.code-copy:hover { background: var(--bg-hover); color: var(--text); }
.ok { color: var(--green); }
</style>
