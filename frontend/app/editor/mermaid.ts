import { mermaidConfig, type DiagramStyle } from './diagramStyles'

/**
 * Mermaid chargé à la demande, avec la mise en page ELK (option `layout` de Mermaid 12 ;
 * une directive écrite dans le diagramme garde la priorité).
 */
type Mermaid = typeof import('mermaid').default
let ready: Promise<{ mermaid: Mermaid; elk: boolean }> | null = null

export function loadMermaid() {
  return (ready ??= (async () => {
    const mermaid = (await import('mermaid')).default
    let elk = false
    try {
      mermaid.registerLayoutLoaders((await import('@mermaid-js/layout-elk')).default)
      elk = true
    } catch {
      // Mise en page par défaut de Mermaid si ELK ne se charge pas.
    }
    return { mermaid, elk }
  })())
}

// La configuration de Mermaid est globale : un rendu à la fois, chacun avec son style.
let queue: Promise<unknown> = Promise.resolve()

export function renderMermaid(id: string, code: string, dark: boolean, style: DiagramStyle) {
  const job = queue.then(async () => {
    const { mermaid, elk } = await loadMermaid()
    mermaid.initialize({ startOnLoad: false, securityLevel: 'strict', ...mermaidConfig(style, dark, elk) } as any)
    return mermaid.render(id, code)
  })
  queue = job.catch(() => {})
  return job
}
