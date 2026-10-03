import StarterKit from '@tiptap/starter-kit'
import Link from '@tiptap/extension-link'
import Underline from '@tiptap/extension-underline'
import Highlight from '@tiptap/extension-highlight'
import Typography from '@tiptap/extension-typography'
import TaskList from '@tiptap/extension-task-list'
import TaskItem from '@tiptap/extension-task-item'
import Table from '@tiptap/extension-table'
import TableRow from '@tiptap/extension-table-row'
import TableCell from '@tiptap/extension-table-cell'
import TableHeader from '@tiptap/extension-table-header'
import Placeholder from '@tiptap/extension-placeholder'
import Mention from '@tiptap/extension-mention'
import { Collaboration } from '@tiptap/extension-collaboration'
import { CollaborationCursor } from '@tiptap/extension-collaboration-cursor'
import type { Extensions } from '@tiptap/core'
import type * as Y from 'yjs'
import type { HocuspocusProvider } from '@hocuspocus/provider'
import type { WorkspaceMember } from '~/types'
import { CodeBlock, MathBlock, MathInline, ResizableImage, Callout, Toggle, ToggleSummary, ToggleContent } from './nodes'
import { HeadingAnchors } from './headings'
import { MathScope } from './mathScope'
import { BlockKeymap } from './blocks'
import { FindInPage } from './search'
import { EmojiSuggestion } from './emoji'
import { slashCommands, type SlashContext } from './slash'
import { mentionExtension } from './mention'
import { TrailingNode } from './TrailingNode'
import { embedExtensions, EmbedPaste } from './embeds'
import { translate as t } from '~/composables/useI18n'

/** Schéma commun : identique en édition et en lecture pour un rendu fidèle. */
function schemaExtensions(opts: { collaborative?: boolean } = {}): Extensions {
  return [
    StarterKit.configure({
      codeBlock: false,
      history: opts.collaborative ? false : undefined,
      dropcursor: { color: 'var(--accent)', width: 2 },
      heading: { levels: [1, 2, 3, 4] },
    }),
    CodeBlock,
    Link.configure({
      openOnClick: false,
      autolink: true,
      linkOnPaste: true,
      HTMLAttributes: { rel: 'noopener noreferrer nofollow' },
      validate: (href) => /^(https?:\/\/|mailto:|\/|#)/i.test(href),
    }),
    Underline,
    Highlight,
    TaskList,
    TaskItem.configure({ nested: true }),
    Table.configure({ resizable: true, lastColumnResizable: false }),
    TableRow,
    TableHeader,
    TableCell,
    ResizableImage,
    MathBlock,
    MathInline,
    MathScope,
    Callout,
    Toggle,
    ToggleSummary,
    ToggleContent,
    HeadingAnchors,
    ...embedExtensions,
  ]
}

export interface DocEditorOptions {
  ydoc?: Y.Doc
  provider?: HocuspocusProvider
  user?: { name: string; color: string }
  getMembers: () => WorkspaceMember[]
  onMention?: (userId: string) => void
  slash?: SlashContext
}

/** Extensions de l'éditeur de documents (collaboratif). */
export function docExtensions(o: DocEditorOptions): Extensions {
  const exts: Extensions = [
    ...schemaExtensions({ collaborative: !!o.ydoc }),
    Typography,
    Placeholder.configure({
      includeChildren: true,
      placeholder: ({ node, editor }) => {
        if (node.type.name === 'heading') return t('editor.headingPlaceholder', { level: node.attrs.level })
        if (node.type.name === 'toggleSummary') return t('editor.togglePlaceholder')
        if (editor.isEmpty) return t('editor.placeholder')
        return t('editor.emptyLine')
      },
    }),
    mentionExtension(o.getMembers, o.onMention),
    slashCommands(o.slash),
    EmojiSuggestion,
    FindInPage,
    BlockKeymap,
    EmbedPaste,
    TrailingNode,
  ]
  if (o.ydoc) {
    exts.push(Collaboration.configure({ document: o.ydoc }))
    if (o.provider) exts.push(CollaborationCursor.configure({ provider: o.provider, user: o.user ?? { name: '?', color: '#7170ff' } }))
  }
  return exts
}

/** Lecture seule (pages publiques, commentaires). */
export function readonlyExtensions(): Extensions {
  return [...schemaExtensions(), Mention.configure({ HTMLAttributes: { class: 'mention' } })]
}
