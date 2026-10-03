import TurndownService from 'turndown'
import JSZip from 'jszip'
import type { Page } from '~/types'
import { api } from '~/composables/api'

/** HTML de l'éditeur -> Markdown (formules, cases à cocher, encadrés, toggles). */
export function htmlToMarkdown(html: string): string {
  const td = new TurndownService({ headingStyle: 'atx', codeBlockStyle: 'fenced', bulletListMarker: '-' })
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
  td.keep(['mark', 'u'])
  return td.turndown(html || '')
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

/** Export ZIP : la page, ses sous-pages (récursif) et les images importées. */
export async function exportZip(wsId: string, root: Page, all: Page[], currentHtml: string) {
  const zip = new JSZip()
  const fetched = new Map<string, string>()
  async function add(page: Page, dir: string, html?: string) {
    const content = html ?? (await api<Page>(`/api/workspaces/${wsId}/pages/${page.id}`)).content ?? ''
    let md = `# ${page.title}\n\n${htmlToMarkdown(content)}`
    for (const m of md.matchAll(/!\[[^\]]*\]\((\/uploads\/[^)\s]+)\)/g)) {
      const url = m[1]
      if (!fetched.has(url)) {
        const res = await fetch(url)
        if (res.ok) {
          const name = url.split('/').pop()!
          zip.file(`assets/${name}`, await res.blob())
          fetched.set(url, name)
        }
      }
      const name = fetched.get(url)
      if (name) md = md.split(url).join(`${'../'.repeat(dir.split('/').filter(Boolean).length)}assets/${name}`)
    }
    zip.file(`${dir}${safe(page.title)}.md`, md)
    for (const child of all.filter((p) => p.parent_id === page.id)) await add(child, `${dir}${safe(page.title)}/`)
  }
  await add(root, '', currentHtml)
  download(await zip.generateAsync({ type: 'blob' }), `${safe(root.title)}.zip`)
}
