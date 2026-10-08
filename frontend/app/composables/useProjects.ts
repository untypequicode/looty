import { computed, type Ref } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from './api'

export type ProjectKind = 'site' | 'latex'
export interface ProjectSummary {
  id: string
  kind: ProjectKind
  name: string
  slug: string | null
  published: boolean
  updated_at: string
  last_build_ok: boolean | null
  files: number
  created_by_name: string | null
}
export interface Project {
  id: string
  workspace_id: string
  kind: ProjectKind
  name: string
  slug: string | null
  published: boolean
  settings: { entry?: string; main?: string; engine?: 'pdflatex' | 'xelatex' | 'lualatex' }
  last_build_at: string | null
  last_build_ok: boolean | null
  updated_at: string
}
export interface ProjectFile {
  id: string
  path: string
  is_dir: boolean
  /** Fichier texte (éditable en direct). */
  text: boolean
  mime: string | null
  size: number
  updated_at: string
  updated_by_name: string | null
}
export interface ProjectDetail {
  project: Project
  files: ProjectFile[]
  public_url: string | null
  can_write: boolean
  can_manage: boolean
  can_publish: boolean
}
export interface LatexIssue { file: string | null; line: number | null; message: string; level: 'error' | 'warning' }
export interface CompileResult { ok: boolean; pdf: boolean; engine: string; duration_ms: number; issues: LatexIssue[]; log: string; local?: boolean }

export const projectKeys = {
  list: (ws: string) => ['projects', ws] as const,
  one: (id: string) => ['project', id] as const,
}

export function useProjectList(ws: Ref<string>) {
  return useQuery({
    queryKey: computed(() => projectKeys.list(ws.value)),
    queryFn: () => api<ProjectSummary[]>(`/api/workspaces/${ws.value}/projects`),
    enabled: computed(() => !!ws.value),
  })
}

export function useProject(id: Ref<string>) {
  return useQuery({
    queryKey: computed(() => projectKeys.one(id.value)),
    queryFn: () => api<ProjectDetail>(`/api/projects/${id.value}`, { silent: true }),
    enabled: computed(() => !!id.value),
  })
}

export function useProjectActions(id: Ref<string>) {
  const qc = useQueryClient()
  const refresh = () => qc.invalidateQueries({ queryKey: projectKeys.one(id.value) })
  const base = () => `/api/projects/${id.value}`
  return {
    refresh,
    createFile: async (path: string, isDir = false, content = '') => {
      const r = await api<{ id: string; path: string }>(`${base()}/files`, { method: 'POST', body: { path, is_dir: isDir, content } })
      await refresh()
      return r
    },
    movePath: async (fid: string, path: string) => {
      const r = await api<{ id: string; path: string }>(`${base()}/files/${fid}`, { method: 'PUT', body: { path } })
      await refresh()
      return r
    },
    remove: async (fid: string) => {
      await api(`${base()}/files/${fid}`, { method: 'DELETE' })
      await refresh()
    },
    upload: async (files: File[] | FileList, folder = '') => {
      const form = new FormData()
      form.append('folder', folder)
      for (const f of [...files]) form.append('file', f, (f as File & { webkitRelativePath?: string }).webkitRelativePath || f.name)
      const r = await api<{ files: { id: string; path: string; text: boolean }[] }>(`${base()}/upload`, { method: 'POST', body: form })
      await refresh()
      return r.files
    },
    update: async (body: Partial<{ name: string; settings: Project['settings']; slug: string; published: boolean }>) => {
      const r = await api<ProjectDetail>(base(), { method: 'PUT', body })
      qc.setQueryData(projectKeys.one(id.value), r)
      return r
    },
  }
}

/** Arbre des fichiers (dossiers implicites compris), trié : dossiers d'abord. */
export interface TreeNode { name: string; path: string; dir: boolean; file?: ProjectFile; children: TreeNode[] }
export function buildFileTree(files: ProjectFile[]): TreeNode[] {
  const root: TreeNode = { name: '', path: '', dir: true, children: [] }
  const dirs = new Map<string, TreeNode>([['', root]])
  const dirNode = (path: string): TreeNode => {
    const existing = dirs.get(path)
    if (existing) return existing
    const i = path.lastIndexOf('/')
    const parent = dirNode(i < 0 ? '' : path.slice(0, i))
    const node: TreeNode = { name: path.slice(i + 1), path, dir: true, children: [] }
    parent.children.push(node)
    dirs.set(path, node)
    return node
  }
  for (const f of files) {
    if (f.is_dir) {
      dirNode(f.path).file = f
      continue
    }
    const i = f.path.lastIndexOf('/')
    dirNode(i < 0 ? '' : f.path.slice(0, i)).children.push({ name: f.path.slice(i + 1), path: f.path, dir: false, file: f, children: [] })
  }
  const sort = (n: TreeNode) => {
    n.children.sort((a, b) => (a.dir === b.dir ? a.name.localeCompare(b.name, undefined, { numeric: true }) : a.dir ? -1 : 1))
    n.children.forEach(sort)
  }
  sort(root)
  return root.children
}

/** Journal LaTeX → erreurs et avertissements (même règles que le serveur ; compilation locale). */
export function parseLatexLog(log: string): LatexIssue[] {
  const out: LatexIssue[] = []
  const lines = log.split('\n')
  lines.forEach((l, i) => {
    const m = l.match(/^(.*?\.tex):(\d+): (.*)$/)
    if (m) {
      const ctx = lines.slice(i + 1, i + 5).find((x) => x.startsWith('l.'))
      out.push({ file: m[1]!.replace(/^\.\//, ''), line: Number(m[2]), message: ctx ? `${m[3]!.trim()} — ${ctx.trim()}` : m[3]!.trim(), level: 'error' })
      return
    }
    if (l.startsWith('! ')) {
      const msg = l.slice(2).trim()
      if (!out.some((e) => e.message.startsWith(msg))) out.push({ file: null, line: null, message: msg, level: 'error' })
      return
    }
    if (l.includes('LaTeX Warning:') || (l.includes('Package') && l.includes('Warning:'))) {
      const line = l.match(/input line (\d+)/)?.[1]
      out.push({ file: null, line: line ? Number(line) : null, message: l.trim(), level: 'warning' })
    }
  })
  return out.slice(0, 200)
}

export const fileSize = (n: number) => (n < 1024 ? `${n} o` : n < 1048576 ? `${Math.round(n / 1024)} Ko` : n < 1073741824 ? `${(n / 1048576).toFixed(1)} Mo` : `${(n / 1073741824).toFixed(2)} Go`)

// --- Drive ---
export interface DriveItem {
  id: string
  folder: string
  name: string
  is_dir: boolean
  mime: string | null
  size: number
  created_at: string
  expires_at: string | null
  uploaded_by: string | null
  uploaded_by_name: string | null
  shared: boolean
  children: number
}
export interface DriveListing {
  folder: string
  items: DriveItem[]
  max_days: number
  quota_bytes: number
  used_bytes: number
  day_choices: number[]
  can_upload: boolean
  can_manage: boolean
  user_id: string
}
export const driveKeys = { list: (ws: string, folder: string, q: string) => ['drive', ws, folder, q] as const }

export function useDrive(ws: Ref<string>, folder: Ref<string>, q: Ref<string>) {
  return useQuery({
    queryKey: computed(() => driveKeys.list(ws.value, folder.value, q.value)),
    queryFn: () => api<DriveListing>(`/api/workspaces/${ws.value}/drive?${new URLSearchParams({ folder: folder.value, ...(q.value ? { q: q.value } : {}) })}`, { silent: true }),
    enabled: computed(() => !!ws.value),
    refetchInterval: 60_000,
  })
}
