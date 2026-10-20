/** Texte comparable sans casse ni accents (« Équipe » est trouvé en tapant « equ »). */
export function fold(s: string): string {
  return s.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '')
}
