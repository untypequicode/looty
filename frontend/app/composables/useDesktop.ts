import { ref } from 'vue'

/** Page servie par l'application de bureau (balise ajoutée par son proxy local). */
export const isDesktop = typeof document !== 'undefined' && !!document.querySelector('meta[name="looty-desktop"]')

export interface NetState {
  online: boolean
  syncing: boolean
  pending: number
  failed: { label: string; status: number }[]
}

/** État réseau de l'application de bureau (hors ligne, modifications en attente). */
export const net = ref<NetState>({ online: true, syncing: false, pending: 0, failed: [] })

/** Ouvre une page de Looty dans l'application de bureau (lien looty://w/…). */
export function openInDesktop(path: string) {
  window.location.href = `looty:/${path}`
}
