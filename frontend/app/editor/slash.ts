import { Extension, type Editor, type Range } from '@tiptap/core'
import Suggestion from '@tiptap/suggestion'
import { PluginKey } from '@tiptap/pm/state'
import type { Component } from 'vue'
import {
  Pilcrow, Heading1, Heading2, Heading3, List, ListOrdered, ListChecks, Quote, Code2, Sigma, Table, Image, Minus,
  Info, AlertTriangle, CheckCircle2, Lightbulb, ChevronRightSquare, Paperclip, CalendarDays, FileText, Superscript, Workflow, PenTool, FileType2, Clapperboard, Kanban,
} from 'lucide-vue-next'
import { suggestionRenderer } from './suggestion'
import { translate as t } from '~/composables/useI18n'
import { uploadFile } from '~/composables/api'

export interface SlashItem {
  id: string
  label: string
  hint?: string
  icon: Component
  group: string
  keywords?: string
  shortcut?: string
  run: (editor: Editor, range: Range) => void
}

function pickFile(accept: string, onFile: (f: File) => void) {
  const input = document.createElement('input')
  input.type = 'file'
  input.accept = accept
  input.onchange = () => { const f = input.files?.[0]; if (f) onFile(f) }
  input.click()
}

export interface SlashContext {
  /** Recherche de documents pour l'insertion de lien interne. */
  pickDoc?: (editor: Editor) => void
  /** Demande une URL à intégrer (YouTube, Figma...). */
  askEmbed?: (editor: Editor) => void
}

export function slashItems(ctx: SlashContext = {}): SlashItem[] {
  const basic = t('editor.slash.basic')
  const blocks = t('editor.slash.blocks')
  const media = t('editor.slash.media')
  const adv = t('editor.slash.advanced')
  const emb = t('editor.slash.embeds')
  const chain = (e: Editor, r: Range) => e.chain().focus().deleteRange(r)
  return [
    { id: 'p', group: basic, label: t('editor.slash.text'), icon: Pilcrow, keywords: 'paragraph texte', run: (e, r) => chain(e, r).setParagraph().run() },
    { id: 'h1', group: basic, label: t('editor.slash.h1'), icon: Heading1, shortcut: '#', keywords: 'heading titre title', run: (e, r) => chain(e, r).setHeading({ level: 1 }).run() },
    { id: 'h2', group: basic, label: t('editor.slash.h2'), icon: Heading2, shortcut: '##', keywords: 'heading titre subtitle', run: (e, r) => chain(e, r).setHeading({ level: 2 }).run() },
    { id: 'h3', group: basic, label: t('editor.slash.h3'), icon: Heading3, shortcut: '###', keywords: 'heading titre', run: (e, r) => chain(e, r).setHeading({ level: 3 }).run() },
    { id: 'ul', group: basic, label: t('editor.slash.bullet'), icon: List, shortcut: '-', keywords: 'bullet liste puces unordered', run: (e, r) => chain(e, r).toggleBulletList().run() },
    { id: 'ol', group: basic, label: t('editor.slash.ordered'), icon: ListOrdered, shortcut: '1.', keywords: 'numbered liste numérotée ordered', run: (e, r) => chain(e, r).toggleOrderedList().run() },
    { id: 'todo', group: basic, label: t('editor.slash.todo'), icon: ListChecks, shortcut: '[]', keywords: 'todo task checkbox tâche case', run: (e, r) => chain(e, r).toggleTaskList().run() },

    { id: 'toggle', group: blocks, label: t('editor.slash.toggle'), hint: t('editor.slash.toggleHint'), icon: ChevronRightSquare, keywords: 'toggle details dépliant accordion', run: (e, r) => chain(e, r).setToggle().run() },
    { id: 'quote', group: blocks, label: t('editor.slash.quote'), icon: Quote, shortcut: '>', keywords: 'quote citation blockquote', run: (e, r) => chain(e, r).toggleBlockquote().run() },
    { id: 'info', group: blocks, label: t('editor.slash.calloutInfo'), icon: Info, shortcut: ':::info', keywords: 'callout notice encadré info', run: (e, r) => chain(e, r).setCallout({ type: 'info' }).run() },
    { id: 'warning', group: blocks, label: t('editor.slash.calloutWarning'), icon: AlertTriangle, keywords: 'callout warning attention avertissement', run: (e, r) => chain(e, r).setCallout({ type: 'warning' }).run() },
    { id: 'success', group: blocks, label: t('editor.slash.calloutSuccess'), icon: CheckCircle2, keywords: 'callout success succès', run: (e, r) => chain(e, r).setCallout({ type: 'success' }).run() },
    { id: 'tip', group: blocks, label: t('editor.slash.calloutTip'), icon: Lightbulb, keywords: 'callout tip astuce conseil', run: (e, r) => chain(e, r).setCallout({ type: 'tip' }).run() },
    { id: 'code', group: blocks, label: t('editor.slash.code'), icon: Code2, shortcut: '```', keywords: 'code snippet bloc', run: (e, r) => chain(e, r).toggleCodeBlock().run() },
    { id: 'table', group: blocks, label: t('editor.slash.table'), icon: Table, keywords: 'table tableau grid', run: (e, r) => chain(e, r).insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run() },
    { id: 'hr', group: blocks, label: t('editor.slash.divider'), icon: Minus, shortcut: '---', keywords: 'divider séparateur hr rule', run: (e, r) => chain(e, r).setHorizontalRule().run() },

    { id: 'image', group: media, label: t('editor.slash.image'), hint: t('editor.slash.imageHint'), icon: Image, keywords: 'image photo picture upload', run: (e, r) => {
      chain(e, r).run()
      pickFile('image/*', async (f) => { const u = await uploadFile(f); e.chain().focus().setImage({ src: u.url, alt: u.name }).run() })
    } },
    { id: 'file', group: media, label: t('editor.slash.file'), hint: t('editor.slash.fileHint'), icon: Paperclip, keywords: 'file fichier attachment pièce jointe pdf', run: (e, r) => {
      chain(e, r).run()
      pickFile('*/*', async (f) => {
        const u = await uploadFile(f)
        if (u.mime === 'application/pdf') return e.chain().focus().insertPdf(u.url, u.name).run()
        e.chain().focus().insertContent([{ type: 'text', text: `📎 ${u.name}`, marks: [{ type: 'link', attrs: { href: u.url } }] }, { type: 'text', text: ' ' }]).run()
      })
    } },

    { id: 'pdf', group: emb, label: t('editor.slash.pdf'), hint: t('editor.slash.pdfHint'), icon: FileType2, keywords: 'pdf document viewer lecteur', run: (e, r) => {
      chain(e, r).run()
      pickFile('application/pdf', async (f) => { const u = await uploadFile(f); e.chain().focus().insertPdf(u.url, u.name).run() })
    } },
    { id: 'mermaid', group: emb, label: t('editor.slash.mermaid'), hint: t('editor.slash.mermaidHint'), icon: Workflow, keywords: 'mermaid diagram diagramme flowchart graph sequence gantt', run: (e, r) => chain(e, r).insertMermaid('graph TD\n  A[Idée] --> B{Validée ?}\n  B -- Oui --> C[Réalisation]\n  B -- Non --> A').run() },
    { id: 'excalidraw', group: emb, label: t('editor.slash.excalidraw'), hint: t('editor.slash.excalidrawHint'), icon: PenTool, keywords: 'excalidraw drawing dessin schéma sketch whiteboard tableau blanc', run: (e, r) => chain(e, r).insertExcalidraw().run() },
    { id: 'board', group: emb, label: t('blocks.slashBoard'), hint: t('blocks.slashBoardHint'), icon: Kanban, keywords: 'board tableau kanban database base de données tâches tasks liste table', run: (e, r) => chain(e, r).insertBoardBlock().run() },
    { id: 'agenda', group: emb, label: t('blocks.slashAgenda'), hint: t('blocks.slashAgendaHint'), icon: CalendarDays, keywords: 'agenda calendrier calendar événements events échéances deadlines semaine week', run: (e, r) => chain(e, r).insertAgendaBlock().run() },
    ...(ctx.askEmbed ? [{ id: 'embed', group: emb, label: t('editor.slash.embed'), hint: 'YouTube, Figma, Loom…', icon: Clapperboard, keywords: 'embed intégrer youtube vimeo loom figma codepen google miro video vidéo', run: (e: Editor, r: Range) => { chain(e, r).run(); ctx.askEmbed!(e) } }] : []),

    { id: 'math', group: adv, label: t('editor.slash.math'), hint: '$$', icon: Sigma, keywords: 'math latex formula équation formule katex', run: (e, r) => chain(e, r).insertMathBlock('').run() },
    { id: 'mathi', group: adv, label: t('editor.slash.mathInline'), hint: '$x$', icon: Superscript, keywords: 'math inline latex formule', run: (e, r) => chain(e, r).insertMathInline('x').run() },
    { id: 'date', group: adv, label: t('editor.slash.date'), icon: CalendarDays, keywords: 'date today aujourd’hui', run: (e, r) => chain(e, r).insertContent(new Date().toLocaleDateString(document.documentElement.lang === 'fr' ? 'fr-FR' : 'en-US', { day: 'numeric', month: 'long', year: 'numeric' }) + ' ').run() },
    ...(ctx.pickDoc ? [{ id: 'doclink', group: adv, label: t('editor.slash.docLink'), icon: FileText, keywords: 'link lien doc page document', run: (e: Editor, r: Range) => { chain(e, r).run(); ctx.pickDoc!(e) } }] : []),
  ]
}

/** Menu « / » façon Outline / Notion. */
export function slashCommands(ctx: SlashContext = {}) {
  return Extension.create({
    name: 'slashCommands',
    addProseMirrorPlugins() {
      return [
        Suggestion({
          editor: this.editor,
          char: '/',
          pluginKey: new PluginKey('slash'),
          startOfLine: false,
          allow: ({ state, range }) => {
            // Pas dans un bloc de code
            const $from = state.doc.resolve(range.from)
            return $from.parent.type.name !== 'codeBlock'
          },
          items: ({ query }) => {
            const q = query.toLowerCase().trim()
            const all = slashItems(ctx)
            if (!q) return all
            return all.filter((i) => `${i.id} ${i.label} ${i.keywords ?? ''}`.toLowerCase().includes(q))
          },
          command: ({ editor, range, props }) => (props as unknown as SlashItem).run(editor, range),
          render: suggestionRenderer({ emptyLabel: t('editor.slash.empty') }),
        }),
      ]
    },
  })
}
