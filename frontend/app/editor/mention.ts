import Mention from '@tiptap/extension-mention'
import type { WorkspaceMember } from '~/types'
import { suggestionRenderer } from './suggestion'

/**
 * Mentions @membre. Le rendu HTML (`span.mention[data-id]`) est conservé côté
 * serveur par le nettoyage des commentaires, qui en déduit les notifications.
 */
export function mentionExtension(getMembers: () => WorkspaceMember[], onMention?: (userId: string) => void) {
  return Mention.configure({
    HTMLAttributes: { class: 'mention' },
    renderLabel: ({ node }) => `@${node.attrs.label ?? node.attrs.id}`,
    suggestion: {
      items: ({ query }) => {
        const q = query.toLowerCase()
        return getMembers()
          .filter((m) => m.name.toLowerCase().includes(q) || m.email.toLowerCase().includes(q))
          .slice(0, 8)
          .map((m) => ({ id: m.user_id, label: m.name, hint: m.email, avatar: { name: m.name, src: m.avatar_url } }))
      },
      command: ({ editor, range, props }) => {
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
