/**
 * Format texte d'un bloc graphe : une ligne par nœud ou par arc, lisible et modifiable à la
 * main. L'édition visuelle ne réécrit que les lignes concernées : en collaboration, deux
 * personnes qui déplacent des nœuds différents ne se gênent pas.
 *
 *   # commentaire
 *   s : Source @ 0 120        nœud « s », étiquette « Source », position (0, 120)
 *   a                         nœud sans étiquette ni position (placé automatiquement)
 *   s -> a : 3/5              arc orienté de s vers a, étiquette (poids, flot…) « 3/5 »
 *   a -- b : 2                arête non orientée
 *   a -> t *                  `*` en fin de ligne : élément mis en avant (chemin, coupe…)
 *
 * Un nœud cité par un arc existe même sans ligne à lui.
 */

export interface GraphNode {
  id: string
  label: string
  x?: number
  y?: number
  hl: boolean
  /** Ligne qui déclare le nœud (-1 : seulement cité par des arcs). */
  line: number
}

export interface GraphEdge {
  /** Identifiant stable tant que l'arc existe : extrémités + rang parmi les arcs parallèles. */
  key: string
  from: string
  to: string
  directed: boolean
  label: string
  hl: boolean
  line: number
}

export interface Graph {
  nodes: GraphNode[]
  edges: GraphEdge[]
  errors: { line: number; text: string }[]
}

const ID = String.raw`[\p{L}\p{N}_']+`
const NUM = String.raw`-?\d+(?:\.\d+)?`
const EDGE_RE = new RegExp(String.raw`^(${ID})\s*(->|<-|--)\s*(${ID})(?:\s*:\s*(.*?))?\s*(\*)?$`, 'u')
const NODE_RE = new RegExp(String.raw`^(${ID})(?:\s*:\s*(.*?))?(?:\s*@\s*(${NUM})[\s,]+(${NUM}))?\s*(\*)?$`, 'u')
const ID_RE = new RegExp(`^${ID}$`, 'u')

export const isNodeId = (s: string) => ID_RE.test(s)
const skip = (l: string) => !l || l.startsWith('#') || l.startsWith('%%')

export function parseGraph(text: string): Graph {
  const nodes = new Map<string, GraphNode>()
  const edges: GraphEdge[] = []
  const errors: Graph['errors'] = []
  const seen = new Map<string, number>()
  const lines = text.split('\n')
  const ensure = (id: string) => {
    if (!nodes.has(id)) nodes.set(id, { id, label: id, hl: false, line: -1 })
    return nodes.get(id)!
  }
  lines.forEach((raw, line) => {
    const l = raw.trim()
    if (skip(l)) return
    let m = l.match(EDGE_RE)
    if (m) {
      let [from, op, to] = [m[1]!, m[2]!, m[3]!]
      if (op === '<-') [from, to] = [to, from]
      ensure(from); ensure(to)
      const directed = op !== '--'
      // Arête non orientée : même clé dans les deux sens.
      const pair = directed ? `${from}>${to}` : [from, to].sort().join('-')
      const n = seen.get(pair) ?? 0
      seen.set(pair, n + 1)
      edges.push({ key: `${pair}#${n}`, from, to, directed, label: (m[4] ?? '').trim(), hl: !!m[5], line })
      return
    }
    m = l.match(NODE_RE)
    if (m) {
      const node = ensure(m[1]!)
      if (node.line < 0) node.line = line
      if (m[2] != null && m[2].trim()) node.label = m[2].trim()
      if (m[3] != null) { node.x = Number(m[3]); node.y = Number(m[4]) }
      node.hl = !!m[5]
      return
    }
    errors.push({ line, text: l })
  })
  return { nodes: [...nodes.values()], edges, errors }
}

const clean = (s: string) => s.replace(/\s+/g, ' ').trim()

export function formatNode(n: { id: string; label?: string; x?: number; y?: number; hl?: boolean }): string {
  const label = n.label != null ? clean(n.label) : ''
  let s = n.id
  if (label && label !== n.id) s += ` : ${label}`
  if (n.x != null && n.y != null) s += ` @ ${Math.round(n.x)} ${Math.round(n.y)}`
  if (n.hl) s += ' *'
  return s
}

export function formatEdge(e: { from: string; to: string; directed: boolean; label?: string; hl?: boolean }): string {
  const label = e.label ? clean(e.label) : ''
  return `${e.from} ${e.directed ? '->' : '--'} ${e.to}${label ? ` : ${label}` : ''}${e.hl ? ' *' : ''}`
}

// --- Modifications (texte → texte) -------------------------------------------

type Lines = string[]
const split = (text: string): Lines => (text ? text.split('\n') : [])
const join = (lines: Lines) => lines.join('\n')

/** Ligne où déclarer un nouveau nœud : après le dernier nœud déclaré, sinon avant les arcs. */
function nodeInsertAt(g: Graph, lines: Lines): number {
  const last = Math.max(-1, ...g.nodes.map((n) => n.line))
  if (last >= 0) return last + 1
  const firstEdge = g.edges.length ? Math.min(...g.edges.map((e) => e.line)) : lines.length
  return firstEdge
}

/** Met à jour (ou crée) la ligne de déclaration de plusieurs nœuds. */
export function updateNodes(text: string, changes: Record<string, Partial<Pick<GraphNode, 'label' | 'x' | 'y' | 'hl'>>>): string {
  const g = parseGraph(text)
  const lines = split(text)
  const added: string[] = []
  for (const [id, ch] of Object.entries(changes)) {
    const n = g.nodes.find((x) => x.id === id) ?? { id, label: id, hl: false, line: -1 }
    const next = formatNode({ ...n, ...ch })
    if (n.line >= 0) lines[n.line] = lines[n.line]!.match(/^\s*/)![0] + next
    else added.push(next)
  }
  if (added.length) lines.splice(nodeInsertAt(g, lines), 0, ...added)
  return join(lines)
}

export function addNode(text: string, node: { id: string; label?: string; x?: number; y?: number }): string {
  return updateNodes(text, { [node.id]: { label: node.label ?? node.id, x: node.x, y: node.y } })
}

/** Supprime des nœuds (avec leurs arcs) et des arcs (par clé). */
export function removeElements(text: string, nodeIds: string[], edgeKeys: string[]): string {
  const g = parseGraph(text)
  const ids = new Set(nodeIds)
  const drop = new Set<number>()
  for (const n of g.nodes) if (ids.has(n.id) && n.line >= 0) drop.add(n.line)
  for (const e of g.edges) if (ids.has(e.from) || ids.has(e.to) || edgeKeys.includes(e.key)) drop.add(e.line)
  return join(split(text).filter((_, i) => !drop.has(i)))
}

export function addEdge(text: string, edge: { from: string; to: string; directed: boolean; label?: string }): string {
  const lines = split(text)
  while (lines.length && !lines[lines.length - 1]!.trim()) lines.pop()
  lines.push(formatEdge(edge))
  return join(lines)
}

export function updateEdge(text: string, key: string, ch: Partial<Pick<GraphEdge, 'label' | 'hl' | 'directed'>>): string {
  const e = parseGraph(text).edges.find((x) => x.key === key)
  if (!e) return text
  const lines = split(text)
  lines[e.line] = lines[e.line]!.match(/^\s*/)![0] + formatEdge({ ...e, ...ch })
  return join(lines)
}

/** Rend tous les arcs orientés, ou tous non orientés. */
export function setDirected(text: string, directed: boolean): string {
  const g = parseGraph(text)
  const lines = split(text)
  for (const e of g.edges) if (e.directed !== directed) lines[e.line] = lines[e.line]!.match(/^\s*/)![0] + formatEdge({ ...e, directed })
  return join(lines)
}

/** Premier identifiant libre : A…Z, puis A1, B1… */
export function nextNodeId(g: Graph): string {
  const used = new Set(g.nodes.map((n) => n.id))
  for (let round = 0; ; round++) {
    for (let c = 65; c <= 90; c++) {
      const id = String.fromCharCode(c) + (round ? String(round) : '')
      if (!used.has(id)) return id
    }
  }
}

/** Exemple inséré par le menu « / » : petit réseau de flot. */
export const GRAPH_SAMPLE = [
  's : Source @ 0 100',
  'a @ 160 20',
  'b @ 160 180',
  't : Puits @ 320 100',
  's -> a : 3/4',
  's -> b : 2/2',
  'a -> b : 1/1',
  'a -> t : 2/3',
  'b -> t : 3/5',
].join('\n')
