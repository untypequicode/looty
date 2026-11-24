import { api } from './api'

/** Assistant de premier lancement : l'état n'est lu qu'une fois par chargement de page (puis après chaque étape terminée). */
export const setupFlag = { checked: false, needed: false }

export async function checkSetup(): Promise<boolean> {
  if (setupFlag.checked) return setupFlag.needed
  try {
    const s = await api<{ needed: boolean }>('/api/setup/state', { silent: true })
    setupFlag.needed = !!s.needed
  } catch {
    setupFlag.needed = false
  }
  setupFlag.checked = true
  return setupFlag.needed
}
