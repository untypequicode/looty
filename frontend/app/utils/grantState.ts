// État d'une carte « Autoriser » de l'Assistant, d'après les autorisations de la discussion.

/** Autorisation terminée (échue ou retirée), `key` comme la clé des cartes (p:, u:, c:). */
export interface EndedGrant { key: string; created_at: string; ended_at: string }

/** Clé d'une demande : une carte par cible (page, jeu modifié, création dans un espace). */
export function needKey(n: { kind: 'page'; page_id: string } | { kind: 'study_update'; set_id: string } | { kind: 'study_create'; personal: boolean; workspace_id: string | null }): string {
  return n.kind === 'page' ? `p:${n.page_id}` : n.kind === 'study_update' ? `u:${n.set_id}` : `c:${n.personal ? 'personal' : n.workspace_id}`
}

/**
 * - `active` : une autorisation en cours couvre la cible.
 * - `expired` : une autorisation a été donnée après cette carte et elle est terminée : la carte
 *   a servi, elle ne doit plus proposer d'autoriser (une nouvelle demande aura sa propre carte).
 * - `pending` : rien n'a été accordé depuis la carte.
 *
 * `cardAt` : date du message de la carte (`undefined` pour une carte reçue en direct : maintenant).
 */
export function cardGrantState(key: string, cardAt: string | undefined, activeKeys: ReadonlySet<string>, ended: readonly EndedGrant[]): 'active' | 'expired' | 'pending' {
  if (activeKeys.has(key)) return 'active'
  const at = cardAt ? Date.parse(cardAt) : Date.now()
  // Une petite marge : la carte est enregistrée juste avant l'autorisation qu'elle déclenche.
  const used = ended.some((g) => g.key === key && Date.parse(g.created_at) >= at - 1000)
  return used ? 'expired' : 'pending'
}
