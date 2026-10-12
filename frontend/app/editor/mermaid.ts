/**
 * Mermaid chargé à la demande, avec la mise en page ELK pour les organigrammes
 * (équivalent de `%%{init: {"flowchart": {"defaultRenderer": "elk"}}}%%` sur chaque
 * diagramme ; une directive écrite dans le diagramme garde la priorité).
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

export async function renderMermaid(id: string, code: string, dark: boolean) {
  const { mermaid, elk } = await loadMermaid()
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: 'strict',
    theme: dark ? 'dark' : 'neutral',
    fontFamily: 'Inter, sans-serif',
    ...(elk ? { flowchart: { defaultRenderer: 'elk' } } : {}),
  })
  return mermaid.render(id, code)
}
