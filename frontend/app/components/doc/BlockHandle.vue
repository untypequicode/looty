<script setup lang="ts">
import type { Editor } from '@tiptap/core'
import { NodeSelection } from '@tiptap/pm/state'
import { Plus, GripVertical, Copy, Trash2, ArrowUp, ArrowDown, Link2, Pilcrow, Heading1, Heading2, Heading3, List, ListOrdered, ListChecks, Quote, Code2, Info } from 'lucide-vue-next'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { blockAt, blockLabel, deleteBlock, duplicateBlock, moveBlock, turnInto, type BlockRef } from '~/editor/blocks'
import { collectHeadings } from '~/editor/headings'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'

/**
 * Poignée de bloc à gauche du contenu (façon Outline / Notion) :
 * affiche le type du bloc survolé, permet de le glisser-déposer,
 * d'insérer un bloc dessous (+) et ouvre le menu du bloc au clic.
 */
const props = defineProps<{ editor: Editor; container: HTMLElement | null }>()
const { t } = useI18n()

const block = ref<BlockRef | null>(null)
const top = ref(0)
const left = ref(0)
const visible = ref(false)
const menuOpen = ref(false)
const menuPos = ref<{ x: number; y: number } | null>(null)
let hideTimer: ReturnType<typeof setTimeout> | null = null

const label = computed(() => (block.value ? blockLabel(block.value.node) : ''))

function locate(e: MouseEvent) {
  if (menuOpen.value || !props.editor.isEditable) return
  const view = props.editor.view
  const rect = view.dom.getBoundingClientRect()
  // On sonde à l'intérieur de la colonne de texte, même si la souris est dans la marge gauche.
  const x = Math.min(Math.max(e.clientX, rect.left + 24), rect.right - 8)
  const found = view.posAtCoords({ left: x, top: e.clientY })
  if (!found) return
  const pos = found.inside >= 0 ? found.inside : found.pos
  const b = blockAt(props.editor, pos)
  if (!b) return
  const dom = view.nodeDOM(b.pos) as HTMLElement | null
  if (!dom || !(dom instanceof HTMLElement) || !props.container) return
  const cRect = props.container.getBoundingClientRect()
  const bRect = dom.getBoundingClientRect()
  // Aligne la poignée sur la première ligne du bloc
  const style = getComputedStyle(dom)
  const lineH = parseFloat(style.lineHeight) || 24
  const padTop = parseFloat(style.paddingTop) || 0
  block.value = b
  top.value = bRect.top - cRect.top + padTop + Math.max(0, (Math.min(lineH, bRect.height) - 24) / 2) + props.container.scrollTop
  left.value = bRect.left - cRect.left - 6
  visible.value = true
  if (hideTimer) clearTimeout(hideTimer)
}

function cancelHide() {
  if (hideTimer) clearTimeout(hideTimer)
  hideTimer = null
}

function scheduleHide() {
  if (hideTimer) clearTimeout(hideTimer)
  hideTimer = setTimeout(() => { if (!menuOpen.value) visible.value = false }, 300)
}

onMounted(() => {
  props.container?.addEventListener('mousemove', locate)
  props.container?.addEventListener('mouseleave', scheduleHide)
  props.editor.on('update', () => { if (!menuOpen.value) visible.value = false })
})
onBeforeUnmount(() => {
  props.container?.removeEventListener('mousemove', locate)
  props.container?.removeEventListener('mouseleave', scheduleHide)
})

// --- Glisser-déposer natif ProseMirror ---
function onDragStart(e: DragEvent) {
  const b = block.value
  if (!b || !e.dataTransfer) return
  const view = props.editor.view
  const sel = NodeSelection.create(view.state.doc, b.pos)
  view.dispatch(view.state.tr.setSelection(sel))
  const slice = sel.content()
  const dom = view.nodeDOM(b.pos) as HTMLElement
  e.dataTransfer.clearData()
  e.dataTransfer.setData('text/html', dom.outerHTML)
  e.dataTransfer.setData('text/plain', b.node.textContent)
  e.dataTransfer.effectAllowed = 'copyMove'
  e.dataTransfer.setDragImage(dom, 0, 0)
  view.dragging = { slice, move: true }
}
function onDragEnd() {
  visible.value = false
}

function insertBelow() {
  const b = block.value
  if (!b) return
  const end = b.pos + b.node.nodeSize
  const isEmptyPara = b.node.type.name === 'paragraph' && b.node.content.size === 0
  if (isEmptyPara) {
    props.editor.chain().focus().setTextSelection(b.pos + 1).insertContent('/').run()
  } else {
    props.editor.chain().focus().insertContentAt(end, { type: 'paragraph' }).setTextSelection(end + 1).insertContent('/').run()
  }
}

function openMenu(e: MouseEvent) {
  const b = block.value
  if (!b) return
  props.editor.view.dispatch(props.editor.state.tr.setSelection(NodeSelection.create(props.editor.state.doc, b.pos)))
  menuPos.value = { x: e.clientX + 8, y: e.clientY + 4 }
  menuOpen.value = true
}
watch(menuOpen, (v) => { if (!v) scheduleHide() })

const isText = computed(() => !!block.value && ['paragraph', 'heading', 'listItem', 'taskItem', 'bulletList', 'orderedList', 'taskList', 'blockquote', 'codeBlock', 'notice'].includes(block.value.node.type.name))
const menuItems = computed<MenuItem[]>(() => {
  const b = block.value
  if (!b) return []
  const items: MenuItem[] = []
  if (isText.value) {
    items.push(
      { heading: t('editor.turnInto') },
      { label: t('editor.slash.text'), icon: Pilcrow, onSelect: () => turnInto(props.editor, b, 'paragraph') },
      { label: t('editor.slash.h1'), icon: Heading1, onSelect: () => turnInto(props.editor, b, 'h1') },
      { label: t('editor.slash.h2'), icon: Heading2, onSelect: () => turnInto(props.editor, b, 'h2') },
      { label: t('editor.slash.h3'), icon: Heading3, onSelect: () => turnInto(props.editor, b, 'h3') },
      { label: t('editor.slash.bullet'), icon: List, onSelect: () => turnInto(props.editor, b, 'bullet') },
      { label: t('editor.slash.ordered'), icon: ListOrdered, onSelect: () => turnInto(props.editor, b, 'ordered') },
      { label: t('editor.slash.todo'), icon: ListChecks, onSelect: () => turnInto(props.editor, b, 'todo') },
      { label: t('editor.slash.quote'), icon: Quote, onSelect: () => turnInto(props.editor, b, 'quote') },
      { label: t('editor.slash.code'), icon: Code2, onSelect: () => turnInto(props.editor, b, 'code') },
      { label: t('editor.slash.calloutInfo'), icon: Info, onSelect: () => turnInto(props.editor, b, 'callout') },
      { divider: true },
    )
  }
  if (b.node.type.name === 'heading') {
    const h = collectHeadings(props.editor.state.doc).find((x) => x.pos === b.pos)
    if (h) items.push({ label: t('editor.copySectionLink'), icon: Link2, onSelect: () => {
      const url = `${location.origin}${location.pathname}#${h.id}`
      navigator.clipboard.writeText(url)
      history.replaceState(null, '', `#${h.id}`)
      toast.success(t('common.linkCopied'))
    } })
  }
  items.push(
    { label: t('common.duplicate'), icon: Copy, shortcut: ['mod', 'D'], onSelect: () => duplicateBlock(props.editor, b) },
    { label: t('editor.moveUp'), icon: ArrowUp, shortcut: ['alt', 'shift', '↑'], onSelect: () => moveBlock(props.editor, b, -1) },
    { label: t('editor.moveDown'), icon: ArrowDown, shortcut: ['alt', 'shift', '↓'], onSelect: () => moveBlock(props.editor, b, 1) },
    { divider: true },
    { label: t('common.delete'), icon: Trash2, danger: true, shortcut: 'del', onSelect: () => deleteBlock(props.editor, b) },
  )
  return items
})
</script>

<template>
  <div v-show="visible && editor.isEditable" class="bh" :style="{ top: `${top}px`, left: `${left}px` }" @mouseenter="cancelHide">
    <UiTooltip :text="$t('editor.addBlock')" placement="top">
      <button class="bh-btn" @mousedown.prevent @click="insertBelow"><Plus :size="15" /></button>
    </UiTooltip>
    <UiTooltip :text="$t('editor.dragHint')" placement="top">
      <button class="bh-grip" draggable="true" @dragstart="onDragStart" @dragend="onDragEnd" @mousedown.stop @click="openMenu">
        <span class="bh-label">{{ label }}</span>
        <GripVertical :size="14" />
      </button>
    </UiTooltip>
  </div>
  <UiMenu v-model:open="menuOpen" :items="menuItems" :virtual-rect="menuPos" :width="240" />
</template>

<style scoped>
.bh {
  position: absolute;
  z-index: 5;
  display: flex;
  align-items: center;
  gap: 1px;
  height: 24px;
  transform: translateX(-100%);
  transition: top 80ms var(--ease);
}
.bh-btn, .bh-grip {
  height: 24px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--r-xs);
  color: var(--text-3);
  transition: background var(--dur-fast), color var(--dur-fast);
}
.bh-btn { width: 22px; }
.bh-grip { gap: 1px; padding: 0 2px 0 4px; cursor: grab; min-width: 22px; }
.bh-grip:active { cursor: grabbing; }
.bh-btn:hover, .bh-grip:hover { background: var(--bg-hover); color: var(--text); }
.bh-label {
  font-size: 10px;
  font-weight: 700;
  font-family: var(--font-mono);
  letter-spacing: -0.02em;
  min-width: 14px;
  text-align: center;
  color: var(--text-3);
}
.bh-grip:hover .bh-label { color: var(--accent-text); }
</style>

<style>
@media (pointer: coarse) { .bh { display: none !important; } }
</style>
