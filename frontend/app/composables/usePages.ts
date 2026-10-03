import { computed } from 'vue'
import { useQueryClient } from '@tanstack/vue-query'
import type { Page } from '~/types'
import { api } from './api'
import { useWorkspace, wsKeys, links } from './useWorkspace'
import { confirmDialog } from './useConfirm'
import { translate } from './useI18n'
import { toast } from './useToast'

export interface PageNode extends Page {
  children: PageNode[]
  depth: number
}

/** Construit l'arbre des pages (trié par position). */
export function buildTree(pages: Page[]): PageNode[] {
  const byParent = new Map<string | null, Page[]>()
  const ids = new Set(pages.map((p) => p.id))
  for (const p of pages) {
    // Parent invisible pour l'utilisateur -> la page remonte à la racine.
    const parent = p.parent_id && ids.has(p.parent_id) ? p.parent_id : null
    if (!byParent.has(parent)) byParent.set(parent, [])
    byParent.get(parent)!.push(p)
  }
  const build = (parent: string | null, depth: number): PageNode[] =>
    (byParent.get(parent) ?? [])
      .sort((a, b) => a.position - b.position || (a.created_at ?? '').localeCompare(b.created_at ?? ''))
      .map((p) => ({ ...p, depth, children: build(p.id, depth + 1) }))
  return build(null, 0)
}

/** Chemin (ancêtres) d'une page, de la racine à la page. */
export function pagePath(pages: Page[], id: string): Page[] {
  const map = new Map(pages.map((p) => [p.id, p]))
  const out: Page[] = []
  let cur = map.get(id)
  let guard = 0
  while (cur && guard++ < 64) {
    out.unshift(cur)
    cur = cur.parent_id ? map.get(cur.parent_id) : undefined
  }
  return out
}

export function usePages() {
  const qc = useQueryClient()
  const { wsId, pages } = useWorkspace()
  const tree = computed(() => buildTree(pages.data.value ?? []))
  const favorites = computed(() => (pages.data.value ?? []).filter((p) => p.is_favorite))

  function patchLocal(id: string, patch: Partial<Page>) {
    qc.setQueryData<Page[]>(wsKeys.pages(wsId.value), (old) => old?.map((p) => (p.id === id ? { ...p, ...patch } : p)))
    qc.setQueryData<Page>(['page', id], (old) => (old ? { ...old, ...patch } : old))
  }

  async function createPage(opts: { parentId?: string | null; title?: string; content?: string; icon?: string } = {}) {
    const page = await api<Page>(`/api/workspaces/${wsId.value}/pages`, {
      method: 'POST',
      body: { title: opts.title ?? '', parent_id: opts.parentId ?? null, content: opts.content, icon: opts.icon },
    })
    qc.setQueryData<Page[]>(wsKeys.pages(wsId.value), (old) => (old && !old.some((p) => p.id === page.id) ? [...old, { ...page, content: null, can_edit: true }] : old))
    return page
  }

  async function createAndOpen(parentId?: string | null) {
    const page = await createPage({ parentId })
    await navigateTo(links.doc(wsId.value, page.id))
    return page
  }

  async function updatePage(id: string, data: Partial<Pick<Page, 'title' | 'icon' | 'parent_id' | 'position' | 'content'>>) {
    const prev = (pages.data.value ?? []).find((p) => p.id === id)
    const { content, ...meta } = data
    patchLocal(id, meta)
    try {
      return await api<Page>(`/api/workspaces/${wsId.value}/pages/${id}`, { method: 'PUT', body: data })
    } catch (e) {
      if (prev) patchLocal(id, prev)
      throw e
    }
  }

  /** Déplace une page avant/après une autre, ou à l'intérieur (en dernier enfant). */
  async function movePage(id: string, targetId: string, where: 'before' | 'after' | 'inside') {
    const all = pages.data.value ?? []
    const target = all.find((p) => p.id === targetId)
    if (!target || id === targetId) return
    let parentId: string | null
    let position: number
    if (where === 'inside') {
      parentId = target.id
      const kids = all.filter((p) => p.parent_id === target.id && p.id !== id)
      position = (kids.reduce((m, p) => Math.max(m, p.position), 0) || 0) + 65536
    } else {
      parentId = target.parent_id ?? null
      const siblings = all.filter((p) => (p.parent_id ?? null) === parentId && p.id !== id).sort((a, b) => a.position - b.position)
      const idx = siblings.findIndex((p) => p.id === target.id)
      const before = where === 'before' ? siblings[idx - 1] : target
      const after = where === 'before' ? target : siblings[idx + 1]
      const lo = before?.position ?? 0
      const hi = after?.position ?? lo + 131072
      position = (lo + hi) / 2
    }
    // Interdit de déplacer une page dans sa propre descendance.
    let cur: string | null = parentId
    while (cur) {
      if (cur === id) return
      cur = all.find((p) => p.id === cur)?.parent_id ?? null
    }
    await updatePage(id, { parent_id: parentId, position })
  }

  async function deletePage(page: Pick<Page, 'id' | 'title'>) {
    const childCount = (pages.data.value ?? []).filter((p) => p.parent_id === page.id).length
    const ok = await confirmDialog({
      title: translate('docs.deleteTitle', { title: page.title || translate('docs.untitled') }),
      message: childCount ? translate('docs.deleteWithChildren', { count: childCount }) : translate('docs.deleteMessage'),
      confirmLabel: translate('common.delete'),
      danger: true,
    })
    if (!ok) return false
    await api(`/api/workspaces/${wsId.value}/pages/${page.id}`, { method: 'DELETE' })
    qc.setQueryData<Page[]>(wsKeys.pages(wsId.value), (old) => old?.filter((p) => p.id !== page.id))
    toast.success(translate('docs.deleted'))
    return true
  }

  async function toggleFavorite(page: Pick<Page, 'id' | 'is_favorite'>) {
    patchLocal(page.id, { is_favorite: !page.is_favorite })
    try {
      const r = await api<{ is_favorite: boolean }>(`/api/workspaces/${wsId.value}/pages/${page.id}/favorite`, { method: 'POST' })
      patchLocal(page.id, { is_favorite: r.is_favorite })
    } catch {
      patchLocal(page.id, { is_favorite: page.is_favorite })
    }
  }

  async function duplicatePage(id: string) {
    const src = await api<Page>(`/api/workspaces/${wsId.value}/pages/${id}`)
    const copy = await createPage({
      parentId: src.parent_id,
      title: `${src.title || translate('docs.untitled')} (${translate('docs.copy')})`,
      content: src.content ?? '',
      icon: src.icon ?? undefined,
    })
    await navigateTo(links.doc(wsId.value, copy.id))
  }

  return { tree, favorites, createPage, createAndOpen, updatePage, movePage, deletePage, toggleFavorite, duplicatePage, patchLocal }
}
