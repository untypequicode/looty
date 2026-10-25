import TurndownService from 'turndown'

/** HTML de l'éditeur -> Markdown (formules, cases à cocher, encadrés, toggles). */
export function htmlToMarkdown(html: string): string {
  const td = new TurndownService({
    headingStyle: 'atx', codeBlockStyle: 'fenced', bulletListMarker: '-',
    // Les nœuds sans texte (formules, dessins) sont « vides » pour Turndown : on applique quand même leur règle.
    blankReplacement: (_c, n) => {
      const el = n as HTMLElement
      if (el.nodeName === 'TH' || el.nodeName === 'TD') return '  |'
      if (el.dataset?.type === 'math-block') return `\n\n$$\n${el.getAttribute('latex') ?? ''}\n$$\n\n`
      if (el.dataset?.type === 'math-inline') return `$${el.getAttribute('latex') ?? ''}$`
      if (el.dataset?.type === 'excalidraw') return '\n\n_(Excalidraw)_\n\n'
      return (n as any).isBlock ? '\n\n' : ''
    },
  })
  td.addRule('mathBlock', {
    filter: (n) => n.nodeName === 'DIV' && (n as HTMLElement).dataset.type === 'math-block',
    replacement: (_c, n) => `\n\n$$\n${(n as HTMLElement).getAttribute('latex') ?? ''}\n$$\n\n`,
  })
  td.addRule('mathInline', {
    filter: (n) => n.nodeName === 'SPAN' && (n as HTMLElement).dataset.type === 'math-inline',
    replacement: (_c, n) => `$${(n as HTMLElement).getAttribute('latex') ?? ''}$`,
  })
  td.addRule('taskItem', {
    filter: (n) => n.nodeName === 'LI' && (n as HTMLElement).dataset.type === 'taskItem',
    replacement: (c, n) => `- [${(n as HTMLElement).dataset.checked === 'true' ? 'x' : ' '}] ${c.trim()}\n`,
  })
  td.addRule('callout', {
    filter: (n) => n.nodeName === 'BLOCKQUOTE' && (n as HTMLElement).dataset.type === 'notice',
    replacement: (c, n) => `\n\n:::${(n as HTMLElement).dataset.noticeType ?? 'info'}\n${c.trim()}\n:::\n\n`,
  })
  td.addRule('toggle', {
    filter: (n) => n.nodeName === 'DETAILS',
    replacement: (c, n) => {
      const summary = (n as HTMLElement).querySelector('summary')?.textContent ?? ''
      return `\n\n<details>\n<summary>${summary}</summary>\n\n${c.replace(summary, '').trim()}\n\n</details>\n\n`
    },
  })
  td.addRule('mermaid', {
    filter: (n) => n.nodeName === 'DIV' && (n as HTMLElement).dataset.type === 'mermaid',
    replacement: (_c, n) => `\n\n\`\`\`mermaid\n${(n as HTMLElement).textContent ?? ''}\n\`\`\`\n\n`,
  })
  td.addRule('embedLike', {
    filter: (n) => n.nodeName === 'DIV' && ['embed', 'pdf'].includes((n as HTMLElement).dataset.type ?? ''),
    replacement: (_c, n) => { const el = n as HTMLElement; const src = el.dataset.src ?? ''; return `\n\n[${el.dataset.name || src}](${src})\n\n` },
  })
  td.addRule('excalidraw', {
    filter: (n) => n.nodeName === 'DIV' && (n as HTMLElement).dataset.type === 'excalidraw',
    replacement: () => '\n\n_(Excalidraw)_\n\n',
  })
  // Tableaux GFM (cellules sur une ligne)
  const cell = (c: string) => c.trim().replace(/\n+/g, ' ').replace(/\|/g, '\\|')
  td.addRule('tableCell', { filter: ['th', 'td'], replacement: (c) => ` ${cell(c)} |` })
  td.addRule('tableRow', {
    filter: 'tr',
    replacement: (c, n) => {
      const row = `|${c}\n`
      const parent = n.parentNode as Element | null
      const first = parent?.firstElementChild === n && (parent?.nodeName !== 'TBODY' || !parent.previousElementSibling || parent.previousElementSibling.nodeName === 'COLGROUP')
      return first ? `${row}|${' --- |'.repeat((n as HTMLElement).children.length)}\n` : row
    },
  })
  td.addRule('table', { filter: 'table', replacement: (c) => `\n\n${c.trim()}\n\n` })
  td.addRule('tableSection', { filter: ['thead', 'tbody', 'tfoot', 'colgroup', 'col'], replacement: (c) => c })
  td.keep(['mark', 'u'])
  // Les formules n'ont pas de texte : on y place le LaTeX pour que Turndown ne les traite pas comme vides.
  const doc = new DOMParser().parseFromString(`<body>${html || ''}</body>`, 'text/html')
  for (const el of Array.from(doc.querySelectorAll('[data-type="math-inline"], [data-type="math-block"]'))) el.textContent = el.getAttribute('latex') ?? ''
  return td.turndown(doc.body)
}

function download(blob: Blob, name: string) {
  const a = document.createElement('a')
  a.href = URL.createObjectURL(blob)
  a.download = name
  a.click()
  setTimeout(() => URL.revokeObjectURL(a.href), 1000)
}
const safe = (s: string) => (s || 'Untitled').replace(/[/\\?%*:|"<>]/g, '-').slice(0, 120)

export function exportMarkdown(title: string, html: string) {
  download(new Blob([`# ${title}\n\n${htmlToMarkdown(html)}`], { type: 'text/markdown;charset=utf-8' }), `${safe(title)}.md`)
}
