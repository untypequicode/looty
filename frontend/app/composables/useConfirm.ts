import { ref } from 'vue'

export interface ConfirmOptions {
  title: string
  message?: string
  confirmLabel?: string
  danger?: boolean
  /** Exige de retaper ce texte pour confirmer (suppressions irréversibles). */
  typeToConfirm?: string
  /** Champ de saisie (voir promptDialog). */
  prompt?: { initial?: string; placeholder?: string }
}

export const confirmState = ref<(ConfirmOptions & { resolve: (v: boolean) => void; value?: string }) | null>(null)

/** Boîte de confirmation asynchrone (remplace window.confirm). */
export function confirmDialog(opts: ConfirmOptions): Promise<boolean> {
  return new Promise((resolve) => {
    confirmState.value = { ...opts, resolve }
  })
}

/** Demande un texte (remplace window.prompt) ; null si annulé. */
export function promptDialog(opts: Omit<ConfirmOptions, 'prompt' | 'typeToConfirm'> & { initial?: string; placeholder?: string }): Promise<string | null> {
  return new Promise((resolve) => {
    // Le champ écrit dans `value` (même objet, via le proxy réactif).
    const st: ConfirmOptions & { resolve: (v: boolean) => void; value: string } = {
      ...opts, prompt: { initial: opts.initial, placeholder: opts.placeholder }, value: opts.initial ?? '',
      resolve: (ok) => resolve(ok ? st.value.trim() : null),
    }
    confirmState.value = st
  })
}
