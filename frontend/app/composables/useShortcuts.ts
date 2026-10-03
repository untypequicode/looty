import { onBeforeUnmount } from 'vue'
import { hasModalLayer } from './useLayer'

/**
 * Raccourcis clavier façon Linear.
 * - combinaisons : "mod+k", "shift+?" (mod = ⌘ sur Mac, Ctrl ailleurs)
 * - séquences : "g h" (G puis H en moins d'une seconde)
 * Ignorés pendant la saisie et quand un calque modal est ouvert (sauf `global`).
 */
export interface Shortcut {
  keys: string
  /** Retourner `false` = non géré : le raccourci suivant est essayé. */
  handler: (e: KeyboardEvent) => void | boolean
  /** Actif même dans un champ de saisie / sous une modale. */
  global?: boolean
}

const registry = new Set<Shortcut>()
let pending: string | null = null
let pendingTimer: ReturnType<typeof setTimeout> | null = null
let installed = false

const isMac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform)

function isTyping(e: KeyboardEvent) {
  const t = e.target as HTMLElement | null
  if (!t) return false
  return t.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(t.tagName)
}

function comboOf(e: KeyboardEvent) {
  const parts: string[] = []
  if (isMac ? e.metaKey : e.ctrlKey) parts.push('mod')
  if (e.altKey) parts.push('alt')
  if (e.shiftKey && (e.key.length > 1 || /^[a-z]$/i.test(e.key))) parts.push('shift')
  parts.push(e.key.toLowerCase())
  return parts.join('+')
}

function onKeydown(e: KeyboardEvent) {
  if (e.defaultPrevented || e.isComposing) return
  const typing = isTyping(e)
  const modal = hasModalLayer()
  const combo = comboOf(e)

  if (pending) {
    const seq = `${pending} ${combo}`
    pending = null
    if (pendingTimer) clearTimeout(pendingTimer)
    for (const s of registry) {
      if (s.keys === seq && !typing && !modal && s.handler(e) !== false) {
        e.preventDefault()
        return
      }
    }
  }

  for (const s of registry) {
    if (s.keys !== combo) continue
    if (!s.global && (typing || modal)) continue
    if (s.handler(e) === false) continue
    e.preventDefault()
    return
  }

  // Début de séquence possible ?
  if (!typing && !modal && !e.metaKey && !e.ctrlKey && !e.altKey) {
    const starts = [...registry].some((s) => s.keys.startsWith(`${combo} `))
    if (starts) {
      pending = combo
      pendingTimer = setTimeout(() => (pending = null), 1000)
    }
  }
}

export function useShortcuts(list: Shortcut[]) {
  if (typeof window === 'undefined') return
  if (!installed) {
    window.addEventListener('keydown', onKeydown)
    installed = true
  }
  for (const s of list) registry.add(s)
  onBeforeUnmount(() => list.forEach((s) => registry.delete(s)))
}
