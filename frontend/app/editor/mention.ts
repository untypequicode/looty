import Mention from '@tiptap/extension-mention'
import type { WorkspaceMember } from '~/types'
import { suggestionRenderer } from './suggestion'
import { pageItems, pageMentionContent, type PageSource } from './pageMention'
import { translate as t } from '~/composables/useI18n'

/**
 * Mentions @membre. Le rendu HTML (`span.mention[data-id]`) est conservé côté
 * serveur par le nettoyage des commentaires, qui en déduit les notifications.
 */
export function mentionExtension(getMembers: () => WorkspaceMember[], onMention?: (userId: string) => void, pages?: PageSource) {
  return Mention.configure({
    HTMLAttributes: { class: 'mention' },
    renderLabel: ({ node }) => `@${node.attrs.label ?? node.attrs.id}`,
    suggestion: {
      items: ({ query }) => {
        const q = query.toLowerCase()
        const people = getMembers()
          .filter((m) => m.name.toLowerCase().includes(q) || m.email.toLowerCase().includes(q))
          .slice(0, 8)
          .map((m) => ({ id: m.user_id, label: m.name, hint: m.email, avatar: { name: m.name, src: m.avatar_url }, group: pages ? t('pageMention.people') : undefined }))
        // Pages ensuite (mêmes mentions que `[[`).
        return pages && q ? [...people, ...pageItems(pages, q, 5)] : people
      },
      command: ({ editor, range, props }) => {
        if (props.kind === 'page' && pages) {
          editor.chain().focus().insertContentAt(range, [pageMentionContent(pages, props as any), { type: 'text', text: ' ' }]).run()
          return
        }
        // Espace après la mention, comme dans Linear / Outline
        editor
          .chain()
          .focus()
          .insertContentAt(range, [
            { type: 'mention', attrs: { id: props.id, label: props.label } },
            { type: 'text', text: ' ' },
          ])
          .run()
        onMention?.(props.id as string)
      },
      render: suggestionRenderer(),
    },
  })
}
