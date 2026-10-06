<script setup lang="ts">
import { BubbleMenu, type Editor } from '@tiptap/vue-3'
import {
  Bold, Italic, Underline, Strikethrough, Code, Link2, Highlighter, Heading1, Heading2, Pilcrow, Quote, ChevronDown, Check, X,
  FileText, ExternalLink, MessageSquarePlus, Rows3, Columns3, Trash2, ArrowUpToLine, ArrowDownToLine, ArrowLeftToLine, ArrowRightToLine, TableCellsMerge,
} from 'lucide-vue-next'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'

const props = defineProps<{ editor: Editor; canComment?: boolean }>()
const { pages, wsId } = useWorkspace()
const { t } = useI18n()

const linkMode = ref(false)
const linkValue = ref('')
const linkInput = ref<HTMLInputElement | null>(null)
const active = ref(0)
const typeOpen = ref(false)

function shouldShowText({ editor, state, from, to }: any) {
  if (!editor.isEditable || from === to) return false
  if (editor.isActive('codeBlock') || editor.isActive('image') || editor.isActive('mathBlock')) return false
  const sel = state.selection
  return !(sel.node) && state.doc.textBetween(from, to).length > 0 || linkMode.value
}
/** Lecture seule : seul le bouton « Commenter » est proposé sur une sélection. */
function shouldShowComment({ editor, state, from, to }: any) {
  return !editor.isEditable && !!props.canComment && from !== to && !state.selection.node && state.doc.textBetween(from, to).trim().length > 0
}
/** Commenter la sélection : la page ouvre la discussion avec le passage cité. */
function comment() {
  const { from, to } = props.editor.state.selection
  const quote = props.editor.state.doc.textBetween(from, to, ' ').trim()
  // Marque provisoire (éditeurs) ; remplacée par l'identifiant du fil une fois créé.
  if (props.editor.isEditable) props.editor.chain().setMark('comment', { id: 'pending' }).run()
  window.dispatchEvent(new CustomEvent('looty:comment-selection', { detail: { quote } }))
}
function shouldShowTable({ editor }: any) {
  return editor.isEditable && editor.isActive('table')
}

function openLink() {
  linkValue.value = props.editor.getAttributes('link').href ?? ''
  linkMode.value = true
  active.value = 0
  nextTick(() => linkInput.value?.focus())
}
const docResults = computed(() => {
  const q = linkValue.value.trim().toLowerCase()
  if (!q || /^(https?:|mailto:|\/|#)/i.test(q)) return []
  return (pages.data.value ?? []).filter((p) => (p.title || '').toLowerCase().includes(q)).slice(0, 6)
})
function applyLink(href?: string) {
  let url = (href ?? linkValue.value).trim()
  if (!url) {
    props.editor.chain().focus().extendMarkRange('link').unsetLink().run()
  } else {
    if (!/^(https?:|mailto:|\/|#)/i.test(url)) url = `https://${url}`
    props.editor.chain().focus().extendMarkRange('link').setLink({ href: url }).run()
  }
  linkMode.value = false
}
function onLinkKey(e: KeyboardEvent) {
  const n = docResults.value.length
  if (e.key === 'ArrowDown' && n) { e.preventDefault(); active.value = (active.value + 1) % n }
  else if (e.key === 'ArrowUp' && n) { e.preventDefault(); active.value = (active.value - 1 + n) % n }
  else if (e.key === 'Enter') { e.preventDefault(); applyLink(n ? links.doc(wsId.value, docResults.value[active.value].id) : undefined) }
  else if (e.key === 'Escape') { e.preventDefault(); linkMode.value = false; props.editor.commands.focus() }
}

// Ctrl+K ouvre l'éditeur de lien quand du texte est sélectionné
function onKey(e: KeyboardEvent) {
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k' && !props.editor.state.selection.empty && props.editor.isFocused) {
    e.preventDefault()
    e.stopPropagation()
    openLink()
  }
}
onMounted(() => props.editor.view.dom.addEventListener('keydown', onKey, true))
onBeforeUnmount(() => props.editor.view.dom?.removeEventListener('keydown', onKey, true))

const blockType = computed(() => {
  if (props.editor.isActive('heading', { level: 1 })) return { label: t('editor.slash.h1'), icon: Heading1 }
  if (props.editor.isActive('heading', { level: 2 })) return { label: t('editor.slash.h2'), icon: Heading2 }
  if (props.editor.isActive('blockquote')) return { label: t('editor.slash.quote'), icon: Quote }
  return { label: t('editor.slash.text'), icon: Pilcrow }
})
const typeItems = computed(() => [
  { label: t('editor.slash.text'), icon: Pilcrow, onSelect: () => props.editor.chain().focus().setParagraph().run() },
  { label: t('editor.slash.h1'), icon: Heading1, onSelect: () => props.editor.chain().focus().toggleHeading({ level: 1 }).run() },
  { label: t('editor.slash.h2'), icon: Heading2, onSelect: () => props.editor.chain().focus().toggleHeading({ level: 2 }).run() },
  { label: t('editor.slash.quote'), icon: Quote, onSelect: () => props.editor.chain().focus().toggleBlockquote().run() },
])
const isInternal = computed(() => (props.editor.getAttributes('link').href ?? '').startsWith('/'))
</script>

<template>
  <BubbleMenu :editor="editor" :should-show="shouldShowText" :tippy-options="{ duration: 120, maxWidth: 'none', zIndex: 1400, onHidden: () => (linkMode = false) }" plugin-key="textBubble">
    <div class="bubble">
      <template v-if="!linkMode">
        <UiMenu v-model:open="typeOpen" :items="typeItems" :width="190">
          <button class="b-type"><component :is="blockType.icon" :size="14" /> <ChevronDown :size="12" /></button>
        </UiMenu>
        <span class="sep" />
        <button :class="{ on: editor.isActive('bold') }" :title="`${t('editor.bold')} (Ctrl+B)`" @click="editor.chain().focus().toggleBold().run()"><Bold :size="15" /></button>
        <button :class="{ on: editor.isActive('italic') }" :title="`${t('editor.italic')} (Ctrl+I)`" @click="editor.chain().focus().toggleItalic().run()"><Italic :size="15" /></button>
        <button :class="{ on: editor.isActive('underline') }" :title="t('editor.underline')" @click="editor.chain().focus().toggleUnderline().run()"><Underline :size="15" /></button>
        <button :class="{ on: editor.isActive('strike') }" :title="t('editor.strike')" @click="editor.chain().focus().toggleStrike().run()"><Strikethrough :size="15" /></button>
        <button :class="{ on: editor.isActive('code') }" :title="t('editor.code')" @click="editor.chain().focus().toggleCode().run()"><Code :size="15" /></button>
        <button :class="{ on: editor.isActive('highlight') }" :title="t('editor.highlight')" @click="editor.chain().focus().toggleHighlight().run()"><Highlighter :size="15" /></button>
        <span class="sep" />
        <button :class="{ on: editor.isActive('link') }" :title="`${t('editor.link')} (Ctrl+K)`" @click="openLink"><Link2 :size="15" /></button>
        <template v-if="canComment">
          <span class="sep" />
          <button :title="t('itemComments.commentSelection')" @click="comment"><MessageSquarePlus :size="15" /></button>
        </template>
        <a v-if="editor.isActive('link')" class="b-open" :href="editor.getAttributes('link').href" :target="isInternal ? undefined : '_blank'" rel="noopener" :title="t('common.open')"><ExternalLink :size="14" /></a>
      </template>
      <div v-else class="link-edit">
        <div class="link-row">
          <Link2 :size="14" class="faint" />
          <input ref="linkInput" v-model="linkValue" :placeholder="t('editor.linkPlaceholder')" @keydown="onLinkKey" />
          <button :title="t('common.apply')" @click="applyLink()"><Check :size="14" /></button>
          <button v-if="editor.isActive('link')" :title="t('editor.removeLink')" @click="applyLink('')"><X :size="14" /></button>
        </div>
        <div v-if="docResults.length" class="link-results">
          <button v-for="(p, i) in docResults" :key="p.id" :class="{ active: i === active }" @mouseenter="active = i" @click="applyLink(links.doc(wsId, p.id))">
            <span v-if="p.icon">{{ p.icon }}</span><FileText v-else :size="14" class="faint" />
            <span class="truncate">{{ p.title || t('docs.untitled') }}</span>
          </button>
        </div>
      </div>
    </div>
  </BubbleMenu>

  <BubbleMenu :editor="editor" :should-show="shouldShowComment" :tippy-options="{ duration: 120, zIndex: 1400 }" plugin-key="commentBubble">
    <div class="bubble">
      <button class="b-comment" @click="comment"><MessageSquarePlus :size="15" /> {{ t('itemComments.commentSelection') }}</button>
    </div>
  </BubbleMenu>

  <BubbleMenu :editor="editor" :should-show="shouldShowTable" :tippy-options="{ duration: 120, placement: 'top-start', zIndex: 1300, maxWidth: 'none' }" plugin-key="tableBubble">
    <div class="bubble">
      <button :title="t('editor.table.rowAbove')" @click="editor.chain().focus().addRowBefore().run()"><ArrowUpToLine :size="15" /></button>
      <button :title="t('editor.table.rowBelow')" @click="editor.chain().focus().addRowAfter().run()"><ArrowDownToLine :size="15" /></button>
      <button :title="t('editor.table.deleteRow')" @click="editor.chain().focus().deleteRow().run()"><Rows3 :size="15" /></button>
      <span class="sep" />
      <button :title="t('editor.table.colLeft')" @click="editor.chain().focus().addColumnBefore().run()"><ArrowLeftToLine :size="15" /></button>
      <button :title="t('editor.table.colRight')" @click="editor.chain().focus().addColumnAfter().run()"><ArrowRightToLine :size="15" /></button>
      <button :title="t('editor.table.deleteCol')" @click="editor.chain().focus().deleteColumn().run()"><Columns3 :size="15" /></button>
      <span class="sep" />
      <button :title="t('editor.table.toggleHeader')" @click="editor.chain().focus().toggleHeaderRow().run()"><TableCellsMerge :size="15" /></button>
      <button class="danger" :title="t('editor.table.delete')" @click="editor.chain().focus().deleteTable().run()"><Trash2 :size="15" /></button>
    </div>
  </BubbleMenu>
</template>

<style scoped>
.b-comment { display: inline-flex; align-items: center; gap: 6px; width: auto !important; padding: 0 8px; }
.bubble {
  display: flex;
  align-items: center;
  gap: 1px;
  padding: 3px;
  background: var(--bg-elevated);
  border-radius: var(--r-md);
  box-shadow: var(--shadow-lg);
}
.bubble > button, .bubble .b-open, .b-type {
  height: 28px;
  min-width: 28px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 2px;
  border-radius: var(--r-sm);
  color: var(--text-2);
  transition: background var(--dur-fast), color var(--dur-fast);
}
.b-type { padding: 0 6px; }
.bubble > button:hover, .b-open:hover, .b-type:hover { background: var(--bg-hover); color: var(--text); }
.bubble > button.on { color: var(--accent-text); background: var(--accent-soft); }
.bubble > button.danger:hover { color: var(--red); background: var(--red-soft); }
.sep { width: 1px; height: 18px; background: var(--border-strong); margin: 0 3px; }
.link-edit { width: 320px; }
.link-row { display: flex; align-items: center; gap: 6px; padding: 0 4px 0 8px; height: 30px; }
.link-row input { flex: 1; border: none; outline: none; background: transparent; min-width: 0; }
.link-row button { width: 26px; height: 26px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-2); }
.link-row button:hover { background: var(--bg-hover); color: var(--text); }
.link-results { border-top: 1px solid var(--border); padding: 4px; display: flex; flex-direction: column; }
.link-results button { display: flex; align-items: center; gap: 8px; height: 30px; padding: 0 8px; border-radius: var(--r-sm); text-align: left; }
.link-results button.active { background: var(--bg-active); }
</style>
