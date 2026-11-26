import { computed, type Component } from 'vue'
import { FileText, Sigma, Code2, NotebookText, PenTool, Presentation } from 'lucide-vue-next'
import { useQueryClient } from '@tanstack/vue-query'
import type { Page } from '~/types'
import { api } from './api'
import { useWorkspace, wsKeys, links } from './useWorkspace'
import { confirmDialog } from './useConfirm'
import { translate } from './useI18n'
import { toast } from './useToast'
import { trashedToast } from './useTrash'
import { useStudySets, useStudyActions, type StudyKind, type StudySummary } from './useStudy'
import { confirmMoveAccess } from './useShare'
import { computeMove, lastRootPosition, type DropWhere, type TreeEntry } from './treeMove'

export interface PageNode extends Page {
  children: PageNode[]
  depth: number
  /** Entraînement rangé dans l'arbre (ADR 0011) : type du jeu ; absent pour une page. */
  study?: StudyKind
}

/** Un entraînement sous forme de nœud d'arbre : mêmes champs de rangement qu'une page. */
export function studyAsNode(s: StudySummary): Page & { study: StudyKind } {
  return {
    id: s.id, workspace_id: s.workspace_id, private_to: s.private_to, parent_id: s.parent_id, icon: null, title: s.title, content: null,
    created_by: s.owner_id, updated_by: null, position: s.position, created_at: s.created_at, updated_at: s.updated_at,
    can_edit: s.can_edit, study: s.kind,
  } as unknown as Page & { study: StudyKind }
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

/** Projet créé avec une page (elle s'ouvre alors sur ses fichiers et son aperçu). */
export interface NewPageProject { kind: 'latex' | 'site' | 'notebook'; template?: string }

export function usePages() {
  const qc = useQueryClient()
  const { wsId, pages } = useWorkspace()
  const study = useStudySets(wsId)
  const studyActions = useStudyActions()
  // Les entraînements se rangent dans l'arbre parmi les pages.
  const tree = computed(() => buildTree([...(pages.data.value ?? []), ...(study.data.value ?? []).map(studyAsNode)]) as PageNode[])
  const favorites = computed(() => (pages.data.value ?? []).filter((p) => p.is_favorite))

  function patchLocal(id: string, patch: Partial<Page>) {
    qc.setQueryData<Page[]>(wsKeys.pages(wsId.value), (old) => old?.map((p) => (p.id === id ? { ...p, ...patch } : p)))
    qc.setQueryData<Page>(['page', id], (old) => (old ? { ...old, ...patch } : old))
  }

  async function createPage(opts: { parentId?: string | null; title?: string; content?: string; icon?: string; project?: NewPageProject; defaultView?: 'doc' | 'canvas' | 'slides'; personal?: boolean; templateId?: string; copySlidesFrom?: string } = {}) {
    const page = await api<Page>(`/api/workspaces/${wsId.value}/pages`, {
      method: 'POST',
      body: { title: opts.title ?? '', parent_id: opts.parentId ?? null, content: opts.content, icon: opts.icon, project: opts.project, default_view: opts.defaultView, personal: opts.personal ?? false, template_id: opts.templateId, copy_slides_from: opts.copySlidesFrom },
    })
    qc.setQueryData<Page[]>(wsKeys.pages(wsId.value), (old) => (old && !old.some((p) => p.id === page.id) ? [...old, { ...page, content: null, can_edit: true }] : old))
    // Droits calculés par le serveur (sous-pages permises par un partage…).
    qc.invalidateQueries({ queryKey: wsKeys.pages(wsId.value) })
    return page
  }

  // `project` : la page porte un nouveau projet (LaTeX, site, notebook).
  async function createAndOpen(parentId?: string | null, project?: NewPageProject, personal = false) {
    const page = await createPage({ parentId, project, personal, title: project ? translate(`projects.pageKinds.${project.kind}`) : undefined })
    await navigateTo(links.doc(wsId.value, page.id))
    return page
  }

  async function updatePage(id: string, data: Partial<Pick<Page, 'title' | 'icon' | 'parent_id' | 'position' | 'content' | 'default_view'>>) {
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

  /** Éléments de l'arbre (pages et entraînements) pour calculer un rangement. */
  function entries(): TreeEntry[] {
    return [
      ...(pages.data.value ?? []).map((p) => ({ id: p.id, parent_id: p.parent_id ?? null, position: p.position })),
      ...(study.data.value ?? []).map((s) => ({ id: s.id, parent_id: s.parent_id ?? null, position: s.position, study: true })),
    ]
  }
  async function place(all: TreeEntry[], id: string, parentId: string | null, position: number) {
    if (all.find((p) => p.id === id)?.study) await studyActions.patch(id, { parent_id: parentId, position })
    else await updatePage(id, { parent_id: parentId, position })
  }

  /** Déplace une page ou un entraînement avant/après un élément, ou à l'intérieur d'une page (en dernier enfant). */
  async function movePage(id: string, targetId: string, where: DropWhere) {
    const all = entries()
    const m = computeMove(all, id, targetId, where)
    if (!m) return
    // Changer de page parente change qui voit la page : on le montre avant (pages seulement).
    const cur = (pages.data.value ?? []).find((p) => p.id === id)
    if (cur && (cur.parent_id ?? null) !== m.parentId && !(await confirmMoveAccess(wsId.value, cur, m.parentId))) return
    await place(all, id, m.parentId, m.position)
  }

  /** Remonte une page ou un entraînement à la racine de l'espace, en dernier. */
  async function moveToRoot(id: string) {
    const all = entries()
    await place(all, id, null, lastRootPosition(all))
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
    // La page et ses sous-pages partent dans la corbeille.
    qc.setQueryData<Page[]>(wsKeys.pages(wsId.value), (old) => {
      if (!old) return old
      const gone = new Set([page.id])
      let grew = true
      while (grew) { grew = false; for (const p of old) if (p.parent_id && gone.has(p.parent_id) && !gone.has(p.id)) { gone.add(p.id); grew = true } }
      return old.filter((p) => !gone.has(p.id))
    })
    trashedToast('pages', page.id)
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
      // Présentation : les diapositives sont copiées par le serveur (leur état n'est pas dans `content`).
      ...(src.default_view === 'slides' ? { defaultView: 'slides' as const, copySlidesFrom: src.id } : {}),
    })
    await navigateTo(links.doc(wsId.value, copy.id))
  }

  return { tree, favorites, createPage, createAndOpen, updatePage, movePage, moveToRoot, deletePage, toggleFavorite, duplicatePage, patchLocal }
}

/** Icône d'une page sans emoji : celle de son projet (LaTeX, site, notebook) ou d'un document. */
export function pageKindIcon(p: Pick<Page, 'project_kind' | 'default_view'>): Component {
  if (!p.project_kind && p.default_view === 'canvas') return PenTool
  if (!p.project_kind && p.default_view === 'slides') return Presentation
  return p.project_kind === 'latex' ? Sigma : p.project_kind === 'site' ? Code2 : p.project_kind === 'notebook' ? NotebookText : FileText
}
