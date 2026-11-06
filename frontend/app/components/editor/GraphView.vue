<script setup lang="ts">
import { NodeViewWrapper, nodeViewProps } from '@tiptap/vue-3'
import { Code2, Eye, Columns2, Maximize2, Minimize2, Plus, Spline, ArrowRight, Minus, Star, Trash2, LayoutDashboard, Scan } from 'lucide-vue-next'
import type { Core, NodeSingular, EdgeSingular, Position } from 'cytoscape'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { useDebounceFn } from '@vueuse/core'
import { useTheme } from '~/composables/useTheme'
import { useI18n } from '~/composables/useI18n'
import { nodeSource, useSourceField } from '~/editor/source'
import { diagramStyleSettings, resolveStyle, FONT_STACKS } from '~/editor/diagramStyles'
import { parseGraph, updateNodes, addEdge, removeElements, updateEdge, setDirected, nextNodeId, type Graph } from '~/editor/graphSource'

/**
 * Bloc graphe (théorie des graphes) : nœuds et arcs placés à la souris, étiquettes (poids,
 * flots), éléments mis en avant. La source est le texte du nœud, une ligne par élément.
 */
const props = defineProps(nodeViewProps)
const { t } = useI18n()
const { mode } = useTheme()
const area = ref<HTMLTextAreaElement | null>(null)
const field = useSourceField(area, props as any)
const code = field.draft
const { carets, present } = field
const view = ref<'graph' | 'split' | 'code'>(nodeSource(toRaw(props.node)) || !props.editor.isEditable ? 'graph' : 'split')
const graph = computed<Graph>(() => parseGraph(code.value))
const editable = () => props.editor.isEditable

const styleId = computed<string | null>({
  get: () => props.node.attrs.diagramStyle ?? null,
  set: (v) => props.updateAttributes({ diagramStyle: v }),
})
const style = computed(() => resolveStyle(styleId.value, diagramStyleSettings.value))
const isDark = () => document.documentElement.classList.contains('dark') || !document.documentElement.classList.contains('light')
const dark = ref(false)
const palette = computed(() => (dark.value ? style.value.dark : style.value.light))

const host = ref<HTMLDivElement | null>(null)
const stage = ref<HTMLDivElement | null>(null)
const full = ref(false)
const linking = ref(false)
const linkFrom = ref<string | null>(null)
const selectedCount = ref(0)
const directedNew = ref(true)
watch(() => graph.value.edges, (e) => { if (e.length) directedNew.value = e.some((x) => x.directed) }, { immediate: true })

let cy: Core | null = null
let ro: ResizeObserver | null = null

// --- Apparence -------------------------------------------------------------

const SHAPES = { rounded: 'round-rectangle', rect: 'rectangle', circle: 'ellipse' } as const
function nodeWidth(label: string, circle: boolean) {
  const w = Math.min(180, label.length * 8.2 + (circle ? 14 : 22))
  return Math.max(circle ? 38 : 44, w)
}
function sheet(): any[] {
  const s = style.value, c = palette.value, font = FONT_STACKS[s.font]
  const circle = s.shape === 'circle'
  return [
    { selector: 'node', style: {
      shape: SHAPES[s.shape], 'background-color': c.node, 'border-color': c.border, 'border-width': 1.5,
      label: 'data(label)', color: c.text, 'font-family': font, 'font-size': 14, 'text-valign': 'center', 'text-halign': 'center',
      'text-wrap': 'wrap', 'text-max-width': '170px',
      width: (n: NodeSingular) => nodeWidth(String(n.data('label') ?? ''), circle),
      height: (n: NodeSingular) => (circle ? nodeWidth(String(n.data('label') ?? ''), true) : 36),
    } },
    { selector: 'edge', style: {
      width: 1.6, 'line-color': c.edge, 'target-arrow-color': c.edge, 'curve-style': 'bezier', 'arrow-scale': 1.15,
      label: 'data(label)', 'font-family': font, 'font-size': 13, color: c.text,
      'text-background-color': c.background, 'text-background-opacity': 1, 'text-background-padding': '2px', 'text-background-shape': 'roundrectangle',
      'loop-direction': '-45deg', 'loop-sweep': '-60deg',
    } },
    { selector: 'edge[?directed]', style: { 'target-arrow-shape': 'triangle' } },
    { selector: 'node.hl', style: { 'border-width': 3.5, 'background-color': c.accent } },
    { selector: 'edge.hl', style: { width: 3.4, 'line-color': c.border, 'target-arrow-color': c.border } },
    { selector: ':selected', style: { 'overlay-color': c.border, 'overlay-opacity': 0.16, 'overlay-padding': 5 } },
    { selector: 'node.link-from', style: { 'border-style': 'dashed', 'border-width': 3 } },
  ]
}

// --- Synchronisation texte → graphe ----------------------------------------

/** Place les nœuds sans position, sans l'écrire (seuls un déplacement ou « Disposer » l'écrivent). */
function placeNew(added: NodeSingular[], all: boolean) {
  if (!cy || !added.length) return
  if (all) {
    runLayout(graph.value.edges.some((e) => e.directed) ? 'layers' : 'force', false)
    return
  }
  const ids = new Set(added.map((n) => n.id()))
  const placed = cy.nodes().filter((n) => !ids.has(n.id()))
  const bb = placed.boundingBox({})
  added.forEach((n, i) => {
    const near = n.neighborhood('node').filter((m) => !ids.has(m.id()))
    if (near.length) {
      let x = 0, y = 0
      near.forEach((m) => { x += m.position().x / near.length; y += m.position().y / near.length })
      n.position({ x: x + 90, y: y + 40 * (i + 1) })
    } else n.position({ x: bb.x2 + 100, y: bb.y1 + 70 * i })
  })
}

function sync(fit = false) {
  if (!cy) return
  const g = graph.value
  const ids = new Set(g.nodes.map((n) => n.id))
  const keys = new Set(g.edges.map((e) => `e:${e.key}`))
  const added: NodeSingular[] = []
  const first = cy.nodes().empty()
  cy.batch(() => {
    cy!.elements().filter((el) => (el.isNode() ? !ids.has(el.id()) : !keys.has(el.id()))).remove()
    for (const n of g.nodes) {
      let el = cy!.getElementById(n.id) as NodeSingular
      if (el.empty()) {
        el = cy!.add({ group: 'nodes', data: { id: n.id, label: n.label }, position: { x: n.x ?? 0, y: n.y ?? 0 } }) as unknown as NodeSingular
        if (n.x == null) added.push(el)
      } else {
        if (el.data('label') !== n.label) el.data('label', n.label)
        if (n.x != null && !el.grabbed() && (el.position().x !== n.x || el.position().y !== n.y)) el.position({ x: n.x, y: n.y! })
      }
      el.toggleClass('hl', n.hl)
    }
    for (const e of g.edges) {
      const id = `e:${e.key}`
      let el = cy!.getElementById(id) as EdgeSingular
      if (el.empty()) el = cy!.add({ group: 'edges', data: { id, source: e.from, target: e.to, label: e.label, directed: e.directed } }) as unknown as EdgeSingular
      else { el.data('label', e.label); el.data('directed', e.directed) }
      el.toggleClass('hl', e.hl)
    }
  })
  placeNew(added, added.length === g.nodes.length)
  if (fit || (first && g.nodes.length)) fitView()
}

function fitView() {
  if (!cy || cy.nodes().empty()) return
  cy.fit(undefined, 28)
  if (cy.zoom() > 1.25) { cy.zoom(1.25); cy.center() }
}

// --- Modifications graphe → texte ------------------------------------------

function write(next: string) { if (next !== code.value) code.value = next }

let pending: Record<string, { x: number; y: number }> = {}
let flushTimer: ReturnType<typeof setTimeout> | null = null
function queuePosition(n: NodeSingular) {
  pending[n.id()] = { ...n.position() }
  if (flushTimer) return
  flushTimer = setTimeout(() => {
    flushTimer = null
    const ch = pending
    pending = {}
    write(updateNodes(code.value, ch))
  }, 0)
}

function addNodeAt(p: Position) {
  const id = nextNodeId(graph.value)
  write(updateNodes(code.value, { [id]: { label: id, x: p.x, y: p.y } }))
  nextTick(() => startEdit('node', id))
}
function addNodeCenter() {
  if (!cy) return
  const e = cy.extent()
  addNodeAt({ x: (e.x1 + e.x2) / 2 + (Math.random() - 0.5) * 40, y: (e.y1 + e.y2) / 2 + (Math.random() - 0.5) * 40 })
}

function toggleLinking() {
  linking.value = !linking.value
  clearLinkFrom()
}
function clearLinkFrom() {
  if (linkFrom.value) cy?.getElementById(linkFrom.value).removeClass('link-from')
  linkFrom.value = null
}
function linkTo(id: string) {
  if (!linkFrom.value) { linkFrom.value = id; cy?.getElementById(id).addClass('link-from'); return }
  const from = linkFrom.value
  clearLinkFrom()
  write(addEdge(code.value, { from, to: id, directed: directedNew.value }))
  const last = graph.value.edges.at(-1)
  if (last) nextTick(() => startEdit('edge', `e:${last.key}`))
}

function toggleDirected() {
  directedNew.value = !directedNew.value
  write(setDirected(code.value, directedNew.value))
}

function selection() {
  const sel = cy?.$(':selected')
  return { nodes: sel?.nodes().map((n) => n.id()) ?? [], edges: sel?.edges().map((e) => e.id().slice(2)) ?? [] }
}
function removeSelected() {
  const s = selection()
  if (s.nodes.length || s.edges.length) write(removeElements(code.value, s.nodes, s.edges))
}
function toggleHighlight() {
  const s = selection()
  const g = graph.value
  const on = ![...g.nodes.filter((n) => s.nodes.includes(n.id)), ...g.edges.filter((e) => s.edges.includes(e.key))].every((x) => x.hl)
  let next = s.nodes.length ? updateNodes(code.value, Object.fromEntries(s.nodes.map((id) => [id, { hl: on }]))) : code.value
  for (const k of s.edges) next = updateEdge(next, k, { hl: on })
  write(next)
}

// --- Dispositions automatiques ---------------------------------------------

type LayoutKind = 'layers' | 'tree' | 'circle' | 'grid' | 'force'
function runLayout(kind: LayoutKind, save = true) {
  if (!cy || cy.nodes().empty()) return
  const roots = cy.nodes().filter((n) => n.incomers('edge[?directed]').empty())
  const opts: Record<LayoutKind, any> = {
    // Couches de gauche à droite (réseaux de flot, DAG) : arbre en largeur, couché.
    layers: { name: 'breadthfirst', directed: true, roots: roots.length ? roots : undefined, spacingFactor: 1.15, transform: (_n: unknown, p: Position) => ({ x: p.y, y: p.x }) },
    tree: { name: 'breadthfirst', directed: true, roots: roots.length ? roots : undefined, spacingFactor: 1.1 },
    circle: { name: 'circle', spacingFactor: 1.1 },
    grid: { name: 'grid', avoidOverlap: true },
    force: { name: 'cose', animate: false, nodeRepulsion: () => 9000, idealEdgeLength: () => 90, randomize: true },
  }
  const layout = cy.layout({ ...opts[kind], fit: false, padding: 28, animate: false })
  layout.one('layoutstop', () => {
    fitView()
    if (save && editable()) write(updateNodes(code.value, Object.fromEntries(cy!.nodes().map((n) => [n.id(), { x: n.position().x, y: n.position().y }]))))
  })
  layout.run()
}
const layoutItems = computed<MenuItem[]>(() => (['layers', 'tree', 'circle', 'grid', 'force'] as const).map((k) => ({ label: t(`graphBlock.layouts.${k}`), onSelect: () => runLayout(k) })))

// --- Édition des étiquettes ------------------------------------------------

const edit = ref<{ kind: 'node' | 'edge'; id: string; x: number; y: number; value: string } | null>(null)
const editInput = ref<HTMLInputElement | null>(null)
function startEdit(kind: 'node' | 'edge', id: string) {
  if (!cy || !editable()) return
  const el = cy.getElementById(id)
  if (el.empty()) return
  const p = kind === 'node' ? (el as NodeSingular).renderedPosition() : (el as EdgeSingular).renderedMidpoint()
  edit.value = { kind, id, x: p.x, y: p.y, value: kind === 'node' ? String(el.data('label') ?? '') : String(el.data('label') ?? '') }
  nextTick(() => { editInput.value?.focus(); editInput.value?.select() })
}
function commitEdit() {
  const e = edit.value
  if (!e) return
  edit.value = null
  if (e.kind === 'node') write(updateNodes(code.value, { [e.id]: { label: e.value.trim() || e.id } }))
  else write(updateEdge(code.value, e.id.slice(2), { label: e.value.trim() }))
}
function onEditKey(ev: KeyboardEvent) {
  ev.stopPropagation()
  if (ev.key === 'Enter') { ev.preventDefault(); commitEdit() }
  else if (ev.key === 'Escape') { ev.preventDefault(); edit.value = null }
}

function onStageKey(e: KeyboardEvent) {
  if (e.target !== stage.value) return
  if (field.onHistoryKey(e)) { e.stopPropagation(); return }
  if (!editable()) return
  if (e.key === 'Delete' || e.key === 'Backspace') { e.preventDefault(); e.stopPropagation(); removeSelected() }
  else if (e.key === 'Escape' && (linking.value || full.value)) { e.preventDefault(); e.stopPropagation(); if (linking.value) { linking.value = false; clearLinkFrom() } else full.value = false }
}

/**
 * Le bloc est déplaçable (glisser-déposer du document) : pendant un geste dans le graphe,
 * le navigateur ne doit pas lancer ce glisser à la place du déplacement d'un nœud.
 */
function holdDrag(e: PointerEvent) {
  const el = (e.currentTarget as HTMLElement).closest('[draggable="true"]') as HTMLElement | null
  if (!el) return
  el.draggable = false
  window.addEventListener('pointerup', () => { el.draggable = true }, { once: true })
}

// --- Cycle de vie ----------------------------------------------------------

onMounted(async () => {
  dark.value = isDark()
  const cytoscape = (await import('cytoscape')).default
  if (!host.value) return
  cy = cytoscape({
    container: host.value,
    style: sheet(),
    minZoom: 0.15, maxZoom: 4,
    userZoomingEnabled: false,
    boxSelectionEnabled: true,
    autoungrabify: !editable(),
    wheelSensitivity: 0.3,
  })
  cy.on('dbltap', (evt) => {
    if (!editable()) return
    if (evt.target === cy) addNodeAt(evt.position)
    else startEdit(evt.target.isNode() ? 'node' : 'edge', evt.target.id())
  })
  cy.on('tap', 'node', (evt) => { if (linking.value && editable()) linkTo(evt.target.id()) })
  cy.on('tap', (evt) => { if (evt.target === cy) clearLinkFrom() })
  cy.on('dragfree', 'node', (evt) => { if (editable()) queuePosition(evt.target) })
  cy.on('select unselect remove', () => { selectedCount.value = cy?.$(':selected').length ?? 0 })
  cy.on('pan zoom', () => { if (edit.value) commitEdit() })
  sync(true)
  ro = new ResizeObserver(() => { cy?.resize(); saveHeight() })
  ro.observe(host.value)
})
onBeforeUnmount(() => { ro?.disconnect(); cy?.destroy(); cy = null; if (flushTimer) clearTimeout(flushTimer) })

watch(graph, () => sync())
// Passage lecture ↔ modification : la vue est redessinée, les nœuds deviennent déplaçables.
const onEditable = () => { cy?.autoungrabify(!editable()); if (!editable() && linking.value) toggleLinking() }
props.editor.on('update', onEditable)
onBeforeUnmount(() => props.editor.off('update', onEditable))
watch([style, mode], () => { dark.value = isDark(); cy?.style(sheet() as any) })
watch(full, (v) => {
  cy?.userZoomingEnabled(v)
  nextTick(() => { cy?.resize(); fitView(); if (v) stage.value?.focus() })
})
watch(view, (v) => { if (v !== 'code') nextTick(() => { cy?.resize(); fitView() }) })

// Hauteur choisie (poignée en bas du bloc), gardée dans le document.
const height = computed(() => Math.min(1200, Math.max(180, Number(props.node.attrs.height) || 340)))
const saveHeight = useDebounceFn(() => {
  const h = host.value?.offsetHeight
  if (h && editable() && !full.value && view.value !== 'code' && Math.abs(h - height.value) > 4) props.updateAttributes({ height: Math.round(h) })
}, 400)

function onKey(e: KeyboardEvent) {
  e.stopPropagation()
  field.onHistoryKey(e)
}
const viewOptions = computed(() => [{ value: 'graph', icon: Eye, title: t('graphBlock.visual') }, { value: 'split', icon: Columns2, title: t('embeds.split') }, { value: 'code', icon: Code2, title: t('embeds.code') }])
</script>

<template>
  <NodeViewWrapper class="gr" :class="{ selected, 'has-who': present.length }" :style="present.length ? { '--who': present[0]!.color } : undefined">
    <div class="gr-bar" contenteditable="false">
      <span class="gr-title" data-drag-handle>{{ t('graphBlock.title') }}</span>
      <span v-if="present.length && !carets.length" class="gr-who"><i v-for="p in present" :key="p.key" :style="{ background: p.color }">{{ p.name }}</i></span>
      <span class="spacer" />
      <EditorDiagramStylePicker v-model="styleId" :editable="editor.isEditable" />
      <button class="gr-btn" :title="t('viewer.fullscreen')" @click="full = true; view = view === 'code' ? 'graph' : view"><Maximize2 :size="14" /></button>
      <UiSegmented v-if="editor.isEditable" v-model="view" :options="viewOptions" />
    </div>
    <div class="gr-body" :class="view" contenteditable="false">
      <div v-if="editor.isEditable && view !== 'graph'" class="gr-code-wrap">
        <textarea ref="area" v-model="code" class="gr-code" spellcheck="false" :placeholder="t('graphBlock.placeholder')" @keydown="onKey" @beforeinput="field.onBeforeInput" />
        <EditorSourceCursors :carets="carets" />
        <div class="gr-help">
          <code>s : Source @ 0 0</code> <code>s -> t : 3/5</code> <code>a -- b : 2</code> <code>a -> b *</code>
          <span>{{ t('graphBlock.syntax') }}</span>
          <span v-for="e in graph.errors.slice(0, 3)" :key="e.line" class="gr-err">{{ t('graphBlock.lineError', { line: e.line + 1 }) }} <code>{{ e.text }}</code></span>
        </div>
      </div>
      <Teleport to="body" :disabled="!full">
        <div v-show="view !== 'code' || full" class="gr-main" :class="{ 'gr-full': full }">
          <div v-if="full" class="gr-full-bar">
            <span class="gr-title">{{ t('graphBlock.title') }}</span>
            <span class="spacer" />
            <EditorDiagramStylePicker v-model="styleId" :editable="editor.isEditable" />
            <button class="gr-btn" :title="t('common.close')" @click="full = false"><Minimize2 :size="14" /></button>
          </div>
          <div v-if="editor.isEditable" class="gr-tools">
            <button class="gr-tool" :title="t('graphBlock.addNodeHint')" @click="addNodeCenter"><Plus :size="14" />{{ t('graphBlock.addNode') }}</button>
            <button class="gr-tool" :class="{ on: linking }" :title="t('graphBlock.linkHint')" @click="toggleLinking"><Spline :size="14" />{{ t('graphBlock.link') }}</button>
            <button class="gr-tool" :title="t('graphBlock.directedHint')" @click="toggleDirected"><component :is="directedNew ? ArrowRight : Minus" :size="14" />{{ directedNew ? t('graphBlock.directed') : t('graphBlock.undirected') }}</button>
            <button class="gr-tool" :disabled="!selectedCount" :title="t('graphBlock.highlightHint')" @click="toggleHighlight"><Star :size="14" />{{ t('graphBlock.highlight') }}</button>
            <UiMenu :items="layoutItems" :width="200">
              <button class="gr-tool" :title="t('graphBlock.layoutHint')"><LayoutDashboard :size="14" />{{ t('graphBlock.layout') }}</button>
            </UiMenu>
            <button class="gr-tool" :disabled="!selectedCount" :title="t('common.delete')" @click="removeSelected"><Trash2 :size="14" /></button>
            <span class="spacer" />
            <button class="gr-tool" :title="t('diagrams.fit')" @click="fitView"><Scan :size="14" /></button>
          </div>
          <div
            ref="stage"
            class="gr-stage"
            tabindex="0"
            :style="{ background: palette.background }"
            @keydown="onStageKey"
            @pointerdown="holdDrag"
          >
            <div ref="host" class="gr-host" :class="{ linking }" :style="full ? undefined : { height: `${height}px` }" />
            <input v-if="edit" ref="editInput" v-model="edit.value" class="gr-edit" :style="{ left: `${edit.x}px`, top: `${edit.y}px` }" @keydown="onEditKey" @blur="commitEdit">
            <span v-if="!graph.nodes.length" class="gr-empty">{{ editor.isEditable ? t('graphBlock.empty') : t('graphBlock.emptyRead') }}</span>
            <span v-else-if="linking" class="gr-mode">{{ linkFrom ? t('graphBlock.linkTarget') : t('graphBlock.linkSource') }}</span>
          </div>
        </div>
      </Teleport>
    </div>
  </NodeViewWrapper>
</template>

<style scoped>
.gr { margin: 1em 0; border-radius: var(--r-lg); background: var(--bg-column); box-shadow: var(--shadow-card); overflow: hidden; }
.gr.selected { box-shadow: 0 0 0 1.5px var(--border-focus); }
.gr.has-who { box-shadow: 0 0 0 1.5px var(--who); }
.gr-who { display: inline-flex; gap: 3px; }
.gr-who i { padding: 0 6px; border-radius: 4px; color: #fff; font: 600 10px/15px var(--font-sans, system-ui, sans-serif); font-style: normal; }
.gr-bar, .gr-full-bar { display: flex; align-items: center; gap: 6px; height: 36px; padding: 0 8px 0 12px; border-bottom: 1px solid var(--border); background: var(--bg-panel); }
.gr-full-bar { height: 44px; }
.gr-title { font-size: var(--fs-xs); font-weight: 600; color: var(--text-3); text-transform: uppercase; letter-spacing: 0.04em; cursor: grab; }
.gr-btn { display: inline-grid; place-items: center; width: 26px; height: 26px; border-radius: var(--r-sm); color: var(--text-2); }
.gr-btn:hover { background: var(--bg-hover); color: var(--text); }
.gr-body { display: grid; }
.gr-body.split { grid-template-columns: minmax(0, 2fr) minmax(0, 3fr); }
.gr-code-wrap { position: relative; display: flex; flex-direction: column; min-width: 0; border-right: 1px solid var(--border); background: var(--bg-code); }
.gr-code { flex: 1; border: none; outline: none; resize: vertical; background: transparent; color: var(--text); min-height: 200px; padding: 12px 14px; font-family: var(--font-mono); font-size: 12.5px; line-height: 1.55; width: 100%; }
.gr-help { display: flex; flex-wrap: wrap; gap: 4px 8px; padding: 8px 12px; border-top: 1px solid var(--border); font-size: 11.5px; color: var(--text-3); }
.gr-help code { font-family: var(--font-mono); font-size: 11px; padding: 0 4px; border-radius: 3px; background: var(--bg-hover); color: var(--text-2); }
.gr-err { width: 100%; color: var(--red); }
.gr-main { display: flex; flex-direction: column; min-width: 0; }
.gr-full { position: fixed; inset: 0; z-index: 2500; background: var(--bg); }
.gr-full .gr-stage { flex: 1; }
.gr-full .gr-host { height: 100%; }
.gr-tools { display: flex; flex-wrap: wrap; align-items: center; gap: 2px; padding: 4px 6px; border-bottom: 1px solid var(--border); background: var(--bg-panel); }
.gr-tool { display: inline-flex; align-items: center; gap: 5px; height: 26px; padding: 0 8px; border-radius: var(--r-sm); font-size: var(--fs-xs); color: var(--text-2); }
.gr-tool:hover:not(:disabled) { background: var(--bg-hover); color: var(--text); }
.gr-tool.on { background: var(--bg-selected, var(--bg-hover)); color: var(--accent, var(--text)); box-shadow: inset 0 0 0 1px var(--border-focus); }
.gr-tool:disabled { opacity: 0.4; }
.gr-stage { position: relative; outline: none; display: flex; flex-direction: column; }
.gr-host { width: 100%; min-height: 180px; resize: vertical; overflow: hidden; }
.gr-full .gr-host { resize: none; }
.gr-host.linking { cursor: crosshair; }
.gr-edit { position: absolute; transform: translate(-50%, -50%); width: 140px; height: 28px; padding: 0 8px; border-radius: var(--r-sm); border: 1px solid var(--border-focus); background: var(--bg-input); color: var(--text); font-size: 13px; text-align: center; outline: none; box-shadow: var(--shadow-card); z-index: 2; }
.gr-empty, .gr-mode { position: absolute; left: 50%; transform: translateX(-50%); pointer-events: none; font-size: var(--fs-sm); color: var(--text-3); }
.gr-empty { top: 45%; }
.gr-mode { bottom: 10px; padding: 3px 10px; border-radius: 99px; background: var(--bg-panel); box-shadow: var(--shadow-card); }
@media (max-width: 639px) { .gr-body.split { grid-template-columns: 1fr; } .gr-code-wrap { border-right: none; border-bottom: 1px solid var(--border); } }
</style>
