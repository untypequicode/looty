import * as Y from 'yjs'
import { HocuspocusProvider } from '@hocuspocus/provider'
import { socketBase } from '~/composables/useRealtime'
import { SLIDES_SCHEMA } from '~/slides/model'

/**
 * Connexion en lecture au document d'une présentation (« slides:<page> ») pour les fenêtres dédiées
 * (vue présentateur, diaporama en lien). L'éditeur garde sa propre connexion (Deck.vue).
 */
export function useSlidesDoc(pageId: string) {
  const config = useRuntimeConfig()
  const doc = new Y.Doc()
  const collabUrl = (config.public.collabUrl as string) || `${socketBase()}/collab`
  const provider = new HocuspocusProvider({ url: `${collabUrl}${collabUrl.includes('?') ? '&' : '?'}schema=${SLIDES_SCHEMA}`, name: `slides:${pageId}`, document: doc })
  const synced = ref(false)
  const denied = ref(false)
  provider.on('synced', () => { synced.value = true })
  provider.on('authenticationFailed', () => { denied.value = true })
  onBeforeUnmount(() => { provider.destroy(); doc.destroy() })
  return { doc, provider, synced, denied }
}
