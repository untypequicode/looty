import Mention from '@tiptap/extension-mention'
import { Users } from 'lucide-vue-next'
import type { WorkspaceMember } from '~/types'
import type { MemberGroup } from '~/composables/useGroups'
import { suggestionRenderer } from './suggestion'
import { pageItems, pageMentionContent, type PageSource } from './pageMention'
import { translate as t } from '~/composables/useI18n'
import { fold } from '~/utils/fold'

/**
 * Mentions @membre et @groupe. Le rendu HTML (`span.mention[data-id]`) est conservé côté
 * serveur par le nettoyage des commentaires, qui en déduit les notifications (pour un
 * groupe, l'identifiant est celui du groupe : chacun de ses membres est notifié).
 */
export function mentionExtension(getMembers: () => WorkspaceMember[], onMention?: (userId: string) => void, pages?: PageSource, getGroups?: () => MemberGroup[]) {
  return Mention.configure({
    HTMLAttributes: { class: 'mention' },
    renderLabel: ({ node }) => `@${node.attrs.label ?? node.attrs.id}`,
    suggestion: {
      items: ({ query }) => {
        const q = fold(query)
        const people = getMembers()
          .filter((m) => fold(m.name).includes(q) || m.email.toLowerCase().includes(q))
          .slice(0, 8)
          .map((m) => ({ id: m.user_id, label: m.name, hint: m.email, avatar: { name: m.name, src: m.avatar_url }, group: pages ? t('pageMention.people') : undefined }))
        const groups = (getGroups?.() ?? [])
          .filter((g) => g.members.length && fold(g.name).includes(q))
          .slice(0, 5)
          .map((g) => ({ id: g.id, label: g.name, hint: t('memberGroups.count', { count: g.members.length }), icon: Users, group: t('memberGroups.section') }))
        const named = groups.length ? [...people.map((p) => ({ ...p, group: t('pageMention.people') })), ...groups] : people
        // Pages ensuite (mêmes mentions que `[[`).
        return pages && q ? [...named, ...pageItems(pages, q, 5)] : named
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
