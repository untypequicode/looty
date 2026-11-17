// Logique pure des fichiers déposés dans le chat de l'Assistant (ADR 0012) : formats lisibles, contrôles
// avant l'envoi, nature d'un dépôt (fichiers ou page glissée depuis l'arbre), péremption.

/** Taille maximale d'un fichier déposé (la même limite côté serveur). */
export const MAX_FILE_BYTES = 25 * 1024 * 1024

/** Extensions dont le serveur sait extraire le texte (miroir de `backend/src/extract.rs`). */
const DOCUMENT_EXT = ['pdf', 'docx', 'pptx']
const TEXT_EXT = ['txt', 'text', 'md', 'markdown', 'csv', 'tsv', 'log', 'json', 'jsonl', 'yaml', 'yml', 'toml', 'ini', 'xml', 'html', 'htm', 'tex', 'bib', 'rst', 'srt']
const CODE_EXT = ['rs', 'py', 'js', 'mjs', 'ts', 'tsx', 'jsx', 'vue', 'java', 'kt', 'c', 'h', 'cpp', 'hpp', 'cc', 'cs', 'go', 'rb', 'php', 'sh', 'bash', 'zsh', 'sql', 'css', 'scss', 'lua', 'r', 'swift', 'scala', 'hs', 'ml', 'pl', 'dart', 'm', 'ipynb']

const extOf = (name: string) => (name.includes('.') ? name.split('.').pop()!.toLowerCase() : '')

/** Type d'icône d'un fichier joint. */
export type FileKind = 'pdf' | 'doc' | 'slides' | 'data' | 'code' | 'text'

export function fileKind(name: string): FileKind {
  const e = extOf(name)
  if (e === 'pdf') return 'pdf'
  if (e === 'docx') return 'doc'
  if (e === 'pptx') return 'slides'
  if (['csv', 'tsv', 'json', 'jsonl', 'yaml', 'yml', 'toml', 'xml'].includes(e)) return 'data'
  if (CODE_EXT.includes(e)) return 'code'
  return 'text'
}

/** L'assistant peut-il lire ce fichier (d'après son nom, ou son type MIME à défaut d'extension connue) ? */
export function isReadable(name: string, mime = ''): boolean {
  const e = extOf(name)
  if (DOCUMENT_EXT.includes(e) || TEXT_EXT.includes(e) || CODE_EXT.includes(e)) return true
  return mime === 'application/pdf' || mime.startsWith('text/')
}

export type FileProblem = 'unsupported' | 'too_large' | 'empty'

/** Contrôle avant l'envoi (le serveur refuse aussi : fichier sans texte, illisible…). */
export function checkFile(f: { name: string; size: number; type?: string }): FileProblem | null {
  if (!isReadable(f.name, f.type ?? '')) return 'unsupported'
  if (f.size > MAX_FILE_BYTES) return 'too_large'
  if (f.size === 0) return 'empty'
  return null
}

/** Clé de traduction du message d'une erreur de dépôt (code du serveur ou problème local). */
export function uploadErrorKey(code: string): string {
  const known = ['file_unsupported', 'file_too_large', 'file_no_text', 'file_unreadable', 'too_many_uploads', 'drive_quota_exceeded', 'demo_unavailable']
  if (known.includes(code)) return `ai.files.err.${code}`
  if (code === 'unsupported' || code === 'too_large') return `ai.files.err.file_${code}`
  if (code === 'empty') return 'ai.files.err.file_no_text'
  return 'ai.files.err.other'
}

/** Ce qu'on est en train de glisser au-dessus de la zone de saisie. */
export type DropIntent = 'files' | 'page' | 'none'

/**
 * Nature d'un glisser : des fichiers de l'ordinateur (`Files`), ou une page / un entraînement de l'arbre
 * (type `application/x-looty-page`, posé par l'arbre). Rien d'autre ne déclenche la zone de dépôt.
 */
export function dropIntent(types: readonly string[] | undefined): DropIntent {
  if (!types) return 'none'
  if (types.includes('Files')) return 'files'
  if (types.includes('application/x-looty-page')) return 'page'
  return 'none'
}

export interface DraggedPage { ws: string; id: string; study?: boolean; title?: string }

/** Pièce jointe qu'on obtient en lâchant une page (ou un entraînement) de l'arbre. */
export function pageAttachment(d: DraggedPage): { kind: 'page' | 'study'; id: string; title: string; ws: string } {
  return { kind: d.study ? 'study' : 'page', id: d.id, title: d.title ?? '', ws: d.ws }
}

/** Fichiers collés (presse-papiers) : les éléments de type fichier, sans le texte ni le HTML. */
export function pastedFiles(items: ArrayLike<{ kind: string; getAsFile(): File | null }> | undefined): File[] {
  if (!items) return []
  const out: File[] = []
  for (let i = 0; i < items.length; i++) {
    const it = items[i]!
    if (it.kind === 'file') {
      const f = it.getAsFile()
      if (f) out.push(f)
    }
  }
  return out
}

/** Le fichier du Drive a-t-il expiré (donc plus lisible par l'assistant) ? Sans date : jamais. */
export function isExpired(expiresAt: string | null | undefined, now: number = Date.now()): boolean {
  return !!expiresAt && new Date(expiresAt).getTime() <= now
}

