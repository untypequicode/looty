import { ref } from 'vue'
import { useQueryClient } from '@tanstack/vue-query'
import type { Page, Workspace } from '~/types'
import { api } from './api'
import { links, wsKeys } from './useWorkspace'
import { toast } from './useToast'
import { translate } from './useI18n'

/** Page à déplacer vers un autre espace (fenêtre « Déplacer vers… »). */
export const moveRequest = ref<{ ws: string; id: string; title: string; kind: 'page' | 'study' } | null>(null)
/** Page en cours de glisser (pour les dépôts d'un espace à l'autre). */
export const draggingPage = ref<{ ws: string; id: string } | null>(null)

export function openMovePage(ws: string, page: Pick<Page, 'id' | 'title'>, kind: 'page' | 'study' = 'page') {
  moveRequest.value = { ws, id: page.id, title: page.title, kind }
}

/** Sous-pages d'une page dans le cache de son espace (elle comprise). */
function subtree(all: Page[], id: string): Set<string> {
  const out = new Set([id])
  let grew = true
  while (grew) {
    grew = false
    for (const p of all) if (p.parent_id && out.has(p.parent_id) && !out.has(p.id)) { out.add(p.id); grew = true }
  }
  return out
}

/**
 * Déplace une page et ses sous-pages vers un autre espace (perso compris). Si la page
 * affichée en fait partie, on la suit dans son nouvel espace.
 */
export function useMovePage() {
  const qc = useQueryClient()
  const route = useRoute()
  function nameOf(ws: string) {
    if (qc.getQueryData<{ id: string }>(['personal-space'])?.id === ws) return translate('personal.title')
    return qc.getQueryData<Workspace[]>(wsKeys.list)?.find((w) => w.id === ws)?.name ?? ''
  }
  /** Entraînement : il change d'espace ou de page parente (mêmes règles qu'une page, sans sous-éléments). */
  async function moveStudy(fromWs: string, id: string, toWs: string, parentId: string | null = null) {
    await api(`/api/study/sets/${id}/move`, { method: 'POST', body: { workspace_id: toWs, parent_id: parentId } })
    await Promise.all([
      qc.invalidateQueries({ queryKey: ['ws', fromWs, 'study'] }),
      qc.invalidateQueries({ queryKey: ['ws', toWs, 'study'] }),
      qc.invalidateQueries({ queryKey: ['study'] }),
    ])
    const shown = route.params.ws === fromWs && route.params.id === id && route.path.includes('/study/')
    if (shown && fromWs !== toWs) await navigateTo(links.studySet(toWs, id))
    toast.success(translate('move.doneStudy', { name: nameOf(toWs) }), shown ? {} : { action: { label: translate('move.open'), run: () => navigateTo(links.studySet(toWs, id)) } })
  }
  async function move(fromWs: string, id: string, toWs: string, parentId: string | null = null) {
    const moved = subtree(qc.getQueryData<Page[]>(wsKeys.pages(fromWs)) ?? [], id)
    const r = await api<{ moved: number }>(`/api/workspaces/${fromWs}/pages/${id}/move`, { method: 'POST', body: { workspace_id: toWs, parent_id: parentId } })
    qc.setQueryData<Page[]>(wsKeys.pages(fromWs), (old) => old?.filter((p) => !moved.has(p.id)))
    await Promise.all([qc.invalidateQueries({ queryKey: wsKeys.pages(fromWs) }), qc.invalidateQueries({ queryKey: wsKeys.pages(toWs) })])
    const open = () => navigateTo(links.doc(toWs, id))
    const shown = route.params.ws === fromWs && typeof route.params.page === 'string' && moved.has(route.params.page)
    if (shown) await navigateTo(links.doc(toWs, route.params.page as string))
    toast.success(translate('move.done', { count: r.moved, name: nameOf(toWs) }), shown ? {} : { action: { label: translate('move.open'), run: open } })
    return r
  }
  return { move, moveStudy }
}
