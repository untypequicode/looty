import type { Editor } from '@tiptap/core'
import type { Node as PMNode, Schema } from '@tiptap/pm/model'
import { VueNodeViewRenderer } from '@tiptap/vue-3'
import { computed, nextTick, ref, toRaw, watch, type Component, type Ref } from 'vue'

/**
 * Nœuds dont la source (LaTeX, Mermaid) est leur propre texte, et non un attribut : en
 * collaboration, Yjs fusionne les frappes caractère par caractère au lieu de garder la
 * dernière version enregistrée, et annuler / rétablir passe par l'historique commun.
 * Valeur : ancien attribut qui contenait la source (documents pas encore convertis).
 */
export const SOURCE_NODES: Record<string, string> = { mathBlock: 'latex', mathInline: 'latex', mermaid: 'code' }

/**
 * Version du format des documents, annoncée au serveur de collaboration : il refuse les
 * interfaces plus anciennes, qui supprimeraient les nœuds qu'elles ne savent pas lire.
 */
export const DOC_SCHEMA = 2

/** Spécification commune : texte brut, sans marques, affiché par la vue du nœud. */
export const sourceSpec = { content: 'text*', marks: '', code: true, atom: true } as const

/**
 * Vue d'un nœud source : elle affiche et édite elle-même la source, ProseMirror ne doit pas
 * y placer le texte (Tiptap fournit sinon un conteneur détaché, et le rendu devient éditable
 * comme du texte au lieu d'être un bloc insécable).
 */
export function sourceNodeView(component: Component) {
  const render = VueNodeViewRenderer(component as any)
  return (props: Parameters<typeof render>[0]) => {
    const view = render(props)
    Object.defineProperty(view, 'contentDOM', { get: () => null })
    return view
  }
}

/** Source d'un nœud (texte, ou ancien attribut tant que le document n'est pas converti). */
export function nodeSource(node: PMNode): string {
  const legacy = SOURCE_NODES[node.type.name]
  return node.textContent || (legacy ? String(node.attrs[legacy] ?? '') : '')
}

/** Contenu d'un nœud source, pour `type.create(attrs, sourceText(schema, latex))`. */
export function sourceText(schema: Schema, text: string) {
  return text ? schema.text(text) : null
}

/** Contenu JSON équivalent (insertContent, imports). */
export function sourceJSON(text: string) {
  return text ? [{ type: 'text', text }] : []
}

/** Plus petite modification qui transforme `a` en `b` : [début, fin dans a, fin dans b]. */
export function textDiff(a: string, b: string): [number, number, number] {
  let start = 0
  const max = Math.min(a.length, b.length)
  while (start < max && a.charCodeAt(start) === b.charCodeAt(start)) start++
  let endA = a.length, endB = b.length
  while (endA > start && endB > start && a.charCodeAt(endA - 1) === b.charCodeAt(endB - 1)) { endA--; endB-- }
  return [start, endA, endB]
}

/**
 * Écrit la nouvelle source du nœud en ne remplaçant que la partie modifiée : deux personnes
 * qui tapent dans la même formule voient leurs saisies fusionnées.
 */
export function writeSource(editor: Editor, getPos: () => number | undefined, next: string): boolean {
  const pos = getPos()
  if (pos == null) return false
  const node = editor.state.doc.nodeAt(pos)
  const legacy = node && SOURCE_NODES[node.type.name]
  if (!node || !legacy) return false
  const cur = node.textContent
  const tr = editor.state.tr
  // Ancien format : la source quitte l'attribut pour devenir le texte du nœud.
  if (!cur && node.attrs[legacy]) tr.setNodeAttribute(pos, legacy, '')
  const [start, endCur, endNext] = textDiff(cur, next)
  if (start !== endCur || start !== endNext) {
    const from = pos + 1 + start, to = pos + 1 + endCur
    const inserted = next.slice(start, endNext)
    if (inserted) tr.replaceWith(from, to, editor.schema.text(inserted))
    else tr.delete(from, to)
  }
  if (!tr.docChanged) return false
  editor.view.dispatch(tr)
  return true
}

/**
 * Champ de saisie (input / textarea) relié à la source d'un nœud : chaque frappe devient une
 * petite modification du document ; les modifications des autres (ou annuler / rétablir)
 * mettent le champ à jour sans déplacer le curseur.
 */
export function useSourceField(
  field: Ref<HTMLInputElement | HTMLTextAreaElement | null>,
  props: { editor: Editor; node: PMNode; getPos: () => number | undefined },
) {
  // toRaw : sous le proxy de Vue, ProseMirror ne lit plus correctement le nœud.
  const source = computed(() => nodeSource(toRaw(props.node)))
  const draft = ref(source.value)

  watch(source, (next) => {
    if (next === draft.value) return
    const el = field.value
    const active = el && document.activeElement === el
    const sel = active ? [el!.selectionStart ?? 0, el!.selectionEnd ?? 0] : null
    const [start, endOld, endNew] = textDiff(draft.value, next)
    draft.value = next
    if (!sel || !el) return
    const delta = endNew - endOld
    const map = (p: number) => (p >= endOld ? p + delta : p > start ? Math.min(p, endNew) : p)
    nextTick(() => el.setSelectionRange(map(sel[0]), map(sel[1])))
  })

  /** À appeler après chaque modification du champ (frappe, complétion…). */
  function sync() {
    if (draft.value !== source.value) writeSource(props.editor, props.getPos, draft.value)
  }
  watch(draft, () => sync(), { flush: 'sync' })

  /** Annuler / rétablir dans le champ : l'historique du document (partagé, collaboratif). */
  function onHistoryKey(e: KeyboardEvent): boolean {
    if (!(e.metaKey || e.ctrlKey) || e.altKey) return false
    const k = e.key.toLowerCase()
    const redo = (k === 'z' && e.shiftKey) || (k === 'y' && !e.shiftKey)
    if (k !== 'z' && !redo) return false
    e.preventDefault()
    if (redo) props.editor.commands.redo()
    else props.editor.commands.undo()
    return true
  }
  /** Annuler du menu contextuel ou des gestes du navigateur. */
  function onBeforeInput(e: InputEvent) {
    if (e.inputType !== 'historyUndo' && e.inputType !== 'historyRedo') return
    e.preventDefault()
    if (e.inputType === 'historyRedo') props.editor.commands.redo()
    else props.editor.commands.undo()
  }
  return { source, draft, sync, onHistoryKey, onBeforeInput }
}

/** JSON d'un ancien document (source dans l'attribut) → source dans le texte du nœud. */
export function upgradeSources<T extends { type?: string; attrs?: Record<string, any>; content?: T[] }>(node: T): T {
  const legacy = node.type ? SOURCE_NODES[node.type] : undefined
  if (legacy && node.attrs?.[legacy] && !node.content?.length) {
    return { ...node, attrs: { ...node.attrs, [legacy]: '' }, content: sourceJSON(String(node.attrs[legacy])) as T[] }
  }
  return node.content ? { ...node, content: node.content.map(upgradeSources) } : node
}
