import { watch, onBeforeUnmount, type Ref } from 'vue'

/**
 * Pile des calques ouverts (popovers, dialogues, panneaux...).
 * Échap ne ferme que le calque le plus haut ; les raccourcis globaux
 * sont désactivés tant qu'un calque modal est ouvert.
 */
const stack: { id: number; close: () => void; modal: boolean }[] = []
let seq = 0
let installed = false

function install() {
  if (installed || typeof window === 'undefined') return
  installed = true
  window.addEventListener(
    'keydown',
    (e) => {
      if (e.key !== 'Escape' || !stack.length) return
      // Laisse les suggestions de l'éditeur (/, @, :) gérer leur propre Échap.
      if (document.querySelector('[data-editor-suggestion-open]')) return
      e.preventDefault()
      e.stopImmediatePropagation()
      stack[stack.length - 1].close()
    },
    true,
  )
}

export function hasModalLayer() {
  return stack.some((l) => l.modal)
}

export function useLayer(open: Ref<boolean>, close: () => void, opts: { modal?: boolean } = {}) {
  install()
  let id = 0
  const remove = () => {
    const i = stack.findIndex((l) => l.id === id)
    if (i !== -1) stack.splice(i, 1)
  }
  watch(
    open,
    (v) => {
      if (v) {
        id = ++seq
        stack.push({ id, close, modal: !!opts.modal })
      } else remove()
    },
    { immediate: true },
  )
  onBeforeUnmount(remove)
}
