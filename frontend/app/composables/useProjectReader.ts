import { IMAGE_EXT } from '~/editor/notebookMarkdown'
import type { ProjectDetail } from './useProjects'

/**
 * Projet porté par une page, en lecture (lien public et mode lecture) : notebook enregistré
 * ou dernier PDF LaTeX, sans noyau ni temps réel. Seule la façon d'atteindre les fichiers
 * change : routes publiques (jeton du lien) ou routes des membres.
 */
export interface ReaderProject {
  kind: 'notebook' | 'latex'
  /** Notebook ouvert d'abord. */
  main: string | null
  notebooks: string[]
  /** Adresse du dernier PDF compilé, null s'il n'y en a pas. */
  pdf: string | null
  /** Contenu d'un .ipynb. */
  notebook: (path: string) => Promise<string>
  /** Téléchargement du .ipynb tel quel. */
  download: (path: string) => string
  /** Fichiers du projet connus (résolution des chemins relatifs) ; null : inconnus (lien public). */
  files: string[] | null
  /** Chemins candidats d'une image ou d'un lien → adresse, ou null. */
  fileUrl: (paths: string[]) => string | null
}

/** Résumé renvoyé par /api/public/pages/<jeton>. */
export interface PublicProjectSummary { kind: string; main: string | null; notebooks: string[]; has_pdf: boolean }

const encodePath = (p: string) => p.split('/').map(encodeURIComponent).join('/')

export function publicProject(token: string, p: PublicProjectSummary | null | undefined): ReaderProject | null {
  if (!p || (p.kind !== 'notebook' && p.kind !== 'latex')) return null
  const base = `/api/public/pages/${token}`
  return {
    kind: p.kind,
    main: p.main,
    notebooks: p.notebooks ?? [],
    pdf: p.kind === 'latex' && p.has_pdf ? `${base}/output.pdf` : null,
    notebook: async (path) => {
      const r = await fetch(`${base}/notebook?path=${encodeURIComponent(path)}`)
      if (!r.ok) throw new Error(String(r.status))
      return r.text()
    },
    download: (path) => `${base}/notebook?path=${encodeURIComponent(path)}&download=1`,
    files: null,
    // Images seulement : le lien public ne sert aucun autre fichier du projet.
    fileUrl: ([first, alt]) => (first && IMAGE_EXT.test(first) ? `${base}/files/${encodePath(first)}${alt ? `?alt=${encodeURIComponent(alt)}` : ''}` : null),
  }
}

export function memberProject(d: ProjectDetail | null | undefined): ReaderProject | null {
  if (!d || (d.project.kind !== 'notebook' && d.project.kind !== 'latex')) return null
  const id = d.project.id
  const files = d.files.filter((f) => !f.is_dir)
  const byPath = new Map(files.map((f) => [f.path, f]))
  const raw = (path: string) => { const f = byPath.get(path); return f ? `/api/projects/${id}/files/${f.id}/raw` : '' }
  const notebooks = d.project.kind === 'notebook' ? files.filter((f) => /\.ipynb$/i.test(f.path)).map((f) => f.path) : []
  const main = d.project.settings.main ?? 'notebook.ipynb'
  return {
    kind: d.project.kind,
    main: notebooks.includes(main) ? main : notebooks[0] ?? null,
    notebooks,
    pdf: d.project.kind === 'latex' && d.project.last_build_at ? `/api/projects/${id}/output.pdf?v=${new Date(d.project.last_build_at).getTime()}` : null,
    notebook: async (path) => {
      const r = await fetch(raw(path), { credentials: 'same-origin' })
      if (!r.ok) throw new Error(String(r.status))
      return r.text()
    },
    download: raw,
    files: files.map((f) => f.path),
    fileUrl: ([p]) => (p && byPath.has(p) ? `${raw(p)}?v=${byPath.get(p)!.size}` : null),
  }
}

/** Texte de la page vide (aucun texte, image ou bloc) : le projet s'affiche seul, sans onglets. */
export function isEmptyPage(html: string | null | undefined): boolean {
  if (!html) return true
  if (/<(img|iframe|video|audio|table|hr|svg|math|embed|object)\b/i.test(html) || /data-type=|class="[^"]*(math|mermaid|embed)/i.test(html)) return false
  return !html.replace(/<[^>]*>/g, '').replace(/&nbsp;|​/g, '').trim()
}
