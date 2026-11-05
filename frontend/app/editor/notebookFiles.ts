import { dirOf } from './notebookMarkdown'

/**
 * Fichiers collés ou glissés sur une cellule de notebook (ADR 0006) : envoyés dans le projet,
 * puis un lien (Markdown) ou un chemin (code) est inséré dans la cellule.
 */

const handled = new WeakSet<Event>()
/** Dépôt pris en charge par une cellule : le dépôt global du projet l'ignore. */
export const markNotebookDrop = (e: Event) => { handled.add(e) }
export const isNotebookDrop = (e: Event) => handled.has(e)

/** Le glisser contient des fichiers (et pas une cellule ou du texte). */
export const hasFiles = (dt: DataTransfer | null | undefined) => !!dt && Array.from(dt.types ?? []).includes('Files')

const pad = (n: number) => String(n).padStart(2, '0')
const EXT: Record<string, string> = { 'image/png': 'png', 'image/jpeg': 'jpg', 'image/gif': 'gif', 'image/webp': 'webp', 'image/svg+xml': 'svg' }
/** Nom d'une image collée : collage-AAAAMMJJ-HHMMSS.png (extension du type de l'image). */
export function pastedName(mime: string, d = new Date()): string {
  return `collage-${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}${pad(d.getSeconds())}.${EXT[mime] ?? 'png'}`
}

/** Nom libre dans le dossier : `nom.ext`, sinon `nom-2.ext`, `nom-3.ext`… */
export function uniqueName(folder: string, name: string, taken: Set<string>): string {
  const join = (n: string) => (folder ? `${folder}/${n}` : n)
  const dot = name.lastIndexOf('.')
  const stem = dot > 0 ? name.slice(0, dot) : name
  const ext = dot > 0 ? name.slice(dot) : ''
  let candidate = name
  for (let i = 2; taken.has(join(candidate)); i++) candidate = `${stem}-${i}${ext}`
  return candidate
}

/** Chemin écrit dans un lien Markdown : espaces et parenthèses encodés. */
export const mdPath = (p: string) => p.replace(/ /g, '%20').replace(/\(/g, '%28').replace(/\)/g, '%29')
export const stemOf = (name: string) => (name.lastIndexOf('.') > 0 ? name.slice(0, name.lastIndexOf('.')) : name)

export interface PlannedFile { file: File; folder: string; name: string; path: string; image: boolean }

/**
 * Où ranger chaque fichier et sous quel nom. Cellule Markdown : images dans `images/` à côté
 * du notebook, autres fichiers à côté du notebook ; cellule de code : à côté du notebook.
 */
export function planUploads(files: File[], opts: { notebookPath: string; cell: 'markdown' | 'code'; pasted: boolean; existing: Iterable<string>; now?: Date }): PlannedFile[] {
  const taken = new Set(opts.existing)
  const dir = dirOf(opts.notebookPath)
  return files.map((file) => {
    const image = file.type.startsWith('image/')
    const folder = opts.cell === 'markdown' && image ? (dir ? `${dir}/images` : 'images') : dir
    const wanted = opts.pasted && image ? pastedName(file.type, opts.now) : (file.name || pastedName(file.type, opts.now)).replace(/[/\\:*?"<>|]/g, '_')
    const name = uniqueName(folder, wanted, taken)
    const path = folder ? `${folder}/${name}` : name
    taken.add(path)
    return { file, folder, name, path, image }
  })
}

/** Texte inséré dans la cellule pour les fichiers envoyés. */
export function insertionText(planned: PlannedFile[], cell: 'markdown' | 'code', notebookPath: string): string {
  const dir = dirOf(notebookPath)
  const rel = (p: string) => (dir && p.startsWith(`${dir}/`) ? p.slice(dir.length + 1) : p)
  if (cell === 'code') return planned.map((p) => JSON.stringify(p.path)).join(', ')
  return planned.map((p) => (p.image ? `![${stemOf(p.name)}](${mdPath(rel(p.path))})` : `[${p.name}](${mdPath(rel(p.path))})`)).join('\n')
}
