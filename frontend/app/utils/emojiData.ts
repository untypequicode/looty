/**
 * Jeu d'émojis complet (emojibase) avec noms et mots-clés français et anglais,
 * chargé à la demande (deux fichiers JSON séparés du reste de l'application).
 */
export interface EmojiItem {
  emoji: string
  en: string
  fr: string
  /** Libellés + mots-clés (fr et en), en minuscules, pour la recherche. */
  search: string
  group: number
}

/** Groupes emojibase affichés (le groupe 2 « composants » est ignoré). */
export const EMOJI_GROUPS = [0, 1, 3, 4, 5, 6, 7, 8, 9] as const

let cache: Promise<EmojiItem[]> | null = null
const fold = (s: string) => s.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '')

export function loadEmojis(): Promise<EmojiItem[]> {
  cache ??= Promise.all([import('emojibase-data/en/compact.json'), import('emojibase-data/fr/compact.json')]).then(([en, fr]) => {
    const frByHex = new Map<string, any>(((fr as any).default ?? fr).map((e: any) => [e.hexcode, e]))
    return ((en as any).default ?? en)
      .filter((e: any) => typeof e.group === 'number' && e.group !== 2)
      .sort((a: any, b: any) => (a.order ?? 0) - (b.order ?? 0))
      .map((e: any) => {
        const f = frByHex.get(e.hexcode)
        return {
          emoji: e.unicode,
          en: e.label,
          fr: f?.label ?? e.label,
          search: fold([e.label, f?.label, ...(e.tags ?? []), ...(f?.tags ?? [])].filter(Boolean).join(' ')),
          group: e.group,
        }
      })
  })
  return cache
}

export function searchEmojis(all: EmojiItem[], query: string, limit = 400): EmojiItem[] {
  const words = fold(query.trim()).split(/\s+/).filter(Boolean)
  if (!words.length) return all
  return all
    .filter((e) => words.every((w) => e.search.includes(w)))
    .sort((a, b) => Number(!(fold(a.fr).startsWith(words[0]!) || fold(a.en).startsWith(words[0]!))) - Number(!(fold(b.fr).startsWith(words[0]!) || fold(b.en).startsWith(words[0]!))))
    .slice(0, limit)
}

const RECENT = 'looty.emoji.recent'
export function recentEmojis(): string[] {
  try { return JSON.parse(localStorage.getItem(RECENT) || '[]').slice(0, 24) } catch { return [] }
}
export function pushRecentEmoji(e: string) {
  const list = [e, ...recentEmojis().filter((x) => x !== e)].slice(0, 24)
  localStorage.setItem(RECENT, JSON.stringify(list))
}
