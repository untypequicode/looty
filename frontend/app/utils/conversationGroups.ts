// Regroupement par date de la liste des conversations de l'Assistant.

export type ConversationBucket = 'today' | 'week' | 'older'

/** Rubrique d'une conversation modifiée à `updatedAt` : aujourd'hui, les 7 derniers jours (aujourd'hui compris), plus ancien. */
export function bucketOf(updatedAt: string | number | Date, now: Date = new Date()): ConversationBucket {
  const at = +new Date(updatedAt)
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime()
  // Calendrier local : un changement d'heure ne décale pas la limite.
  const weekStart = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 6).getTime()
  return at >= startOfToday ? 'today' : at >= weekStart ? 'week' : 'older'
}

/** Conversations rangées par rubrique, dans l'ordre reçu ; les rubriques vides sont omises. */
export function groupConversations<T extends { updated_at: string }>(items: readonly T[], now: Date = new Date()): { key: ConversationBucket; items: T[] }[] {
  const out: { key: ConversationBucket; items: T[] }[] = [
    { key: 'today', items: [] },
    { key: 'week', items: [] },
    { key: 'older', items: [] },
  ]
  for (const c of items) out.find((g) => g.key === bucketOf(c.updated_at, now))!.items.push(c)
  return out.filter((g) => g.items.length)
}
