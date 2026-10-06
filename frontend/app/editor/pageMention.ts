import { Node, mergeAttributes, Extension } from '@tiptap/core'
import { VueNodeViewRenderer } from '@tiptap/vue-3'
import Suggestion from '@tiptap/suggestion'
import { PluginKey } from '@tiptap/pm/state'
import { FileText } from 'lucide-vue-next'
import type { Page } from '~/types'
import PageMentionView from '~/components/editor/PageMentionView.vue'
import { suggestionRenderer } from './suggestion'
import { translate as t } from '~/composables/useI18n'

/**
 * Mention d'une page (`[[` ou `@`) : lien vers le document, affiché avec son titre et son
 * icône actuels. L'attribut `href` contient « /doc/<id> », ce qui fait apparaître le lien
 * dans le graphe (contenu HTML comme état Yjs) sans traitement particulier côté serveur.
 */
export const PageMention = Node.create({
  name: 'pageMention',
  group: 'inline',
  inline: true,
  atom: true,
  selectable: true,
  addAttributes() {
    return {
      id: { default: null, parseHTML: (el) => el.getAttribute('data-page-id') },
      label: { default: '', parseHTML: (el) => el.textContent ?? '' },
      href: { default: null, parseHTML: (el) => el.getAttribute('href') },
    }
  },
  parseHTML() {
    return [{ tag: 'a[data-type="page-mention"]' }]
  },
  renderHTML({ node, HTMLAttributes }) {
    return ['a', mergeAttributes({ 'data-type': 'page-mention', 'data-page-id': node.attrs.id, href: node.attrs.href, class: 'page-mention' }, { class: HTMLAttributes.class }), node.attrs.label || t('docs.untitled')]
  },
  renderText({ node }) {
    return node.attrs.label || ''
  },
  addNodeView() {
    return VueNodeViewRenderer(PageMentionView as any)
  },
})

export interface PageSource {
  wsId: () => string
  pages: () => Page[]
}

/** Pages proposées pour une recherche (titre), les plus récentes d'abord à requête vide. */
export function pageItems(src: PageSource, query: string, limit = 8) {
  const q = query.toLowerCase().trim()
  return src.pages()
    .filter((p) => !q || (p.title || '').toLowerCase().includes(q))
    .sort((a, b) => (q ? (a.title || '').length - (b.title || '').length : +new Date(b.updated_at ?? 0) - +new Date(a.updated_at ?? 0)))
    .slice(0, limit)
    .map((p) => ({ id: p.id, label: p.title || t('docs.untitled'), emoji: p.icon && !p.icon.startsWith('http') ? p.icon : undefined, icon: p.icon ? undefined : FileText, kind: 'page', group: t('pageMention.pages') }))
}

export function pageMentionContent(src: PageSource, item: { id: string; label: string }) {
  return { type: 'pageMention', attrs: { id: item.id, label: item.label, href: `/w/${src.wsId()}/doc/${item.id}` } }
}

/** Déclencheur `[[` (comme Obsidian) : insère une mention de page. */
export function pageMentionSuggest(src: PageSource) {
  return Extension.create({
    name: 'pageMentionSuggest',
    addProseMirrorPlugins() {
      return [
        Suggestion({
          editor: this.editor,
          pluginKey: new PluginKey('pageMentionSuggest'),
          char: '[[',
          allowSpaces: true,
          items: ({ query }) => pageItems(src, query.replace(/\]+$/, '')),
          command: ({ editor, range, props }) => {
            // Supprime aussi d'éventuels « ]] » tapés après la recherche.
            const after = editor.state.doc.textBetween(range.to, Math.min(range.to + 2, editor.state.doc.content.size))
            const to = after === ']]' ? range.to + 2 : range.to
            editor.chain().focus().insertContentAt({ from: range.from, to }, [pageMentionContent(src, props as any), { type: 'text', text: ' ' }]).run()
          },
          render: suggestionRenderer({ emptyLabel: t('pageMention.none') }),
        }),
      ]
    },
  })
}
