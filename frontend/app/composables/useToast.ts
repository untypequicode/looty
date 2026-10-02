import { ref } from 'vue'

export type ToastKind = 'info' | 'success' | 'error'
export interface Toast {
  id: number
  kind: ToastKind
  message: string
  action?: { label: string; run: () => void }
  duration: number
}

const toasts = ref<Toast[]>([])
let seq = 0

export function toast(message: string, opts: Partial<Omit<Toast, 'id' | 'message'>> = {}) {
  const t: Toast = { id: ++seq, message, kind: opts.kind ?? 'info', action: opts.action, duration: opts.duration ?? 4000 }
  toasts.value = [...toasts.value.slice(-4), t]
  if (t.duration > 0) setTimeout(() => dismissToast(t.id), t.duration)
  return t.id
}
toast.success = (m: string, o: Partial<Toast> = {}) => toast(m, { ...o, kind: 'success' })
toast.error = (m: string, o: Partial<Toast> = {}) => toast(m, { ...o, kind: 'error' })

export function dismissToast(id: number) {
  toasts.value = toasts.value.filter((t) => t.id !== id)
}

export function useToasts() {
  return { toasts, toast, dismissToast }
}
