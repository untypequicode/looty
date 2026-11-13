import * as Y from 'yjs'
import { prosemirrorToYXmlFragment } from '@tiptap/y-tiptap'
import type { Node as PMNode } from '@tiptap/pm/model'

/**
 * Identifiant Yjs de l'amorçage, calculé à partir du contenu (FNV-1a 32 bits) : deux clients
 * qui amorcent le même contenu produisent exactement les mêmes opérations, que Yjs
 * dédoublonne à la fusion. Deux contenus différents (copies HTML lues à des moments
 * différents) ont des identifiants différents : au pire un doublon, jamais deux documents
 * incohérents sous le même identifiant.
 */
export function seedClientId(doc: PMNode): number {
  const s = JSON.stringify(doc.toJSON())
  let h = 0x811c9dc5
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 0x01000193)
  return (h >>> 0) || 1
}

/**
 * Amorce un document Yjs vide à partir d'un document ProseMirror (migration de l'ancien
 * contenu HTML), de façon idempotente : deux clients (ou onglets) qui amorcent le même
 * contenu en même temps obtiennent après fusion un seul exemplaire. Les enfants présents
 * avant l'amorçage (paragraphe vide du serveur) sont supprimés (suppression idempotente).
 * Retourne false si le document a déjà été amorcé.
 */
export function seedYDoc(ydoc: Y.Doc, doc: PMNode, field = 'default'): boolean {
  // Pas de bloc final obligatoire ajouté localement par l'éditeur (une ouverture simultanée
  // en ajouterait un par client) : on le fixe ici.
  const last = doc.lastChild
  const para = doc.type.schema.nodes.paragraph
  if (para && last && last.type !== para) doc = doc.type.create(doc.attrs, doc.content.addToEnd(para.create()), doc.marks)

  const clientId = seedClientId(doc)
  if (Y.getState(ydoc.store, clientId) > 0) return false
  const tmp = new Y.Doc()
  tmp.clientID = clientId
  prosemirrorToYXmlFragment(doc, tmp.getXmlFragment(field))
  const update = Y.encodeStateAsUpdate(tmp)
  tmp.destroy()

  const fragment = ydoc.getXmlFragment(field)
  const before = new Set(fragment.toArray())
  ydoc.transact(() => {
    Y.applyUpdate(ydoc, update)
    for (let i = fragment.length - 1; i >= 0; i--) if (before.has(fragment.get(i))) fragment.delete(i, 1)
  })
  return true
}
