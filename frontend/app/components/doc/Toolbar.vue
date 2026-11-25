<script setup lang="ts">
import type { Component } from 'vue'
import type { Editor } from '@tiptap/vue-3'
import { useResizeObserver, useEventListener } from '@vueuse/core'
import {
  Undo2, Redo2, ChevronDown, Bold, Italic, Underline, Strikethrough, Code, Highlighter, Palette, List, ListOrdered, ListChecks,
  IndentIncrease, IndentDecrease, AlignLeft, AlignCenter, AlignRight, AlignJustify, Link2, Image, Paperclip, Table, Sigma, Minus,
  Plus, MoreHorizontal, Pilcrow, Heading1, Heading2, Heading3, Quote, Code2, Check, X, ArrowUpToLine, ArrowDownToLine,
  ArrowLeftToLine, ArrowRightToLine, Rows3, Columns3, TableCellsMerge, TableCellsSplit, Trash2, Table2, EyeOff, ChevronRight,
  SlidersHorizontal, PanelTop, PanelBottom, LayoutTemplate, ChevronUp,
} from 'lucide-vue-next'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { slashItems, type SlashContext, type SlashItem } from '~/editor/slash'
import { clampRowHeight } from '~/editor/tableSizing'
import {
  EMPTY_STATE, GROUP_LABEL, SEP_WIDTH, SEP_WIDTH_TOUCH, TAB_GROUPS, toolbarGroups, groupTools, buttonWidth, splitGroups, insertItems, toolbarState, sameState,
  rovingNext, keyboardInset, effectiveStyle, effectivePosition, animateOn, barPhase, ribbonTabs, ribbonTabOr,
  type ToolbarGroupId, type ToolId, type ToolbarState, type BlockType, type RibbonTab,
} from '~/editor/toolbarModel'
import { useBreakpoint } from '~/composables/useBreakpoint'
import { useI18n } from '~/composables/useI18n'
import { useUserPrefs } from '~/composables/usePrefs'
import { useToolbarConfig, useRecentColors } from '~/composables/useToolbarConfig'
import { links, useWorkspace } from '~/composables/useWorkspace'

/**
 * Barre d'outils fixe de l'éditeur de pages. L'état de la sélection est calculé une fois par image
 * (rAF) et ne remplace la valeur réactive que s'il a changé : une frappe ordinaire ne réaffiche rien.
 */
const props = defineProps<{ editor: Editor; slash: SlashContext }>()
const { t } = useI18n()
const { isMobile } = useBreakpoint()
const { update: updatePrefs } = useUserPrefs()
const { cfg, patch: patchCfg } = useToolbarConfig()
const { recent, push: pushRecent } = useRecentColors()
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
// « update » : setEditable (passage lecture -> édition) ne passe pas par une transaction.
onMounted(() => { props.editor.on('transaction', refresh); props.editor.on('update', refresh); refresh() })
onBeforeUnmount(() => { props.editor.off('transaction', refresh); props.editor.off('update', refresh); if (raf) cancelAnimationFrame(raf) })

// --- Style, position, largeur disponible ------------------------------------------------------
const ribbon = computed(() => effectiveStyle(cfg.value, isMobile.value) === 'ribbon')
const position = computed(() => effectivePosition(cfg.value, isMobile.value))
const atBottom = computed(() => position.value === 'bottom')
const reducedMotion = typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches
const animated = computed(() => animateOn(cfg.value, reducedMotion))
const placeUp = computed(() => (atBottom.value ? 'top' : 'bottom'))
const menuPlace = computed(() => (atBottom.value ? 'top-start' : 'bottom-start') as 'top-start' | 'bottom-start')
const menuPlaceEnd = computed(() => (atBottom.value ? 'top-end' : 'bottom-end') as 'top-end' | 'bottom-end')
const iconSize = computed(() => (ribbon.value || isMobile.value ? 18 : 16))

const tab = ref<RibbonTab>('home')
const curTab = computed(() => ribbonTabOr(tab.value, S.value.inTable))
const tabs = computed(() => ribbonTabs(S.value.inTable))

// Largeur disponible -> groupes visibles ou repliés dans « Plus » (la barre est observée, pas la fenêtre).
const width = ref(1000)
useResizeObserver(root, (entries) => { width.value = Math.round(entries[0]?.contentRect.width ?? 1000) })
const bw = computed(() => buttonWidth(cfg.value.size, isMobile.value) + (ribbon.value ? 6 : 0))
const allGroups = computed(() => toolbarGroups(cfg.value, S.value.inTable, ribbon.value ? TAB_GROUPS[curTab.value] : undefined, isMobile.value))
const split = computed(() => splitGroups(width.value - (isMobile.value ? 10 : 16), allGroups.value, bw.value, ribbon.value || isMobile.value ? 0 : bw.value + 6, isMobile.value ? SEP_WIDTH_TOUCH : SEP_WIDTH))
const shown = computed(() => split.value.visible)
const hasOverflow = computed(() => split.value.overflow.length > 0)
// Téléphone : pas de place pour un second bouton, les options passent dans le menu « Plus ».
const showMore = computed(() => hasOverflow.value || (isMobile.value && !ribbon.value))

// Bas : fixée au bas de la fenêtre (au-dessus du clavier virtuel), alignée sur la colonne de l'éditeur.
const kb = ref(0)
function onViewport() { kb.value = keyboardInset(window.innerHeight, window.visualViewport) }
onMounted(onViewport)
useEventListener(() => (typeof window !== 'undefined' ? window.visualViewport : null), 'resize', onViewport)
useEventListener(() => (typeof window !== 'undefined' ? window.visualViewport : null), 'scroll', onViewport)
const host = ref<{ left: number; width: number } | null>(null)
function measureHost() {
  const p = root.value?.parentElement
  if (!p) return
  const r = p.getBoundingClientRect()
  host.value = { left: Math.round(r.left), width: Math.round(r.width) }
}
useResizeObserver(() => root.value?.parentElement, measureHost)
useEventListener(() => (typeof window !== 'undefined' ? window : null), 'resize', measureHost)
watch(root, () => nextTick(measureHost))
const rootStyle = computed(() => {
  const st: Record<string, string> = { '--tb-op': String(cfg.value.opacity) }
  if (atBottom.value) {
    st.bottom = `calc(${kb.value}px + max(10px, var(--safe-bottom, 0px)))`
    if (!isMobile.value && host.value) { st.left = `${host.value.left}px`; st.width = `${host.value.width}px` }
  }
  return st
})

// --- Pendant la frappe : réduite, translucide ou masquée, de retour dès que la souris bouge ---------
const typing = ref(false)
const hover = ref(false)
const focusIn = ref(false)
const anyOpen = computed(() => blockOpen.value || alignOpen.value || tableOpen.value || insertOpen.value || moreOpen.value || colorOpen.value || colorMoreOpen.value || linkOpen.value || optsOpen.value || custOpen.value || peek.value)
const phase = computed(() => barPhase(typing.value, hover.value || focusIn.value || anyOpen.value, cfg.value))
const wantsTyping = computed(() => cfg.value.reduce || cfg.value.translucent || cfg.value.autoHide)
let startTimer = 0
let idleTimer = 0
let lx = 0
let ly = 0
function stopTyping() { clearTimeout(startTimer); clearTimeout(idleTimer); startTimer = 0; typing.value = false }
function onTypeKey(e: KeyboardEvent) {
  if (!wantsTyping.value || e.ctrlKey || e.metaKey || e.altKey || ['Shift', 'Control', 'Alt', 'Meta', 'Escape', 'Tab'].includes(e.key)) return
  if (!typing.value && !startTimer) startTimer = window.setTimeout(() => { startTimer = 0; typing.value = true }, 450)
  clearTimeout(idleTimer)
  idleTimer = window.setTimeout(stopTyping, 4000)
}
function onPointer(e: PointerEvent) {
  const moved = Math.hypot(e.clientX - lx, e.clientY - ly) > 4
  lx = e.clientX; ly = e.clientY
  if ((typing.value || startTimer) && moved) stopTyping()
}
onMounted(() => { try { props.editor.view.dom.addEventListener('keydown', onTypeKey, { passive: true }) } catch { /* éditeur détruit */ } })
onBeforeUnmount(() => { stopTyping(); try { props.editor.view.dom.removeEventListener('keydown', onTypeKey) } catch { /* éditeur détruit */ } })
useEventListener(() => (typeof document !== 'undefined' ? document : null), 'pointermove', onPointer, { passive: true })
watch(wantsTyping, (v) => { if (!v) stopTyping() })

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

interface Btn { id: string; bar?: string | null; label: string; icon: Component; shortcut?: string[]; on?: boolean; disabled?: boolean; run: () => void }
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
const lastHighlight = computed(() => (recent.value.highlight?.[0] as string | undefined) ?? 'yellow')
const highlightBtn = computed<Btn>(() => ({
  id: 'highlight', label: t('editor.highlight'), icon: Highlighter, shortcut: ['mod', 'shift', 'H'], on: S.value.highlight, bar: S.value.highlightColor ?? lastHighlight.value,
  run: () => (S.value.highlight ? ch().unsetHighlight().run() : ch().setHighlight({ color: lastHighlight.value }).run()),
}))
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
  { id: 'table', label: t('toolbar.table'), icon: Table, run: () => runSlash('table') },
])
/** Boutons simples par outil : un seul modèle pour la barre, le ruban et le menu « Plus ». */
const simple = computed<Record<string, Btn>>(() => Object.fromEntries([...historyBtns.value, ...markBtns.value, highlightBtn.value, ...listBtns.value, ...insertBtns.value].map((b) => [b.id, b])))
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
const colorMoreOpen = ref(false)
const optsOpen = ref(false)
const custOpen = ref(false)
const peek = ref(false)

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

// --- Couleur : palette (texte et surlignage) -------------------------------------------------------
const colorBar = computed(() => (S.value.color ? `var(--pal-${S.value.color})` : null))
function pickText(c: string | null) {
  colorOpen.value = false
  colorMoreOpen.value = false
  if (c) { ch().setTextColor(c).run(); pushRecent('text', c) } else ch().unsetTextColor().run()
}
function pickHighlight(c: string | null) {
  colorOpen.value = false
  colorMoreOpen.value = false
  if (c) { ch().setHighlight({ color: c }).run(); pushRecent('highlight', c) } else ch().unsetHighlight().run()
}
// Palette ouverte depuis le menu « Plus » : ancrée sur le bouton « Plus ».
const morePos = ref<{ x: number; y: number } | null>(null)
function openPaletteFromMore() {
  const r = root.value?.querySelector('[data-tb="more"]')?.getBoundingClientRect()
  morePos.value = r ? { x: r.right - 120, y: atBottom.value ? r.top - 6 : r.bottom + 6 } : null
  nextTick(() => { colorMoreOpen.value = true })
}

// --- Lien -------------------------------------------------------------------------------------
const linkOpen = ref(false)
const linkValue = ref('')
const linkPos = ref<{ x: number; y: number } | null>(null)
const linkInput = ref<HTMLInputElement | null>(null)
function openLink() {
  const r = (root.value?.getBoundingClientRect() ?? { left: 0, bottom: 0, top: 0 })
  linkPos.value = { x: r.left + 8, y: atBottom.value ? r.top - 6 : r.bottom + 6 }
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

// --- Menu « Plus » : groupes repliés, du plus utile au moins utile ---------------------------------
function asItem(b: Btn): MenuItem { return { label: b.label, icon: b.icon, shortcut: b.shortcut, checked: b.on, disabled: b.disabled, onSelect: b.run } }
const overflowItems = computed<MenuItem[]>(() => {
  const out: MenuItem[] = []
  for (const id of split.value.overflow) {
    const tools = groupTools(id, cfg.value, S.value.inTable)
    const items: MenuItem[] = []
    switch (id as ToolbarGroupId) {
      case 'block': items.push(...blockItems.value); break
      case 'align': items.push(...alignItems.value); break
      case 'color':
        if (tools.includes('color')) items.push({ label: t('toolbar.colorTool'), icon: Palette, onSelect: openPaletteFromMore })
        if (tools.includes('highlight')) items.push(asItem(highlightBtn.value))
        break
      case 'blocks': items.push(...insertMenu.value); break
      case 'table': items.push(...tableItems.value); break
      default: items.push(...tools.map((tl) => simple.value[tl]).filter((b): b is Btn => !!b).map(asItem))
    }
    if (items.length && out.length) out.push({ divider: true })
    out.push(...items)
  }
  if (isMobile.value) out.push(...(out.length ? [{ divider: true } as MenuItem] : []), ...optionItems.value)
  return out
})
function hide() { updatePrefs({ editorToolbar: false }).catch(() => {}) }

// --- Options (personnaliser, position, style) ------------------------------------------------------
const optionItems = computed<MenuItem[]>(() => [
  { label: t('toolbar.customize'), icon: SlidersHorizontal, onSelect: () => { custOpen.value = true } },
  { label: atBottom.value ? t('toolbar.moveTop') : t('toolbar.moveBottom'), icon: atBottom.value ? PanelTop : PanelBottom, onSelect: () => { patchCfg({ position: atBottom.value ? 'top' : 'bottom' }).catch(() => {}) } },
  ...(isMobile.value ? [] : [{ label: ribbon.value ? t('toolbar.useBar') : t('toolbar.useRibbon'), icon: LayoutTemplate, onSelect: () => { patchCfg({ style: ribbon.value ? 'bar' : 'ribbon' }).catch(() => {}) } }]),
  { divider: true },
  { label: t('toolbar.hide'), icon: EyeOff, onSelect: hide },
])

// --- Ruban : onglets, repli -------------------------------------------------------------------------
const TAB_KEY: Record<RibbonTab, string> = { home: 'toolbar.tabHome', insert: 'toolbar.tabInsert', table: 'toolbar.tabTable' }
const bodyShown = computed(() => !ribbon.value || !cfg.value.collapsed || peek.value)
function selectTab(tb: RibbonTab) {
  if (cfg.value.collapsed) peek.value = tab.value === tb && peek.value ? false : true
  tab.value = tb
}
function toggleCollapse() { peek.value = false; patchCfg({ collapsed: !cfg.value.collapsed }).catch(() => {}) }
function onBodyClick(e: MouseEvent) { if (peek.value && (e.target as HTMLElement).closest('.tb-btn')) nextTick(() => { peek.value = false }) }
useEventListener(() => (typeof document !== 'undefined' ? document : null), 'pointerdown', (e: PointerEvent) => { if (peek.value && !root.value?.contains(e.target as Node)) peek.value = false })

// --- Navigation au clavier (barre d'outils à tabulation unique) ---------------------------------
const rover = ref(0)
function buttons(): HTMLElement[] { return [...(root.value?.querySelectorAll<HTMLElement>('.tb-btn:not([disabled])') ?? [])] }
function syncTab() {
  root.value?.querySelectorAll('.tb-btn').forEach((b) => b.setAttribute('tabindex', '-1'))
  buttons()[rover.value]?.setAttribute('tabindex', '0')
}
onMounted(syncTab)
onUpdated(syncTab)
function onTabKey(e: KeyboardEvent) {
  if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(e.key)) return
  const list = tabs.value
  const i = rovingNext(list.indexOf(curTab.value), list.length, e.key)
  if (i < 0) return
  e.preventDefault()
  tab.value = list[i]!
  nextTick(() => root.value?.querySelector<HTMLElement>(`[data-tab="${tab.value}"]`)?.focus())
}
function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') { peek.value = false; props.editor.commands.focus(); return }
  if ((e.target as HTMLElement).closest?.('.rb-tab')) { onTabKey(e); return }
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
    :class="[
      ribbon ? 'ribbon' : 'bar', atBottom ? 'bottom' : 'top', { mobile: isMobile, anim: animated, comfy: cfg.size === 'comfortable', shrunk: phase.shrunk, faded: phase.faded, away: phase.hidden, collapsed: ribbon && cfg.collapsed, peek, nolabels: !cfg.labels }
    ]"
    :style="rootStyle"
    :role="ribbon ? undefined : 'toolbar'"
    :aria-label="ribbon ? undefined : t('toolbar.label')"
    :aria-orientation="ribbon ? undefined : 'horizontal'"
    data-doc-toolbar
    :data-style="ribbon ? 'ribbon' : 'bar'"
    :data-position="position"
    :data-phase="phase.hidden ? 'hidden' : phase.faded ? 'faded' : phase.shrunk ? 'shrunk' : 'normal'"
    @mousedown.prevent
    @keydown="onKey"
    @focusin="onFocusIn(($event as FocusEvent)); focusIn = true"
    @focusout="focusIn = false"
    @pointerenter="hover = true"
    @pointerleave="hover = false"
  >
    <div v-if="ribbon" class="rb-tabs" role="tablist" :aria-label="t('toolbar.label')">
      <button
        v-for="tb in tabs" :key="tb" class="rb-tab" role="tab" type="button" :class="{ on: curTab === tb }" :data-tab="tb"
        :aria-selected="curTab === tb" :tabindex="curTab === tb ? 0 : -1" @click="selectTab(tb)"
      >{{ t(TAB_KEY[tb]) }}</button>
      <span class="spacer" />
      <UiMenu v-model:open="optsOpen" :items="optionItems" :width="250" :placement="menuPlaceEnd">
        <UiTooltip :text="t('toolbar.options')" :placement="placeUp">
          <button class="tb-btn" data-tb="options" :aria-label="t('toolbar.options')" aria-haspopup="menu" :aria-expanded="optsOpen"><SlidersHorizontal :size="15" /></button>
        </UiTooltip>
      </UiMenu>
      <UiTooltip :text="cfg.collapsed ? t('toolbar.expandRibbon') : t('toolbar.collapseRibbon')" :placement="placeUp">
        <button class="tb-btn" data-tb="collapse" :aria-label="cfg.collapsed ? t('toolbar.expandRibbon') : t('toolbar.collapseRibbon')" :aria-expanded="!cfg.collapsed" @click="toggleCollapse"><ChevronUp :size="15" class="chev" :class="{ flip: cfg.collapsed }" /></button>
      </UiTooltip>
    </div>

    <div v-show="bodyShown" class="tb-body" :role="ribbon ? 'toolbar' : undefined" :aria-label="ribbon ? t('toolbar.label') : undefined" @click="onBodyClick">
      <div v-for="g in shown" :key="g" class="grp" role="group" :aria-label="t(GROUP_LABEL[g])" :data-group="g">
        <div class="grp-tools">
          <template v-for="tool in groupTools(g, cfg, S.inTable)" :key="tool">
            <UiMenu v-if="tool === 'block'" v-model:open="blockOpen" :items="blockItems" :width="200" :placement="menuPlace">
              <UiTooltip :text="t('toolbar.blockStyle')" :placement="placeUp">
                <button class="tb-btn tb-wide" data-tb="block" :aria-label="t('toolbar.blockStyle')" aria-haspopup="menu" :aria-expanded="blockOpen">
                  <component :is="currentBlock.icon" :size="iconSize" /><span v-if="cfg.labels" class="tb-label">{{ t(currentBlock.key) }}</span><ChevronDown :size="12" />
                </button>
              </UiTooltip>
            </UiMenu>

            <UiPopover v-else-if="tool === 'color'" v-model:open="colorOpen" :width="240" :placement="menuPlace" :sheet-on-mobile="false">
              <template #trigger>
                <UiTooltip :text="t('toolbar.colorTool')" :placement="placeUp">
                  <button class="tb-btn" :class="{ on: !!S.color }" data-tb="color" :aria-label="t('toolbar.colorTool')" aria-haspopup="dialog" :aria-expanded="colorOpen">
                    <span class="glyph"><Palette :size="iconSize" /><i class="bar" :style="{ background: colorBar ?? 'var(--text-3)', opacity: colorBar ? 1 : 0.35 }" /></span><ChevronDown :size="11" />
                  </button>
                </UiTooltip>
              </template>
              <DocPalette :color="S.color" :highlight="S.highlightColor" :recent-text="recent.text ?? []" :recent-highlight="recent.highlight ?? []" @text="pickText" @highlight="pickHighlight" />
            </UiPopover>

            <UiMenu v-else-if="tool === 'align'" v-model:open="alignOpen" :items="alignItems" :width="190" :placement="menuPlace">
              <UiTooltip :text="t('toolbar.align')" :placement="placeUp">
                <button class="tb-btn" data-tb="align" :aria-label="t('toolbar.align')" aria-haspopup="menu" :aria-expanded="alignOpen"><component :is="currentAlign.icon" :size="iconSize" /><ChevronDown :size="11" /></button>
              </UiTooltip>
            </UiMenu>

            <UiMenu v-else-if="tool === 'insert'" v-model:open="insertOpen" :items="insertMenu" :width="240" :placement="menuPlace">
              <UiTooltip :text="t('toolbar.insert')" :placement="placeUp">
                <button class="tb-btn tb-wide" data-tb="insert" :aria-label="t('toolbar.insert')" aria-haspopup="menu" :aria-expanded="insertOpen"><Plus :size="iconSize" /><span v-if="cfg.labels" class="tb-label">{{ t('toolbar.insert') }}</span><ChevronDown :size="12" /></button>
              </UiTooltip>
            </UiMenu>

            <UiMenu v-else-if="tool === 'table-actions'" v-model:open="tableOpen" :items="tableItems" :width="220" :placement="menuPlace">
              <UiTooltip :text="t('toolbar.tableActions')" :placement="placeUp">
                <button class="tb-btn on" data-tb="table-actions" :aria-label="t('toolbar.tableActions')" aria-haspopup="menu" :aria-expanded="tableOpen"><Table2 :size="iconSize" /><ChevronDown :size="11" /></button>
              </UiTooltip>
            </UiMenu>

            <UiTooltip v-else-if="simple[tool]" :text="simple[tool]!.label" :shortcut="simple[tool]!.shortcut" :placement="placeUp">
              <button
                class="tb-btn" :class="{ on: simple[tool]!.on }" :data-tb="tool" :aria-label="simple[tool]!.label"
                :aria-pressed="simple[tool]!.on === undefined ? undefined : !!simple[tool]!.on" :disabled="simple[tool]!.disabled" @click="simple[tool]!.run()"
              >
                <span v-if="simple[tool]!.bar" class="glyph"><component :is="simple[tool]!.icon" :size="iconSize" /><i class="bar" :style="{ background: `var(--pal-${simple[tool]!.bar})` }" /></span>
                <component :is="simple[tool]!.icon" v-else :size="iconSize" />
              </button>
            </UiTooltip>
          </template>
        </div>
        <span v-if="ribbon && cfg.labels" class="grp-label">{{ t(GROUP_LABEL[g]) }}</span>
      </div>

      <span class="spacer" />
      <UiMenu v-if="showMore" v-model:open="moreOpen" :items="overflowItems" :width="250" :placement="menuPlaceEnd">
        <UiTooltip :text="t('toolbar.more')" :placement="placeUp">
          <button class="tb-btn" data-tb="more" :aria-label="t('toolbar.more')" aria-haspopup="menu" :aria-expanded="moreOpen"><MoreHorizontal :size="iconSize" /></button>
        </UiTooltip>
      </UiMenu>
      <UiMenu v-if="!ribbon && !isMobile" v-model:open="optsOpen" :items="optionItems" :width="250" :placement="menuPlaceEnd">
        <UiTooltip :text="t('toolbar.options')" :placement="placeUp">
          <button class="tb-btn tb-opts" data-tb="options" :aria-label="t('toolbar.options')" aria-haspopup="menu" :aria-expanded="optsOpen"><SlidersHorizontal :size="15" /></button>
        </UiTooltip>
      </UiMenu>
    </div>

    <UiPopover v-if="split.overflow.includes('color')" v-model:open="colorMoreOpen" :virtual-rect="morePos" :width="240" :placement="menuPlace" :sheet-on-mobile="false">
      <DocPalette :color="S.color" :highlight="S.highlightColor" :recent-text="recent.text ?? []" :recent-highlight="recent.highlight ?? []" @text="pickText" @highlight="pickHighlight" />
    </UiPopover>

    <UiPopover v-model:open="linkOpen" :virtual-rect="linkPos" :width="320" :placement="menuPlace">
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
  <DocToolbarSettings v-model:open="custOpen" />
</template>

<style scoped>
.tb {
  --bh: 32px;
  --ico: 16px;
  z-index: 20;
  min-width: 0;
  background: var(--bg-elevated);
  border-radius: var(--r-md);
  box-shadow: var(--shadow-md);
  transform-origin: top center;
}
.tb.comfy { --bh: 38px; }
.tb.mobile { --bh: 40px; }
.tb.top { position: sticky; top: 8px; margin: 0 0 12px; }
.tb.bottom { position: fixed; left: 8px; right: 8px; margin: 0; z-index: 40; transform-origin: bottom center; box-shadow: var(--shadow-md), 0 -1px 0 var(--border); }
.tb.bottom:not(.mobile) { left: 0; right: auto; }
.tb.bar { display: flex; align-items: center; gap: 1px; padding: 3px 4px; overflow: hidden; }
.tb.bar .tb-body { display: flex; align-items: center; flex: 1; min-width: 0; gap: 0; }
/* Colonne de 760px trop étroite pour tous les groupes : la barre du haut déborde vers la marge gauche, vide, quand il y a la place. */
@media (min-width: 1100px) { .tb.top.bar:not(.mobile) { margin-left: -76px; } }

.grp { display: flex; flex-direction: column; align-items: center; flex: none; position: relative; }
.grp-tools { display: flex; align-items: center; gap: 1px; }
.tb.bar .grp + .grp { margin-left: 9px; }
.tb.bar .grp + .grp::before { content: ''; position: absolute; left: -5px; top: 50%; width: 1px; height: 18px; margin-top: -9px; background: var(--border-strong); }
.spacer { flex: 1; }
.tb :deep(.popover-ref), .tb :deep(.tt-ref) { flex: none; }

.tb-btn {
  height: var(--bh);
  min-width: var(--bh);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 3px;
  padding: 0 4px;
  border-radius: var(--r-sm);
  color: var(--text-2);
  transition: background var(--dur-fast), color var(--dur-fast), transform var(--dur-fast);
}
.tb-btn:hover:not(:disabled) { background: var(--bg-hover); color: var(--text); }
.tb-btn:active:not(:disabled) { transform: scale(0.94); }
.tb-btn:focus-visible { outline: 2px solid var(--border-focus); outline-offset: -2px; }
.tb-btn.on, .tb-btn[aria-expanded='true'] { color: var(--accent-text); background: var(--accent-soft); }
.tb-btn:disabled { opacity: 0.4; cursor: default; }
.tb-wide { padding: 0 7px; }
.tb-label { font-size: var(--fs-md); white-space: nowrap; }
.tb-opts { opacity: 0.7; }
.glyph { position: relative; display: inline-grid; place-items: center; }
.glyph .bar { position: absolute; left: 1px; right: 1px; bottom: -3px; height: 3px; border-radius: 2px; }
.link-row { display: flex; align-items: center; gap: 6px; padding: 0 4px 0 10px; height: 34px; }
.link-row input { flex: 1; min-width: 0; border: none; outline: none; background: transparent; }
.link-row button { width: 28px; height: 28px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-2); }
.link-row button:hover { background: var(--bg-hover); color: var(--text); }
.link-results { border-top: 1px solid var(--border); padding: 4px; display: flex; flex-direction: column; }
.link-results button { display: flex; align-items: center; gap: 8px; height: 32px; padding: 0 8px; border-radius: var(--r-sm); text-align: left; }
.link-results button:hover { background: var(--bg-hover); }

/* Ruban : onglets en haut, groupes légendés en dessous, boutons plus grands. */
.tb.ribbon { display: block; overflow: visible; }
.rb-tabs { display: flex; align-items: center; gap: 2px; padding: 4px 6px 0; overflow-x: auto; scrollbar-width: none; border-bottom: 1px solid var(--border); }
.rb-tabs::-webkit-scrollbar { display: none; }
.rb-tab { height: 32px; padding: 0 14px; flex: none; border-radius: var(--r-sm) var(--r-sm) 0 0; color: var(--text-2); font-weight: 500; position: relative; }
.rb-tab:hover { color: var(--text); background: var(--bg-hover); }
.rb-tab.on { color: var(--accent-text); }
.rb-tab.on::after { content: ''; position: absolute; left: 8px; right: 8px; bottom: -1px; height: 2px; border-radius: 2px; background: var(--accent); }
.rb-tab:focus-visible { outline: 2px solid var(--border-focus); outline-offset: -2px; }
.rb-tabs .tb-btn { --bh: 28px; flex: none; }
.chev { transition: transform var(--dur-fast); }
.chev.flip { transform: rotate(180deg); }
.tb.ribbon .tb-body { display: flex; align-items: stretch; padding: 6px 8px 4px; gap: 0; min-height: 66px; }
.tb.ribbon .tb-body > .tb-btn, .tb.ribbon .tb-body > :deep(.popover-ref) { align-self: center; }
.tb.ribbon .grp { justify-content: space-between; padding: 0 8px; }
.tb.ribbon .grp + .grp { border-left: 1px solid var(--border); }
.tb.ribbon .grp-tools { flex: 1; }
.tb.ribbon .tb-btn { --bh: 38px; }
.tb.ribbon.comfy .tb-btn { --bh: 42px; }
.tb.ribbon .tb-btn { padding: 0 6px; }
.grp-label { font-size: 11px; line-height: 1; color: var(--text-3); padding-top: 4px; white-space: nowrap; }
.tb.ribbon.collapsed .tb-body { display: none; }
.tb.ribbon.collapsed.peek .tb-body { display: flex; position: absolute; left: 0; right: 0; top: 100%; background: var(--bg-elevated); border-radius: var(--r-md); box-shadow: var(--shadow-md); margin-top: 4px; z-index: 2; }
.tb.ribbon.collapsed.bottom.peek .tb-body { top: auto; bottom: 100%; margin: 0 0 4px; }
.tb.ribbon.collapsed .rb-tabs { border-bottom-color: transparent; }

/* Téléphone : barre compacte fixée au-dessus du clavier, boutons de 40px, sans gêner le texte (marge basse de l'éditeur). */
.tb.mobile.bar .tb-label { display: none; }
.tb.mobile { padding: 4px; }
.tb.mobile .tb-wide { padding: 0 2px; }
.tb.mobile.bar .grp + .grp { margin-left: 5px; }
.tb.mobile.bar .grp + .grp::before { left: -3px; }
.tb.nolabels .tb-label { display: none; }

/* Animation (option, désactivée par défaut si le système demande moins de mouvement) : uniquement opacité et transform. */
.tb.anim { transition: opacity 0.22s ease, transform 0.22s ease; animation: tb-in 0.24s ease-out both; }
.tb.anim.bottom { --from: 12px; }
.tb.anim.top { --from: -8px; }
@keyframes tb-in { from { opacity: 0; transform: translateY(var(--from, 0)); } to { opacity: 1; transform: none; } }
.tb.anim .grp { animation: grp-in 0.2s ease-out both; }
@keyframes grp-in { from { opacity: 0; transform: scale(0.96); } to { opacity: 1; transform: none; } }
/* Pendant la frappe (la survol et le focus annulent tout) */
.tb.shrunk { transform: scale(0.9); }
.tb.faded { opacity: var(--tb-op, 0.55); }
.tb.away { opacity: 0; transform: translateY(-10px); pointer-events: none; }
.tb.away.bottom { transform: translateY(10px); }
@media (prefers-reduced-motion: reduce) { .tb-btn, .chev { transition: none; } .tb.anim, .tb.anim .grp { animation: none; transition: none; } }
</style>
