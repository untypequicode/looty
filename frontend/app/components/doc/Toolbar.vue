<script setup lang="ts">
import type { Component } from 'vue'
import type { Editor } from '@tiptap/vue-3'
import { useResizeObserver, useEventListener } from '@vueuse/core'
import {
  Undo2, Redo2, ChevronDown, Bold, Italic, Underline, Strikethrough, Code, Highlighter, Palette, List, ListOrdered, ListChecks,
  IndentIncrease, IndentDecrease, AlignLeft, AlignCenter, AlignRight, AlignJustify, Link2, Image, Paperclip, Table, Sigma, Minus,
  Plus, MoreHorizontal, Pilcrow, Heading1, Heading2, Heading3, Quote, Code2, Check, X, ArrowUpToLine, ArrowDownToLine,
  ArrowLeftToLine, ArrowRightToLine, Rows3, Columns3, TableCellsMerge, TableCellsSplit, Trash2, Table2, EyeOff, ChevronRight,
} from 'lucide-vue-next'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { slashItems, type SlashContext, type SlashItem } from '~/editor/slash'
import { TEXT_COLORS } from '~/editor/textStyle'
import { clampRowHeight } from '~/editor/tableSizing'
import {
  TOOLBAR_GROUPS, BTN_WIDTH, EMPTY_STATE, splitGroups, insertItems, toolbarState, sameState, rovingNext, keyboardInset,
  type ToolbarGroupId, type ToolbarState, type BlockType,
} from '~/editor/toolbarModel'
import { useBreakpoint } from '~/composables/useBreakpoint'
import { useI18n } from '~/composables/useI18n'
import { useUserPrefs } from '~/composables/usePrefs'
import { links, useWorkspace } from '~/composables/useWorkspace'

/**
 * Barre d'outils fixe de l'éditeur de pages. L'état de la sélection est calculé une fois par image
 * (rAF) et ne remplace la valeur réactive que s'il a changé : une frappe ordinaire ne réaffiche rien.
 */
const props = defineProps<{ editor: Editor; slash: SlashContext }>()
const { t } = useI18n()
const { isMobile } = useBreakpoint()
const { update: updatePrefs } = useUserPrefs()
const { pages, wsId } = useWorkspace()

const root = ref<HTMLElement | null>(null)
const st = shallowRef<ToolbarState>({ ...EMPTY_STATE })
const editable = ref(props.editor.isEditable)

function read(): ToolbarState {
  const e = props.editor
  const can = () => e.can()
  return toolbarState({
    isActive: (n, a) => e.isActive(n, a),
    getAttributes: (n) => e.getAttributes(n),
    can: {
      undo: () => can().undo(),
      redo: () => can().redo(),
      indent: () => can().sinkListItem('listItem') || can().sinkListItem('taskItem'),
      outdent: () => can().liftListItem('listItem') || can().liftListItem('taskItem'),
      mergeCells: () => can().mergeCells(),
      splitCell: () => can().splitCell(),
    },
  })
}
let raf = 0
function refresh() {
  if (raf) return
  raf = requestAnimationFrame(() => {
    raf = 0
    const e = props.editor
    if (e.isDestroyed) return
    editable.value = e.isEditable
    if (!e.isEditable) return
    const next = read()
    if (!sameState(st.value, next)) st.value = next
  })
}
onMounted(() => { props.editor.on('transaction', refresh); refresh() })
onBeforeUnmount(() => { props.editor.off('transaction', refresh); if (raf) cancelAnimationFrame(raf) })

// Largeur disponible -> groupes visibles ou repliés dans « Plus ».
const width = ref(1000)
useResizeObserver(root, (entries) => { width.value = Math.round(entries[0]?.contentRect.width ?? 1000) })
const btn = computed(() => (isMobile.value ? 38 : BTN_WIDTH))
const split = computed(() => splitGroups(width.value - 12, TOOLBAR_GROUPS, btn.value))
const vis = computed(() => new Set(split.value.visible))
const hasOverflow = computed(() => split.value.overflow.length > 0)

// Mobile : barre fixée en bas, au-dessus du clavier virtuel (visualViewport).
const kb = ref(0)
function onViewport() { kb.value = keyboardInset(window.innerHeight, window.visualViewport) }
onMounted(onViewport)
useEventListener(() => (typeof window !== 'undefined' ? window.visualViewport : null), 'resize', onViewport)
useEventListener(() => (typeof window !== 'undefined' ? window.visualViewport : null), 'scroll', onViewport)

const slashList = computed(() => slashItems(props.slash))
const slashById = computed(() => Object.fromEntries(slashList.value.map((i) => [i.id, i])) as Record<string, SlashItem | undefined>)
const insertList = computed(() => insertItems(slashList.value))
function runSlash(id: string) {
  const pos = props.editor.state.selection.from
  slashById.value[id]?.run(props.editor, { from: pos, to: pos })
}
const ch = () => props.editor.chain().focus()

const BLOCKS: { id: BlockType; key: string; icon: Component; run: () => void }[] = [
  { id: 'paragraph', key: 'editor.slash.text', icon: Pilcrow, run: () => ch().setParagraph().run() },
  { id: 'h1', key: 'editor.slash.h1', icon: Heading1, run: () => ch().toggleHeading({ level: 1 }).run() },
  { id: 'h2', key: 'editor.slash.h2', icon: Heading2, run: () => ch().toggleHeading({ level: 2 }).run() },
  { id: 'h3', key: 'editor.slash.h3', icon: Heading3, run: () => ch().toggleHeading({ level: 3 }).run() },
  { id: 'quote', key: 'editor.slash.quote', icon: Quote, run: () => ch().toggleBlockquote().run() },
  { id: 'code', key: 'editor.slash.code', icon: Code2, run: () => ch().toggleCodeBlock().run() },
]
const currentBlock = computed(() => BLOCKS.find((b) => b.id === st.value.block) ?? BLOCKS[0]!)

interface Btn { id: string; label: string; icon: Component; shortcut?: string[]; on?: boolean; disabled?: boolean; run: () => void }
const S = computed(() => st.value)
const historyBtns = computed<Btn[]>(() => [
  { id: 'undo', label: t('toolbar.undo'), icon: Undo2, shortcut: ['mod', 'Z'], disabled: !S.value.canUndo, run: () => ch().undo().run() },
  { id: 'redo', label: t('toolbar.redo'), icon: Redo2, shortcut: ['mod', 'shift', 'Z'], disabled: !S.value.canRedo, run: () => ch().redo().run() },
])
const markBtns = computed<Btn[]>(() => [
  { id: 'bold', label: t('editor.bold'), icon: Bold, shortcut: ['mod', 'B'], on: S.value.bold, run: () => ch().toggleBold().run() },
  { id: 'italic', label: t('editor.italic'), icon: Italic, shortcut: ['mod', 'I'], on: S.value.italic, run: () => ch().toggleItalic().run() },
  { id: 'underline', label: t('editor.underline'), icon: Underline, shortcut: ['mod', 'U'], on: S.value.underline, run: () => ch().toggleUnderline().run() },
  { id: 'strike', label: t('editor.strike'), icon: Strikethrough, shortcut: ['mod', 'shift', 'S'], on: S.value.strike, run: () => ch().toggleStrike().run() },
  { id: 'code', label: t('editor.code'), icon: Code, shortcut: ['mod', 'E'], on: S.value.code, run: () => ch().toggleCode().run() },
])
const highlightBtn = computed<Btn>(() => ({ id: 'highlight', label: t('editor.highlight'), icon: Highlighter, shortcut: ['mod', 'shift', 'H'], on: S.value.highlight, run: () => ch().toggleHighlight().run() }))
const listBtns = computed<Btn[]>(() => [
  { id: 'bullet', label: t('editor.slash.bullet'), icon: List, shortcut: ['mod', 'shift', '8'], on: S.value.bulletList, run: () => ch().toggleBulletList().run() },
  { id: 'ordered', label: t('editor.slash.ordered'), icon: ListOrdered, shortcut: ['mod', 'shift', '7'], on: S.value.orderedList, run: () => ch().toggleOrderedList().run() },
  { id: 'todo', label: t('editor.slash.todo'), icon: ListChecks, shortcut: ['mod', 'shift', '9'], on: S.value.taskList, run: () => ch().toggleTaskList().run() },
  { id: 'outdent', label: t('toolbar.outdent'), icon: IndentDecrease, shortcut: ['shift', 'Tab'], disabled: !S.value.canOutdent, run: () => { if (!ch().liftListItem('listItem').run()) ch().liftListItem('taskItem').run() } },
  { id: 'indent', label: t('toolbar.indent'), icon: IndentIncrease, shortcut: ['Tab'], disabled: !S.value.canIndent, run: () => { if (!ch().sinkListItem('listItem').run()) ch().sinkListItem('taskItem').run() } },
])
const insertBtns = computed<Btn[]>(() => [
  { id: 'link', label: t('toolbar.link'), icon: Link2, shortcut: ['mod', 'K'], on: S.value.link, run: () => openLink() },
  { id: 'image', label: t('toolbar.image'), icon: Image, run: () => runSlash('image') },
  { id: 'file', label: t('toolbar.file'), icon: Paperclip, run: () => runSlash('file') },
  { id: 'formula', label: t('toolbar.formula'), icon: Sigma, run: () => runSlash('math') },
  { id: 'divider', label: t('toolbar.divider'), icon: Minus, run: () => runSlash('hr') },
])
const ALIGNS = [
  { id: 'left', key: 'editor.alignLeft', icon: AlignLeft },
  { id: 'center', key: 'editor.alignCenter', icon: AlignCenter },
  { id: 'right', key: 'toolbar.alignRight', icon: AlignRight },
  { id: 'justify', key: 'toolbar.alignJustify', icon: AlignJustify },
] as const
const currentAlign = computed(() => ALIGNS.find((a) => a.id === S.value.align) ?? ALIGNS[0])

// --- Menus déroulants -------------------------------------------------------------------------
const blockOpen = ref(false)
const alignOpen = ref(false)
const tableOpen = ref(false)
const insertOpen = ref(false)
const moreOpen = ref(false)
const colorOpen = ref(false)

const blockItems = computed<MenuItem[]>(() => BLOCKS.map((b) => ({ label: t(b.key), icon: b.icon, checked: S.value.block === b.id, onSelect: b.run })))
const alignItems = computed<MenuItem[]>(() => ALIGNS.map((a) => ({ label: t(a.key), icon: a.icon, checked: S.value.align === a.id, onSelect: () => ch().setTextAlign(a.id).run() })))
const ROW_HEIGHTS = [{ key: 'toolbar.rowAuto', h: null }, { key: 'toolbar.rowMedium', h: 56 }, { key: 'toolbar.rowTall', h: 96 }] as const
function setRowHeight(h: number | null) { ch().updateAttributes('tableRow', { rowheight: clampRowHeight(h) }).run() }
const tableItems = computed<MenuItem[]>(() => [
  { label: t('editor.table.rowAbove'), icon: ArrowUpToLine, onSelect: () => ch().addRowBefore().run() },
  { label: t('editor.table.rowBelow'), icon: ArrowDownToLine, onSelect: () => ch().addRowAfter().run() },
  { label: t('editor.table.deleteRow'), icon: Rows3, onSelect: () => ch().deleteRow().run() },
  { divider: true },
  { label: t('editor.table.colLeft'), icon: ArrowLeftToLine, onSelect: () => ch().addColumnBefore().run() },
  { label: t('editor.table.colRight'), icon: ArrowRightToLine, onSelect: () => ch().addColumnAfter().run() },
  { label: t('editor.table.deleteCol'), icon: Columns3, onSelect: () => ch().deleteColumn().run() },
  { divider: true },
  { label: t('toolbar.mergeCells'), icon: TableCellsMerge, disabled: !S.value.canMerge, onSelect: () => ch().mergeCells().run() },
  { label: t('toolbar.splitCell'), icon: TableCellsSplit, disabled: !S.value.canSplit, onSelect: () => ch().splitCell().run() },
  { label: t('editor.table.toggleHeader'), icon: Table2, onSelect: () => ch().toggleHeaderRow().run() },
  { divider: true },
  { heading: t('toolbar.rowHeight') },
  ...ROW_HEIGHTS.map((r) => ({ label: t(r.key), onSelect: () => setRowHeight(r.h) })),
  { divider: true },
  { label: t('editor.table.delete'), icon: Trash2, danger: true, onSelect: () => ch().deleteTable().run() },
])
const insertMenu = computed<MenuItem[]>(() => {
  const out: MenuItem[] = []
  let group = ''
  for (const i of insertList.value) {
    if (i.group !== group) { group = i.group; out.push({ heading: group }) }
    out.push({ label: i.label, icon: i.icon, shortcut: i.shortcut, onSelect: () => runSlash(i.id) })
  }
  return out
})

// --- Couleur du texte -------------------------------------------------------------------------
function setColor(c: string | null) {
  colorOpen.value = false
  if (c) ch().setTextColor(c).run()
  else ch().unsetTextColor().run()
}

// --- Lien -------------------------------------------------------------------------------------
const linkOpen = ref(false)
const linkValue = ref('')
const linkPos = ref<{ x: number; y: number } | null>(null)
const linkInput = ref<HTMLInputElement | null>(null)
function openLink() {
  const r = (root.value?.getBoundingClientRect() ?? { left: 0, bottom: 0 })
  linkPos.value = { x: r.left + 8, y: r.bottom + 6 }
  linkValue.value = props.editor.getAttributes('link').href ?? ''
  linkOpen.value = true
  nextTick(() => linkInput.value?.focus())
}
function applyLink(remove = false) {
  let url = remove ? '' : linkValue.value.trim()
  linkOpen.value = false
  const e = props.editor
  if (!url) return void e.chain().focus().extendMarkRange('link').unsetLink().run()
  if (!/^(https?:|mailto:|\/|#)/i.test(url)) url = `https://${url}`
  if (e.state.selection.empty && !e.isActive('link')) {
    e.chain().focus().insertContent({ type: 'text', text: url, marks: [{ type: 'link', attrs: { href: url } }] }).run()
  } else {
    e.chain().focus().extendMarkRange('link').setLink({ href: url }).run()
  }
}
const docMatches = computed(() => {
  const q = linkValue.value.trim().toLowerCase()
  if (!q || /^(https?:|mailto:|\/|#)/i.test(q)) return []
  return (pages.data.value ?? []).filter((p) => (p.title || '').toLowerCase().includes(q)).slice(0, 5)
})
function pickDoc(id: string) { linkValue.value = links.doc(wsId.value, id); applyLink() }

// --- Menu « Plus » : groupes repliés ----------------------------------------------------------
function asItem(b: Btn): MenuItem { return { label: b.label, icon: b.icon, shortcut: b.shortcut, checked: b.on, disabled: b.disabled, onSelect: b.run } }
const overflowItems = computed<MenuItem[]>(() => {
  const out: MenuItem[] = []
  for (const id of split.value.overflow) {
    if (out.length) out.push({ divider: true })
    const add = (items: MenuItem[]) => out.push(...items)
    switch (id as ToolbarGroupId) {
      case 'history': add(historyBtns.value.map(asItem)); break
      case 'block': add(blockItems.value); break
      case 'marks': add(markBtns.value.map(asItem)); break
      case 'color': add([...TEXT_COLORS.map((c) => ({ label: t(`toolbar.colors.${c}`), icon: Palette, checked: S.value.color === c, onSelect: () => setColor(c) })), { label: t('toolbar.noColor'), icon: X, onSelect: () => setColor(null) }, asItem(highlightBtn.value)]); break
      case 'lists': add(listBtns.value.map(asItem)); break
      case 'align': add(alignItems.value); break
      case 'insert': add(insertBtns.value.map(asItem)); break
      case 'table': add([{ label: t('toolbar.table'), icon: Table, onSelect: () => runSlash('table') }, ...(S.value.inTable ? [{ divider: true } as MenuItem, ...tableItems.value] : [])]); break
      default: break
    }
  }
  if (out.length) out.push({ divider: true })
  out.push({ label: t('toolbar.hide'), icon: EyeOff, onSelect: hide })
  return out
})
function hide() { updatePrefs({ editorToolbar: false }).catch(() => {}) }

// --- Navigation au clavier (barre d'outils à tabulation unique) ---------------------------------
const rover = ref(0)
function buttons(): HTMLElement[] { return [...(root.value?.querySelectorAll<HTMLElement>('.tb-btn:not([disabled])') ?? [])] }
function syncTab() { buttons().forEach((b, i) => b.setAttribute('tabindex', i === rover.value ? '0' : '-1')) }
onMounted(syncTab)
onUpdated(syncTab)
function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') { props.editor.commands.focus(); return }
  if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(e.key)) return
  const list = buttons()
  const cur = list.indexOf(document.activeElement as HTMLElement)
  if (cur < 0) return
  e.preventDefault()
  rover.value = rovingNext(cur, list.length, e.key)
  syncTab()
  list[rover.value]?.focus()
}
function onFocusIn(e: FocusEvent) {
  const i = buttons().indexOf(e.target as HTMLElement)
  if (i >= 0 && i !== rover.value) { rover.value = i; syncTab() }
}
</script>

<template>
  <div
    v-if="editable"
    ref="root"
    class="tb"
    :class="{ mobile: isMobile }"
    :style="isMobile ? { bottom: `${kb}px` } : undefined"
    role="toolbar"
    :aria-label="t('toolbar.label')"
    aria-orientation="horizontal"
    data-doc-toolbar
    @mousedown.prevent
    @keydown="onKey"
    @focusin="onFocusIn"
  >
    <template v-if="vis.has('history')">
      <UiTooltip v-for="b in historyBtns" :key="b.id" :text="b.label" :shortcut="b.shortcut">
        <button class="tb-btn" :data-tb="b.id" :aria-label="b.label" :disabled="b.disabled" @click="b.run()"><component :is="b.icon" :size="15" /></button>
      </UiTooltip>
      <span class="sep" role="separator" />
    </template>

    <template v-if="vis.has('block')">
      <UiMenu v-model:open="blockOpen" :items="blockItems" :width="200">
        <UiTooltip :text="t('toolbar.blockStyle')">
          <button class="tb-btn tb-wide" data-tb="block" :aria-label="t('toolbar.blockStyle')" aria-haspopup="menu" :aria-expanded="blockOpen">
            <component :is="currentBlock.icon" :size="15" /><span class="tb-label">{{ t(currentBlock.key) }}</span><ChevronDown :size="12" />
          </button>
        </UiTooltip>
      </UiMenu>
      <span class="sep" role="separator" />
    </template>

    <template v-if="vis.has('marks')">
      <UiTooltip v-for="b in markBtns" :key="b.id" :text="b.label" :shortcut="b.shortcut">
        <button class="tb-btn" :class="{ on: b.on }" :data-tb="b.id" :aria-label="b.label" :aria-pressed="!!b.on" @click="b.run()"><component :is="b.icon" :size="15" /></button>
      </UiTooltip>
      <span class="sep" role="separator" />
    </template>

    <template v-if="vis.has('color')">
      <UiPopover v-model:open="colorOpen" :width="188">
        <template #trigger>
          <UiTooltip :text="t('toolbar.textColor')">
            <button class="tb-btn" data-tb="color" :aria-label="t('toolbar.textColor')" aria-haspopup="dialog" :aria-expanded="colorOpen">
              <Palette :size="15" :style="S.color ? { color: `var(--${S.color === 'gray' ? 'text-3' : S.color})` } : undefined" /><ChevronDown :size="11" />
            </button>
          </UiTooltip>
        </template>
        <div class="palette">
          <button v-for="c in TEXT_COLORS" :key="c" class="swatch" :class="{ on: S.color === c }" :style="{ background: `var(--${c === 'gray' ? 'text-3' : c})` }" :title="t(`toolbar.colors.${c}`)" :aria-label="t(`toolbar.colors.${c}`)" @click="setColor(c)"><Check v-if="S.color === c" :size="12" /></button>
          <button class="swatch none" :title="t('toolbar.noColor')" :aria-label="t('toolbar.noColor')" @click="setColor(null)"><X :size="12" /></button>
        </div>
      </UiPopover>
      <UiTooltip :text="highlightBtn.label" :shortcut="highlightBtn.shortcut">
        <button class="tb-btn" :class="{ on: highlightBtn.on }" data-tb="highlight" :aria-label="highlightBtn.label" :aria-pressed="!!highlightBtn.on" @click="highlightBtn.run()"><Highlighter :size="15" /></button>
      </UiTooltip>
      <span class="sep" role="separator" />
    </template>

    <template v-if="vis.has('lists')">
      <UiTooltip v-for="b in listBtns" :key="b.id" :text="b.label" :shortcut="b.shortcut">
        <button class="tb-btn" :class="{ on: b.on }" :data-tb="b.id" :aria-label="b.label" :aria-pressed="b.on === undefined ? undefined : b.on" :disabled="b.disabled" @click="b.run()"><component :is="b.icon" :size="15" /></button>
      </UiTooltip>
      <span class="sep" role="separator" />
    </template>

    <template v-if="vis.has('align')">
      <UiMenu v-model:open="alignOpen" :items="alignItems" :width="190">
        <UiTooltip :text="t('toolbar.align')">
          <button class="tb-btn" data-tb="align" :aria-label="t('toolbar.align')" aria-haspopup="menu" :aria-expanded="alignOpen"><component :is="currentAlign.icon" :size="15" /><ChevronDown :size="11" /></button>
        </UiTooltip>
      </UiMenu>
      <span class="sep" role="separator" />
    </template>

    <template v-if="vis.has('insert')">
      <UiTooltip v-for="b in insertBtns" :key="b.id" :text="b.label" :shortcut="b.shortcut">
        <button class="tb-btn" :class="{ on: b.on }" :data-tb="b.id" :aria-label="b.label" @click="b.run()"><component :is="b.icon" :size="15" /></button>
      </UiTooltip>
      <span class="sep" role="separator" />
    </template>

    <template v-if="vis.has('table')">
      <UiTooltip :text="t('toolbar.table')">
        <button class="tb-btn" data-tb="table" :aria-label="t('toolbar.table')" @click="runSlash('table')"><Table :size="15" /></button>
      </UiTooltip>
      <UiMenu v-if="S.inTable" v-model:open="tableOpen" :items="tableItems" :width="220">
        <UiTooltip :text="t('toolbar.tableActions')">
          <button class="tb-btn on" data-tb="table-actions" :aria-label="t('toolbar.tableActions')" aria-haspopup="menu" :aria-expanded="tableOpen"><Table2 :size="15" /><ChevronDown :size="11" /></button>
        </UiTooltip>
      </UiMenu>
      <span class="sep" role="separator" />
    </template>

    <UiMenu v-model:open="insertOpen" :items="insertMenu" :width="240">
      <UiTooltip :text="t('toolbar.insert')">
        <button class="tb-btn tb-wide" data-tb="insert" :aria-label="t('toolbar.insert')" aria-haspopup="menu" :aria-expanded="insertOpen"><Plus :size="15" /><span class="tb-label">{{ t('toolbar.insert') }}</span><ChevronDown :size="12" /></button>
      </UiTooltip>
    </UiMenu>

    <span class="spacer" />
    <UiMenu v-if="hasOverflow" v-model:open="moreOpen" :items="overflowItems" :width="240" placement="bottom-end">
      <UiTooltip :text="t('toolbar.more')">
        <button class="tb-btn" data-tb="more" :aria-label="t('toolbar.more')" aria-haspopup="menu" :aria-expanded="moreOpen"><MoreHorizontal :size="16" /></button>
      </UiTooltip>
    </UiMenu>
    <UiTooltip v-else :text="t('toolbar.hide')">
      <button class="tb-btn tb-hide" data-tb="hide" :aria-label="t('toolbar.hide')" @click="hide"><EyeOff :size="14" /></button>
    </UiTooltip>

    <UiPopover v-model:open="linkOpen" :virtual-rect="linkPos" :width="320">
      <form class="link-pop" @submit.prevent="applyLink()" @mousedown.stop>
        <div class="link-row">
          <Link2 :size="14" class="faint" />
          <input ref="linkInput" v-model="linkValue" :placeholder="t('editor.linkPlaceholder')" :aria-label="t('toolbar.link')" />
          <button type="submit" :title="t('toolbar.linkApply')" :aria-label="t('toolbar.linkApply')"><Check :size="14" /></button>
          <button v-if="S.link" type="button" :title="t('toolbar.linkRemove')" :aria-label="t('toolbar.linkRemove')" @click="applyLink(true)"><X :size="14" /></button>
        </div>
        <div v-if="docMatches.length" class="link-results">
          <button v-for="p in docMatches" :key="p.id" type="button" @click="pickDoc(p.id)"><ChevronRight :size="12" class="faint" /><span class="truncate">{{ p.title || t('docs.untitled') }}</span></button>
        </div>
      </form>
    </UiPopover>
  </div>
</template>

<style scoped>
.tb {
  position: sticky;
  top: 8px;
  z-index: 20;
  display: flex;
  align-items: center;
  gap: 1px;
  margin: 0 0 12px;
  padding: 3px 4px;
  min-width: 0;
  overflow: hidden;
  background: var(--bg-elevated);
  border-radius: var(--r-md);
  box-shadow: var(--shadow-md);
}
.tb :deep(.popover-ref), .tb :deep(.tt-ref) { flex: none; }
.tb-btn {
  height: 28px;
  min-width: 28px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 3px;
  padding: 0 4px;
  border-radius: var(--r-sm);
  color: var(--text-2);
  transition: background var(--dur-fast), color var(--dur-fast);
}
.tb-btn:hover:not(:disabled) { background: var(--bg-hover); color: var(--text); }
.tb-btn:focus-visible { outline: 2px solid var(--border-focus); outline-offset: -2px; }
.tb-btn.on, .tb-btn[aria-expanded='true'] { color: var(--accent-text); background: var(--accent-soft); }
.tb-btn:disabled { opacity: 0.4; cursor: default; }
.tb-wide { padding: 0 7px; }
.tb-label { font-size: var(--fs-md); white-space: nowrap; }
.tb-hide { opacity: 0.6; }
.sep { width: 1px; height: 18px; flex: none; background: var(--border-strong); margin: 0 4px; }
.spacer { flex: 1; }
.palette { display: grid; grid-template-columns: repeat(4, 1fr); gap: 8px; padding: 10px; }
.swatch { width: 32px; height: 32px; border-radius: var(--r-full); display: grid; place-items: center; color: var(--bg); box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.15); }
.swatch.none { background: transparent; color: var(--text-2); box-shadow: inset 0 0 0 1px var(--border-strong); }
.swatch:hover { transform: scale(1.08); }
.link-row { display: flex; align-items: center; gap: 6px; padding: 0 4px 0 10px; height: 34px; }
.link-row input { flex: 1; min-width: 0; border: none; outline: none; background: transparent; }
.link-row button { width: 26px; height: 26px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-2); }
.link-row button:hover { background: var(--bg-hover); color: var(--text); }
.link-results { border-top: 1px solid var(--border); padding: 4px; display: flex; flex-direction: column; }
.link-results button { display: flex; align-items: center; gap: 8px; height: 30px; padding: 0 8px; border-radius: var(--r-sm); text-align: left; }
.link-results button:hover { background: var(--bg-hover); }

/* Mobile : fixée en bas au-dessus du clavier, boutons de 38px, sans gêner le texte (marge basse de l'éditeur). */
.tb.mobile {
  position: fixed;
  left: 8px;
  right: 8px;
  top: auto;
  margin: 0;
  z-index: 40;
  padding: 4px;
  margin-bottom: max(8px, var(--safe-bottom, 0px));
}
.tb.mobile .tb-btn { height: 38px; min-width: 38px; }
.tb.mobile .tb-label { display: none; }
@media (prefers-reduced-motion: reduce) { .tb-btn, .swatch { transition: none; } }
</style>
