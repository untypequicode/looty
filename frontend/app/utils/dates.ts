/**
 * Échéances : une date « jour » est stockée à midi UTC (sans heure) ; toute autre heure est
 * une heure précise choisie par l'utilisateur (ou le début d'un événement lié).
 */
export function isTimed(iso?: string | null): boolean {
  if (!iso) return false
  const d = new Date(iso)
  return !(d.getUTCHours() === 12 && d.getUTCMinutes() === 0 && d.getUTCSeconds() === 0)
}

/** Heure locale « HH:MM » d'une échéance horaire, sinon ''. */
export function timeOf(iso?: string | null): string {
  if (!iso || !isTimed(iso)) return ''
  const d = new Date(iso)
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
}
