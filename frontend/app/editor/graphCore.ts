import type { Core, NodeSingular, Position } from 'cytoscape'
import { type DiagramStyle, type Palette, FONT_STACKS } from './diagramStyles'
import { parseGraph } from './graphSource'

/**
 * Cœur de dessin du bloc graphe, partagé par l'éditeur (`GraphView.vue`, interactif) et par le
 * rendu riche des réponses et des entraînements (image statique) : même feuille de style, mêmes
 * dispositions automatiques.
 */

const SHAPES = { rounded: 'round-rectangle', rect: 'rectangle', circle: 'ellipse' } as const

function nodeWidth(label: string, circle: boolean) {
  const w = Math.min(180, label.length * 8.2 + (circle ? 14 : 22))
  return Math.max(circle ? 38 : 44, w)
}

/** Feuille de style Cytoscape d'un style de diagramme, pour la palette claire ou sombre choisie. */
export function graphSheet(s: DiagramStyle, c: Palette): any[] {
  const font = FONT_STACKS[s.font]
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

export type LayoutKind = 'layers' | 'tree' | 'circle' | 'grid' | 'force'

/** Options de disposition automatique ; `roots` : nœuds sans arc entrant (couches, arbre). */
export function layoutOptions(kind: LayoutKind, roots: unknown): Record<string, any> {
  const r = roots && (roots as { length: number }).length ? roots : undefined
  const opts: Record<LayoutKind, any> = {
    // Couches de gauche à droite (réseaux de flot, DAG) : arbre en largeur, couché.
    layers: { name: 'breadthfirst', directed: true, roots: r, spacingFactor: 1.15, transform: (_n: unknown, p: Position) => ({ x: p.y, y: p.x }) },
    tree: { name: 'breadthfirst', directed: true, roots: r, spacingFactor: 1.1 },
    circle: { name: 'circle', spacingFactor: 1.1 },
    grid: { name: 'grid', avoidOverlap: true },
    force: { name: 'cose', animate: false, nodeRepulsion: () => 9000, idealEdgeLength: () => 90, randomize: true },
  }
  return opts[kind]
}

export const MAX_GRAPH_SOURCE = 20000

/**
 * Dessine un bloc graphe en image PNG (lecture seule) : Cytoscape dans un conteneur hors écran,
 * détruit aussitôt, pour ne garder aucun canevas vivant par bloc. Rejette si le graphe est vide
 * ou contient des lignes invalides.
 */
export async function renderGraphImage(source: string, dark: boolean, style: DiagramStyle): Promise<string> {
  const g = parseGraph(source)
  if (!g.nodes.length) throw new Error('empty')
  if (g.errors.length) throw new Error(`line ${g.errors[0]!.line + 1}`)
  if (source.length > MAX_GRAPH_SOURCE) throw new Error('too large')
  const cytoscape = (await import('cytoscape')).default
  const pal = dark ? style.dark : style.light
  const host = document.createElement('div')
  host.setAttribute('aria-hidden', 'true')
  host.style.cssText = 'position:fixed;left:-10000px;top:0;width:640px;height:340px;pointer-events:none'
  document.body.append(host)
  let cy: Core | null = null
  try {
    cy = cytoscape({
      container: host,
      style: graphSheet(style, pal),
      elements: [
        ...g.nodes.map((n) => ({ group: 'nodes' as const, data: { id: n.id, label: n.label }, position: { x: n.x ?? 0, y: n.y ?? 0 }, classes: n.hl ? 'hl' : '' })),
        ...g.edges.map((e) => ({ group: 'edges' as const, data: { id: `e:${e.key}`, source: e.from, target: e.to, label: e.label, directed: e.directed }, classes: e.hl ? 'hl' : '' })),
      ],
      userZoomingEnabled: false, userPanningEnabled: false, boxSelectionEnabled: false, autoungrabify: true,
    })
    // Positions partielles : tout est disposé, comme à l'ouverture d'un graphe sans coordonnées.
    if (g.nodes.some((n) => n.x == null)) {
      const roots = cy.nodes().filter((n) => n.incomers('edge[?directed]').empty())
      const kind: LayoutKind = g.edges.some((e) => e.directed) ? 'layers' : 'force'
      const layout = cy.layout({ ...layoutOptions(kind, roots), fit: false, padding: 28, animate: false } as any)
      await new Promise<void>((resolve) => { layout.one('layoutstop', () => resolve()); layout.run() })
    }
    cy.fit(undefined, 28)
    if (cy.zoom() > 1.25) { cy.zoom(1.25); cy.center() }
    return cy.png({ output: 'base64uri', bg: pal.background, full: true, scale: 2, maxWidth: 1600, maxHeight: 1200 })
  } finally {
    cy?.destroy()
    host.remove()
  }
}
