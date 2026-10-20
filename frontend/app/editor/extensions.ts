import StarterKit from '@tiptap/starter-kit'
import Highlight from '@tiptap/extension-highlight'
import Typography from '@tiptap/extension-typography'
import { TaskList } from '@tiptap/extension-task-list'
import { TaskItem } from '@tiptap/extension-task-item'
import { Table, TableRow, TableCell, TableHeader } from '@tiptap/extension-table'
import { Placeholder, TrailingNode } from '@tiptap/extensions'
import Mention from '@tiptap/extension-mention'
import { Collaboration } from '@tiptap/extension-collaboration'
import { CollaborationCaret } from '@tiptap/extension-collaboration-caret'
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
import { PageMention, pageMentionSuggest, type PageSource } from './pageMention'
import { CommentMark } from './commentMark'
import { embedExtensions, EmbedPaste } from './embeds'
import { CollaborativeUndo } from './undo'
import { StablePluginState } from './pluginState'
import { SOURCE_NODES } from './source'
import { translate as t } from '~/composables/useI18n'
import type { MemberGroup } from '~/composables/useGroups'

/** Schéma commun : identique en édition et en lecture pour un rendu fidèle. */
function schemaExtensions(opts: { collaborative?: boolean } = {}): Extensions {
  return [
    StarterKit.configure({
      codeBlock: false,
      undoRedo: opts.collaborative ? false : undefined,
      dropcursor: { color: 'var(--accent)', width: 2 },
      heading: { levels: [1, 2, 3, 4] },
      // La fin de document est gérée par l'éditeur (pas en lecture seule).
      trailingNode: false,
      link: {
        openOnClick: false,
        autolink: true,
        linkOnPaste: true,
        HTMLAttributes: { rel: 'noopener noreferrer nofollow' },
        shouldAutoLink: (href) => /^(https?:\/\/|mailto:|\/|#)/i.test(href),
      },
    }),
    CodeBlock,
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
    PageMention,
    CommentMark,
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
  /** Groupes de membres (mentions @groupe). */
  getGroups?: () => MemberGroup[]
  onMention?: (userId: string) => void
  slash?: SlashContext
  /** Pages de l'espace : mentions `[[` et `@`. */
  pages?: PageSource
}

/** Extensions de l'éditeur de documents (collaboratif). */
export function docExtensions(o: DocEditorOptions): Extensions {
  const exts: Extensions = [
    ...schemaExtensions({ collaborative: !!o.ydoc }),
    Typography,
    Placeholder.configure({
      includeChildren: true,
      placeholder: ({ node, editor }) => {
        // Seuls les blocs de texte affichent une invite : un encadré, une liste ou une cellule
        // vides contiennent eux-mêmes un paragraphe vide (sinon deux textes superposés).
        if (!node.isTextblock || SOURCE_NODES[node.type.name]) return ''
        if (node.type.name === 'heading') return t('editor.headingPlaceholder', { level: node.attrs.level })
        if (node.type.name === 'toggleSummary') return t('editor.togglePlaceholder')
        if (editor.isEmpty) return t('editor.placeholder')
        return t('editor.emptyLine')
      },
    }),
    mentionExtension(o.getMembers, o.onMention, o.pages, o.getGroups),
    ...(o.pages ? [pageMentionSuggest(o.pages)] : []),
    slashCommands(o.slash),
    EmojiSuggestion,
    FindInPage,
    BlockKeymap,
    EmbedPaste,
    TrailingNode.configure({ node: 'paragraph', notAfter: ['paragraph'] }),
  ]
  if (o.ydoc) {
    exts.push(Collaboration.configure({ document: o.ydoc }), CollaborativeUndo, StablePluginState)
    if (o.provider) exts.push(CollaborationCaret.configure({ provider: o.provider, user: o.user ?? { name: '?', color: '#7170ff' } }))
  }
  return exts
}

/** Lecture seule (pages publiques, commentaires). */
export function readonlyExtensions(): Extensions {
  return [...schemaExtensions(), Mention.configure({ HTMLAttributes: { class: 'mention' } })]
}

/** Aperçu d'une version enregistrée (état Yjs) : lecture seule, sans fournisseur réseau. */
export function revisionExtensions(ydoc: Y.Doc): Extensions {
  return [...schemaExtensions({ collaborative: true }), Mention.configure({ HTMLAttributes: { class: 'mention' } }), Collaboration.configure({ document: ydoc }), StablePluginState]
}
