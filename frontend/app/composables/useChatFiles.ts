import { ref } from 'vue'
import { translate } from './useI18n'
import { toast } from './useToast'
import { checkFile, uploadErrorKey } from '~/utils/chatFiles'
import type { AiAttachment } from './useAi'

/** Réponse du serveur à `POST /api/ai/files` : le fichier est rangé dans le Drive personnel (ADR 0012). */
export interface UploadedChatFile {
  id: string
  workspace_id: string
  name: string
  size: number
  pages: number
  chars: number
  expires_at: string
}

/** Envoi en cours (puce avec progression) ou refusé. */
export interface ChatUpload { key: string; name: string; progress: number }

/** Envoie un fichier avec suivi de progression (XHR : `fetch` n'expose pas l'envoi). */
export function uploadChatFile(file: File, onProgress: (ratio: number) => void): Promise<UploadedChatFile> {
  const form = new FormData()
  form.append('file', file, file.name)
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest()
    xhr.open('POST', '/api/ai/files')
    xhr.withCredentials = true
    xhr.upload.onprogress = (e) => { if (e.lengthComputable) onProgress(e.loaded / e.total) }
    xhr.onload = () => {
      let body: any = {}
      try { body = JSON.parse(xhr.responseText || '{}') } catch { /* corps vide */ }
      if (xhr.status < 300) resolve(body as UploadedChatFile)
      else reject({ status: xhr.status, code: String(body?.error ?? '') })
    }
    xhr.onerror = () => reject({ status: 0, code: 'network' })
    xhr.send(form)
  })
}

/** Pièce jointe d'un fichier déposé : même forme que les autres, avec la date d'expiration du Drive. */
export function fileAttachment(f: UploadedChatFile): AiAttachment {
  return { kind: 'file', id: f.id, title: f.name, ws: f.workspace_id, expiresAt: f.expires_at, pages: f.pages }
}

/**
 * Dépose des fichiers dans le chat : contrôle local (format, taille), envoi, puis `onDone` avec la pièce jointe.
 * Chaque refus affiche un message (format non lisible, trop gros, PDF scanné sans texte…).
 */
export function useChatUploads(onDone: (a: AiAttachment) => void) {
  const uploads = ref<ChatUpload[]>([])
  async function one(file: File) {
    const problem = checkFile(file)
    if (problem) { toast.error(`${file.name} : ${translate(uploadErrorKey(problem))}`); return }
    const entry: ChatUpload = { key: `${Date.now()}-${Math.random()}`, name: file.name, progress: 0 }
    uploads.value = [...uploads.value, entry]
    try {
      const done = await uploadChatFile(file, (r) => { uploads.value = uploads.value.map((u) => (u.key === entry.key ? { ...u, progress: r } : u)) })
      onDone(fileAttachment(done))
    } catch (e: any) {
      toast.error(`${file.name} : ${translate(uploadErrorKey(String(e?.code ?? '')))}`)
    } finally {
      uploads.value = uploads.value.filter((u) => u.key !== entry.key)
    }
  }
  /** Au plus 10 fichiers à la fois (le maximum de pièces jointes d'un message). */
  async function addFiles(files: File[]) {
    await Promise.all(files.slice(0, 10).map(one))
  }
  return { uploads, addFiles }
}
