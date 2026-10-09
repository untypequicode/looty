import { Extension } from '@tiptap/core'
import { Plugin, PluginKey } from '@tiptap/pm/state'
import type { Node as PMNode } from '@tiptap/pm/model'
import { computed, shallowRef, watch, type ShallowRef } from 'vue'
import { scopeBuilder, ensureMhchem, type Scope } from './latex'
import { nodeSource } from './source'

/**
 * Préambule LaTeX de l'espace de travail courant (réglages de l'espace) :
 * ses macros et paquets s'appliquent avant celles de chaque document.
 */
export const workspacePreamble = shallowRef('')

export interface MathScopeStorage {
  /** Incrémenté quand les déclarations du document (ou leur ordre) changent. */
  version: ShallowRef<number>
  /** Portée de chaque formule, indexée par sa position. */
  scopes: Map<number, Scope>
  signature: string
  rebuild?: () => void
  stop?: () => void
}

const MATH = new Set(['mathBlock', 'mathInline'])

/**
 * Préambule LaTeX partagé : les \newcommand, \DeclareMathOperator, \usepackage…
 * d'une formule s'appliquent aux formules suivantes du document.
 */
export const MathScope = Extension.create<object, MathScopeStorage>({
  name: 'mathScope',
  addStorage() {
    return { version: shallowRef(0), scopes: new Map(), signature: '' }
  },
  onCreate() {
    this.storage.stop = watch(workspacePreamble, () => this.storage.rebuild?.())
  },
  onDestroy() {
    this.storage.stop?.()
  },
  addProseMirrorPlugins() {
    const storage = this.storage
    const bump = () => { storage.version.value++ }
    const build = (doc: PMNode) => {
      // Le préambule de l'espace sert de point de départ (comme un \input commun).
      const base = scopeBuilder().next(workspacePreamble.value)
      const builder = scopeBuilder(base.macros, base.needsMhchem)
      const scopes = new Map<number, Scope>()
      const parts: string[] = []
      let seen = 0
      let mhchem = false
      doc.descendants((node, pos) => {
        if (!MATH.has(node.type.name)) return
        const s = builder.next(nodeSource(node))
        scopes.set(pos, s)
        // Signature : déclarations de chaque formule et nombre de déclarations qui la précèdent.
        parts.push(`${seen}:${s.parsed.statements.map((x) => x.source).join('\u0001')}`)
        seen += s.parsed.statements.length
        mhchem ||= s.needsMhchem
      })
      storage.scopes = scopes
      const signature = `${workspacePreamble.value}\u0003${parts.join('\u0002')}`
      if (signature !== storage.signature) {
        storage.signature = signature
        bump()
      }
      if (mhchem || base.needsMhchem) ensureMhchem(bump)
    }
    const editor = this.editor
    storage.rebuild = () => build(editor.state.doc)
    return [
      new Plugin({
        key: new PluginKey('mathScope'),
        state: {
          init: (_config, state) => build(state.doc),
          apply: (tr, value, _old, state) => {
            if (tr.docChanged) build(state.doc)
            return value
          },
        },
      }),
    ]
  },
})

/** Portée (réactive) d'une formule dans sa vue de nœud. */
export function useNodeScope(props: { editor: any; node: any; getPos: () => number | undefined }) {
  const storage = props.editor.storage?.mathScope as MathScopeStorage | undefined
  // Nouvel objet à chaque version : les rendus dépendants sont relancés même si la
  // portée est inchangée (ex. extension mhchem chargée entre-temps).
  return computed<{ scope: Scope | undefined; version: number }>(() => {
    const version = storage?.version.value ?? 0
    void props.node
    const pos = props.getPos()
    return { scope: pos == null ? undefined : storage?.scopes.get(pos), version }
  })
}
