import { ref } from 'vue'

export interface ConfirmOptions {
  title: string
  message?: string
  confirmLabel?: string
  danger?: boolean
  /** Exige de retaper ce texte pour confirmer (suppressions irréversibles). */
  typeToConfirm?: string
}

export const confirmState = ref<(ConfirmOptions & { resolve: (v: boolean) => void }) | null>(null)

/** Boîte de confirmation asynchrone (remplace window.confirm). */
export function confirmDialog(opts: ConfirmOptions): Promise<boolean> {
  return new Promise((resolve) => {
    confirmState.value = { ...opts, resolve }
  })
}
