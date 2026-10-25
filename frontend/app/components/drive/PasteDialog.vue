<script setup lang="ts">
import { useI18n } from '~/composables/useI18n'

/**
 * Nouveau fichier texte dans le drive : contenu collé (ou saisi) et nom du fichier.
 * L'extension est proposée d'après le contenu (Markdown, JSON, CSV, HTML, texte).
 */
const open = defineModel<boolean>('open', { default: false })
const props = defineProps<{ initial?: string }>()
const emit = defineEmits<{ (e: 'save', file: File): void }>()
const { t } = useI18n()

const text = ref('')
const name = ref('')
const field = ref<HTMLTextAreaElement | null>(null)

/** Extension la plus probable pour un contenu collé. */
function guessExt(s: string) {
  const v = s.trim()
  if (/^[[{]/.test(v)) {
    try { JSON.parse(v); return 'json' } catch { /* pas du JSON */ }
  }
  if (/^<(!doctype|html|div|p|section|body)\b/i.test(v)) return 'html'
  const lines = v.split('\n').slice(0, 5)
  if (lines.length > 1 && lines.every((l) => l.split(/[;,\t]/).length > 2 && l.split(/[;,\t]/).length === lines[0]!.split(/[;,\t]/).length)) return 'csv'
  if (/^(#{1,6} |[-*] |\d+\. |> |```)/m.test(v)) return 'md'
  return 'txt'
}
const defaultName = () => {
  const d = new Date()
  const p = (n: number) => String(n).padStart(2, '0')
  return `${t('drive.pastedName')} ${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}h${p(d.getMinutes())}`
}

watch(open, (v) => {
  if (!v) return
  text.value = props.initial ?? ''
  name.value = `${defaultName()}.${guessExt(text.value)}`
  nextTick(() => field.value?.focus())
}, { immediate: true })

// Nom encore proposé automatiquement : l'extension suit le contenu.
watch(text, (v, old) => {
  const base = name.value.replace(/\.[a-z0-9]+$/i, '')
  if (name.value === `${base}.${guessExt(old ?? '')}`) name.value = `${base}.${guessExt(v)}`
})

const canSave = computed(() => !!text.value && !!name.value.trim())
function save() {
  if (!canSave.value) return
  const n = name.value.trim().includes('.') ? name.value.trim() : `${name.value.trim()}.txt`
  const type = n.endsWith('.json') ? 'application/json' : n.endsWith('.html') ? 'text/html' : n.endsWith('.csv') ? 'text/csv' : n.endsWith('.md') ? 'text/markdown' : 'text/plain'
  emit('save', new File([text.value], n, { type: `${type};charset=utf-8` }))
  open.value = false
}
</script>

<template>
  <UiDialog v-model:open="open" :title="t('drive.newText')" :description="t('drive.newTextHint')" :width="620">
    <div class="pd">
      <UiInput v-model="name" :aria-label="t('drive.fileName')" :placeholder="t('drive.fileName')" @keydown.enter.prevent="save" />
      <textarea ref="field" v-model="text" class="pd-text" spellcheck="false" :placeholder="t('drive.pastePlaceholder')" @keydown.enter="(e: KeyboardEvent) => { if (e.metaKey || e.ctrlKey) { e.preventDefault(); save() } }" />
      <div class="pd-meta faint">{{ t('drive.chars', { count: text.length }) }}</div>
    </div>
    <template #footer="{ close }">
      <UiButton variant="ghost" @click="close">{{ t('common.cancel') }}</UiButton>
      <UiButton variant="primary" :disabled="!canSave" @click="save">{{ t('drive.saveText') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.pd { display: flex; flex-direction: column; gap: 10px; }
.pd-text {
  min-height: 260px; max-height: 60vh; resize: vertical; padding: 10px 12px; border-radius: var(--r-md);
  border: 1px solid var(--border); background: var(--bg); color: var(--text);
  font: 13px/1.55 var(--font-mono, ui-monospace, monospace);
}
.pd-text:focus { outline: none; border-color: var(--accent); }
.pd-meta { font-size: var(--fs-xs); text-align: right; }
</style>
