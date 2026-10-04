<script setup lang="ts">
import { forceSimulation, forceLink, forceManyBody, forceCollide, forceX, forceY, type Simulation, type SimulationNodeDatum, type SimulationLinkDatum } from 'd3-force'
import type { GraphNode, GraphEdge } from '~/composables/useGraph'

type SimNode = GraphNode & SimulationNodeDatum & { degree: number; r: number }
type SimLink = SimulationLinkDatum<SimNode> & { kind: GraphEdge['kind'] }

const props = defineProps<{ nodes: GraphNode[]; edges: GraphEdge[]; selected?: string | null; search?: string; canLink?: boolean }>()
const emit = defineEmits<{ select: [node: GraphNode | null]; open: [node: GraphNode, newTab: boolean]; link: [a: GraphNode, b: GraphNode] }>()

const wrap = ref<HTMLElement | null>(null)
const canvas = ref<HTMLCanvasElement | null>(null)
let ctx: CanvasRenderingContext2D | null = null
let sim: Simulation<SimNode, SimLink> | null = null
let nodes: SimNode[] = []
let links: SimLink[] = []
let neighbors = new Map<string, Set<string>>()
const view = { x: 0, y: 0, k: 1 }
let width = 0, height = 0, dpr = 1
let hover: SimNode | null = null
let raf = 0
let colors: Record<string, string> = {}

/** Palette catégorielle validée (3 types, toutes paires), une déclinaison par thème. */
const PALETTE = {
  light: { page: '#2a78d6', task: '#eb6834', board: '#1baf7a', edge: '#a5a7ad', text: '#0b0b0b', muted: '#52514e' },
  dark: { page: '#3987e5', task: '#d95926', board: '#199e70', edge: '#55575e', text: '#ffffff', muted: '#c3c2b7' },
}
function readColors() {
  const dark = document.documentElement.classList.contains('dark')
  const s = getComputedStyle(wrap.value!)
  colors = { ...PALETTE[dark ? 'dark' : 'light'], relation: s.getPropertyValue('--accent').trim() || '#5e6ad2', surface: s.getPropertyValue('--bg').trim() || (dark ? '#0b0b0d' : '#fcfcfd') }
}

/** Reconstruit la simulation en gardant les positions des nœuds déjà placés. */
function rebuild() {
  const old = new Map(nodes.map((n) => [n.id, n]))
  const degree = new Map<string, number>()
  for (const e of props.edges) {
    degree.set(e.source, (degree.get(e.source) ?? 0) + 1)
    degree.set(e.target, (degree.get(e.target) ?? 0) + 1)
  }
  nodes = props.nodes.map((n) => {
    const d = degree.get(n.id) ?? 0
    const r = n.kind === 'board' ? 7 + Math.sqrt(d) * 1.2 : n.kind === 'page' ? 4 + Math.sqrt(d) * 1.6 : 3 + Math.sqrt(d) * 1.2
    const prev = old.get(n.id)
    return Object.assign({}, n, { degree: d, r, x: prev?.x, y: prev?.y, vx: prev?.vx, vy: prev?.vy })
  })
  const byId = new Map(nodes.map((n) => [n.id, n]))
  links = props.edges.filter((e) => byId.has(e.source) && byId.has(e.target)).map((e) => ({ source: byId.get(e.source)!, target: byId.get(e.target)!, kind: e.kind }))
  neighbors = new Map()
  for (const l of links) {
    const a = (l.source as SimNode).id, b = (l.target as SimNode).id
    if (!neighbors.has(a)) neighbors.set(a, new Set())
    if (!neighbors.has(b)) neighbors.set(b, new Set())
    neighbors.get(a)!.add(b)
    neighbors.get(b)!.add(a)
  }
  sim?.stop()
  sim = forceSimulation<SimNode>(nodes)
    .force('link', forceLink<SimNode, SimLink>(links).distance((l) => (l.kind === 'board' ? 40 : l.kind === 'parent' ? 45 : 70)).strength((l) => (l.kind === 'board' ? 0.4 : 0.6)))
    .force('charge', forceManyBody<SimNode>().strength((n) => (n.kind === 'board' ? -220 : -70)).distanceMax(400))
    .force('collide', forceCollide<SimNode>((n) => n.r + 3))
    .force('x', forceX(0).strength(0.04))
    .force('y', forceY(0).strength(0.04))
    .alpha(old.size ? 0.4 : 1)
    .on('tick', schedule)
}

function schedule() {
  if (!raf) raf = requestAnimationFrame(draw)
}

function toScreen(x: number, y: number) {
  return [x * view.k + view.x + width / 2, y * view.k + view.y + height / 2] as const
}
function toWorld(sx: number, sy: number) {
  return [(sx - width / 2 - view.x) / view.k, (sy - height / 2 - view.y) / view.k] as const
}

const q = computed(() => props.search?.trim().toLowerCase() ?? '')
let drawingLink: { from: SimNode; x: number; y: number } | null = null

function draw() {
  raf = 0
  if (!ctx) return
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.clearRect(0, 0, width, height)
  ctx.save()
  ctx.translate(width / 2 + view.x, height / 2 + view.y)
  ctx.scale(view.k, view.k)
  const focus = hover?.id ?? props.selected ?? null
  const near = focus ? neighbors.get(focus) ?? new Set<string>() : null
  const lit = (id: string) => !near || id === focus || near.has(id)
  const matches = (n: SimNode) => !q.value || n.label.toLowerCase().includes(q.value)

  // Arêtes : fines et discrètes ; relations manuelles en couleur d'accent, en pointillés.
  for (const l of links) {
    const a = l.source as SimNode, b = l.target as SimNode
    const on = !near || a.id === focus || b.id === focus
    ctx.globalAlpha = on ? (near ? 0.9 : 0.45) : 0.06
    ctx.strokeStyle = l.kind === 'relation' ? colors.relation! : colors.edge!
    ctx.lineWidth = (l.kind === 'relation' ? 1.6 : 1) / Math.sqrt(view.k)
    ctx.setLineDash(l.kind === 'relation' ? [4 / view.k, 3 / view.k] : [])
    ctx.beginPath()
    ctx.moveTo(a.x!, a.y!)
    ctx.lineTo(b.x!, b.y!)
    ctx.stroke()
  }
  ctx.setLineDash([])
  // Nœuds : forme et couleur par type (cercle page, petit cercle tâche, carré tableau).
  for (const n of nodes) {
    const on = lit(n.id) && matches(n)
    ctx.globalAlpha = on ? 1 : 0.12
    const c = colors[n.kind]!
    ctx.fillStyle = c
    ctx.strokeStyle = colors.surface!
    ctx.lineWidth = 2 / view.k
    ctx.beginPath()
    if (n.kind === 'board') {
      const s = n.r * 1.7
      ctx.roundRect(n.x! - s / 2, n.y! - s / 2, s, s, 3)
    } else {
      ctx.arc(n.x!, n.y!, n.r, 0, Math.PI * 2)
    }
    if (n.kind === 'task' && n.done) {
      // Tâche terminée : anneau creux (repère sans dépendre de la couleur seule).
      ctx.fillStyle = colors.surface!
      ctx.fill()
      ctx.strokeStyle = c
      ctx.lineWidth = 1.5 / view.k
      ctx.stroke()
    } else {
      ctx.fill()
      ctx.stroke()
    }
    if (n.id === props.selected) {
      ctx.strokeStyle = colors.text!
      ctx.lineWidth = 2 / view.k
      ctx.beginPath()
      ctx.arc(n.x!, n.y!, n.r + 4 / view.k, 0, Math.PI * 2)
      ctx.stroke()
    }
  }
  // Libellés : tableaux toujours, le reste selon le zoom ; voisinage du survol et recherche.
  ctx.textAlign = 'center'
  ctx.textBaseline = 'top'
  for (const n of nodes) {
    const big = n.kind === 'board' || n.r * view.k > 9 || view.k > 1.6
    const show = (big && !near) || (near && lit(n.id)) || (q.value && matches(n))
    if (!show || (q.value && !matches(n))) continue
    const size = (n.kind === 'board' ? 12 : 11) / view.k
    ctx.font = `${n.kind === 'board' ? 600 : 400} ${size}px system-ui, sans-serif`
    ctx.globalAlpha = lit(n.id) ? 1 : 0.3
    ctx.fillStyle = n.id === focus ? colors.text! : colors.muted!
    const label = (n.icon ? `${n.icon} ` : '') + (n.label.length > 40 ? n.label.slice(0, 38) + '…' : n.label || '—')
    ctx.fillText(label, n.x!, n.y! + n.r + 3 / view.k)
  }
  ctx.restore()
  if (drawingLink) {
    const [sx, sy] = toScreen(drawingLink.from.x!, drawingLink.from.y!)
    ctx.globalAlpha = 1
    ctx.strokeStyle = colors.relation!
    ctx.lineWidth = 2
    ctx.setLineDash([5, 4])
    ctx.beginPath()
    ctx.moveTo(sx, sy)
    ctx.lineTo(drawingLink.x, drawingLink.y)
    ctx.stroke()
    ctx.setLineDash([])
  }
  ctx.globalAlpha = 1
}

function nodeAt(sx: number, sy: number): SimNode | null {
  const [x, y] = toWorld(sx, sy)
  let best: SimNode | null = null
  let bestD = Infinity
  for (const n of nodes) {
    const d = (n.x! - x) ** 2 + (n.y! - y) ** 2
    const hit = (n.r + 4 / view.k) ** 2
    if (d < hit && d < bestD) { best = n; bestD = d }
  }
  return best
}

// --- Interaction : survol, glisser (nœud ou fond), Maj+glisser = relier, molette = zoom ---
let drag: { node: SimNode | null; sx: number; sy: number; vx: number; vy: number; moved: boolean } | null = null
function pos(e: PointerEvent | WheelEvent | MouseEvent) {
  const r = canvas.value!.getBoundingClientRect()
  return [e.clientX - r.left, e.clientY - r.top] as const
}
function onPointerDown(e: PointerEvent) {
  const [sx, sy] = pos(e)
  const n = nodeAt(sx, sy)
  canvas.value!.setPointerCapture(e.pointerId)
  if (n && e.shiftKey && props.canLink) {
    drawingLink = { from: n, x: sx, y: sy }
    return
  }
  drag = { node: n, sx, sy, vx: view.x, vy: view.y, moved: false }
  if (n) { n.fx = n.x; n.fy = n.y; sim?.alphaTarget(0.2).restart() }
}
function onPointerMove(e: PointerEvent) {
  const [sx, sy] = pos(e)
  if (drawingLink) { drawingLink.x = sx; drawingLink.y = sy; hover = nodeAt(sx, sy); schedule(); return }
  if (drag) {
    if (Math.abs(sx - drag.sx) + Math.abs(sy - drag.sy) > 3) drag.moved = true
    if (drag.node) {
      const [x, y] = toWorld(sx, sy)
      drag.node.fx = x
      drag.node.fy = y
    } else {
      view.x = drag.vx + (sx - drag.sx)
      view.y = drag.vy + (sy - drag.sy)
    }
    schedule()
    return
  }
  const n = nodeAt(sx, sy)
  if (n !== hover) { hover = n; canvas.value!.style.cursor = n ? 'pointer' : 'grab'; schedule() }
}
function onPointerUp(e: PointerEvent) {
  const [sx, sy] = pos(e)
  if (drawingLink) {
    const target = nodeAt(sx, sy)
    if (target && target.id !== drawingLink.from.id && target.kind !== 'board' && drawingLink.from.kind !== 'board') emit('link', drawingLink.from, target)
    drawingLink = null
    schedule()
    return
  }
  if (!drag) return
  const { node, moved } = drag
  if (node) { node.fx = null; node.fy = null; sim?.alphaTarget(0) }
  if (!moved) {
    if (node && (e.ctrlKey || e.metaKey || e.button === 1)) emit('open', node, true)
    else emit('select', node)
  }
  drag = null
}
function onDblClick(e: MouseEvent) {
  const [sx, sy] = pos(e)
  const n = nodeAt(sx, sy)
  if (n) emit('open', n, false)
}
function onWheel(e: WheelEvent) {
  e.preventDefault()
  const [sx, sy] = pos(e)
  const [wx, wy] = toWorld(sx, sy)
  const k = Math.min(6, Math.max(0.1, view.k * Math.exp(-e.deltaY * 0.0015)))
  view.k = k
  view.x = sx - width / 2 - wx * k
  view.y = sy - height / 2 - wy * k
  schedule()
}

/** Centre la vue sur un nœud. */
function centerOn(id: string, zoom = 1.6) {
  const n = nodes.find((x) => x.id === id)
  if (!n || n.x == null) return
  view.k = zoom
  view.x = -n.x * zoom
  view.y = -n.y! * zoom
  schedule()
}
/** Ajuste le zoom pour tout voir. */
function fit() {
  if (!nodes.length) return
  const xs = nodes.map((n) => n.x ?? 0), ys = nodes.map((n) => n.y ?? 0)
  const minX = Math.min(...xs), maxX = Math.max(...xs), minY = Math.min(...ys), maxY = Math.max(...ys)
  const k = Math.min(2, 0.9 * Math.min(width / Math.max(1, maxX - minX), height / Math.max(1, maxY - minY)))
  view.k = k
  view.x = -((minX + maxX) / 2) * k
  view.y = -((minY + maxY) / 2) * k
  schedule()
}
defineExpose({ centerOn, fit })
// Tests de bout en bout : positions à l'écran (actif seulement si la page le demande).
if (typeof window !== 'undefined' && (window as any).__lootyGraphTest) {
  ;(window as any).__lootyGraphNodes = () => {
    const r = canvas.value?.getBoundingClientRect()
    return nodes.map((n) => { const [x, y] = toScreen(n.x ?? 0, n.y ?? 0); return { id: n.id, label: n.label, x: x + (r?.left ?? 0), y: y + (r?.top ?? 0) } })
  }
}

function resize() {
  if (!wrap.value || !canvas.value) return
  dpr = window.devicePixelRatio || 1
  width = wrap.value.clientWidth
  height = wrap.value.clientHeight
  canvas.value.width = width * dpr
  canvas.value.height = height * dpr
  canvas.value.style.width = `${width}px`
  canvas.value.style.height = `${height}px`
  schedule()
}

let ro: ResizeObserver | null = null
let themeObs: MutationObserver | null = null
onMounted(() => {
  ctx = canvas.value!.getContext('2d')
  readColors()
  resize()
  ro = new ResizeObserver(resize)
  ro.observe(wrap.value!)
  themeObs = new MutationObserver(() => { readColors(); schedule() })
  themeObs.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
  rebuild()
  // Premier cadrage une fois la disposition stabilisée.
  setTimeout(fit, 900)
})
onBeforeUnmount(() => { sim?.stop(); ro?.disconnect(); themeObs?.disconnect(); cancelAnimationFrame(raf) })
watch(() => [props.nodes, props.edges], rebuild)
watch(() => [props.selected, props.search], schedule)
</script>

<template>
  <div ref="wrap" class="fg">
    <canvas
      ref="canvas" role="img" :aria-label="$attrs['aria-label'] as string"
      @pointerdown="onPointerDown" @pointermove="onPointerMove" @pointerup="onPointerUp" @pointerleave="hover = null; schedule()"
      @dblclick="onDblClick" @wheel="onWheel" @auxclick.prevent @contextmenu.prevent
    />
  </div>
</template>

<style scoped>
.fg { position: relative; width: 100%; height: 100%; overflow: hidden; touch-action: none; }
canvas { display: block; cursor: grab; }
</style>
