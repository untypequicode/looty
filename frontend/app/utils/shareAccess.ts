import { fold } from './fold.ts'

/** Niveaux de partage d'une page, du plus bas au plus haut ; `deny` retire l'accès. */
export type ShareLevel = 'read' | 'comment' | 'edit' | 'manage' | 'deny'
const RANK: Record<string, number> = { none: 0, deny: 0, read: 1, comment: 2, edit: 4, manage: 5 }
export const levelRank = (l: string) => RANK[l] ?? 0

/** D'où vient le droit d'une personne : directement, d'une page parente, d'un groupe ou de son rôle dans l'espace. */
export type ShareOrigin = 'direct' | 'inherited' | 'group' | 'space'

export interface AccessPerson {
  user_id: string
  name: string
  email: string
  avatar_url?: string | null
  level: ShareLevel
  origin: ShareOrigin
  is_member: boolean
  inherit: boolean
  allow_subpages?: boolean
  from?: { id: string | null; title: string } | null
  via_group?: { id: string; name: string } | null
}
export interface AccessGroup { group_id: string; name: string; member_count: number; level: ShareLevel; inherit: boolean; allow_subpages?: boolean; from?: { id: string | null; title: string } | null }
export interface AccessSummary {
  people: AccessPerson[]
  groups: AccessGroup[]
  counts: { total: number; outside: number; space: number }
  pending_requests: number
  directory: 'all' | 'shared' | 'email'
}

/** Donner « gestion » demande une confirmation : la personne pourra partager et supprimer. */
export const needsManageWarning = (level: string) => level === 'manage'

/** Lignes à montrer, groupées : partages de cette page, héritages, rôle dans l'espace. */
export function sectionsOf(people: AccessPerson[]) {
  return {
    direct: people.filter((p) => p.origin === 'direct' || p.origin === 'group'),
    inherited: people.filter((p) => p.origin === 'inherited'),
    space: people.filter((p) => p.origin === 'space'),
  }
}

/** Personnes extérieures à l'espace qui ont (au moins) la lecture. */
export const outsiders = (people: AccessPerson[]) => people.filter((p) => !p.is_member && p.level !== 'deny')

export interface MoveChange { user_id: string; name: string; before: string; after: string }
export type MoveKind = 'lost' | 'gained' | 'changed'
/** Perd l'accès, le gagne, ou change de niveau. */
export function moveKind(c: MoveChange): MoveKind {
  if (levelRank(c.after) === 0 && levelRank(c.before) > 0) return 'lost'
  if (levelRank(c.before) === 0 && levelRank(c.after) > 0) return 'gained'
  return 'changed'
}
/** Changements d'un déplacement, rangés : accès perdus, puis gagnés, puis niveaux modifiés. */
export function sortMoveChanges(changes: MoveChange[]): MoveChange[] {
  const order: Record<MoveKind, number> = { lost: 0, gained: 1, changed: 2 }
  return [...changes].sort((a, b) => order[moveKind(a)] - order[moveKind(b)] || a.name.localeCompare(b.name))
}

export interface SharedItem {
  id: string
  workspace_id: string
  workspace_name: string
  title: string
  icon?: string | null
  updated_at: string
  level: string
  root: boolean
  origin: 'direct' | 'group' | 'inherited'
  via_group?: string | null
  shared_by?: string | null
  from?: { id: string; title: string } | null
  path: { id: string; title: string }[]
  path_hidden: boolean
}

/** « Partagé avec moi » : filtré par recherche, groupé par espace (le plus récemment modifié en premier), pages par date. */
export function groupShared(items: SharedItem[], query = ''): { workspace_id: string; name: string; items: SharedItem[] }[] {
  const q = fold(query.trim())
  const match = (i: SharedItem) => !q || [i.title, i.workspace_name, i.shared_by ?? '', i.via_group ?? '', ...i.path.map((p) => p.title)].some((s) => fold(s).includes(q))
  const groups = new Map<string, { workspace_id: string; name: string; items: SharedItem[] }>()
  for (const i of items.filter(match).sort((a, b) => b.updated_at.localeCompare(a.updated_at))) {
    if (!groups.has(i.workspace_id)) groups.set(i.workspace_id, { workspace_id: i.workspace_id, name: i.workspace_name, items: [] })
    groups.get(i.workspace_id)!.items.push(i)
  }
  return [...groups.values()]
}

/** Chemin montré sous un titre : les parents visibles, précédés de « … » si d'autres sont masqués. */
export function pathLabel(i: Pick<SharedItem, 'path' | 'path_hidden'>): string {
  const parts = i.path.map((p) => p.title || '…')
  return (i.path_hidden ? ['…', ...parts] : parts).join(' / ')
}

/** Page d'un espace (`/w/{ws}/doc/{id}`) : son identifiant, pour proposer de demander l'accès quand elle est refusée. */
export function docIdOfPath(path: string): string | null {
  const m = /^\/w\/[^/]+\/doc\/([0-9a-f-]{36})(?:[/?#]|$)/i.exec(path)
  return m ? m[1]! : null
}
