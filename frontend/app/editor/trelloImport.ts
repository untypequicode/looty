/**
 * Création d'un tableau Looty à partir d'un export JSON de Trello (voir utils/importers.ts pour la lecture) :
 * listes en colonnes, cartes en tâches (description, échéance, étiquettes, personnes), listes de contrôle en
 * sous-tâches, pièces jointes en liens dans la description, commentaires conservés avec leur auteur.
 */
import { api } from '~/composables/api'
import { commentHtml, matchMember, trelloColor, type ImportIssue, type TrelloPlan } from '~/utils/importers'
import { markdownToHtml } from './outline'

interface Created { id: string }

export interface TrelloImportOptions {
  wsId: string
  members: { user_id: string; name: string; email: string }[]
  onProgress?: (done: number, total: number) => void
  issues: ImportIssue[]
}

const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')

/** Crée le tableau ; renvoie son identifiant et le nombre de tâches créées. */
export async function runTrelloImport(plan: TrelloPlan, o: TrelloImportOptions): Promise<{ boardId: string; tasks: number }> {
  const cards = plan.lists.flatMap((l) => l.cards)
  const total = 1 + plan.lists.length + cards.length
  let done = 0
  const tick = () => o.onProgress?.(++done, total)

  const board = await api<Created>(`/api/workspaces/${o.wsId}/boards`, { method: 'POST', body: { name: plan.name.slice(0, 120), with_default_columns: false } })
  tick()

  // Étiquettes de tout le tableau, créées une fois (nom + couleur).
  const labelIds = new Map<string, string>()
  async function label(l: { name: string; color: string }) {
    const key = `${l.name}\u0000${l.color}`
    if (!labelIds.has(key)) {
      const created = await api<Created>(`/api/boards/${board.id}/labels`, { method: 'POST', body: { name: l.name.slice(0, 60), color: trelloColor(l.color) } })
      labelIds.set(key, created.id)
    }
    return labelIds.get(key)!
  }

  let tasks = 0
  const unmatched = new Set<string>()
  for (const list of plan.lists) {
    const column = await api<Created>(`/api/boards/${board.id}/columns`, { method: 'POST', body: { name: list.name.slice(0, 80) } })
    tick()
    for (const card of list.cards) {
      const assignees: string[] = []
      for (const m of card.members) {
        const id = matchMember(m, o.members)
        if (id) { if (!assignees.includes(id)) assignees.push(id) }
        else unmatched.add(m.fullName || m.username)
      }
      let description = card.desc.trim() ? markdownToHtml(card.desc).html : ''
      // Les pièces jointes restent chez Trello : un lien vers chacune.
      if (card.attachments.length) {
        description += `<ul>${card.attachments.map((a) => `<li><a href="${esc(a.url)}">${esc(a.name)}</a></li>`).join('')}</ul>`
        for (const a of card.attachments) o.issues.push({ kind: 'attachment', detail: `${card.name} — ${a.name}` })
      }
      const task = await api<Created>(`/api/boards/columns/${column.id}/tasks`, {
        method: 'POST',
        body: {
          title: card.name.slice(0, 255),
          description: description || undefined,
          due_date: card.due ?? undefined,
          labels: await Promise.all(card.labels.map(label)),
          assignees,
        },
      })
      tasks++
      if (card.done) await api(`/api/boards/tasks/${task.id}`, { method: 'PUT', body: { is_completed: true } })
      // Listes de contrôle : des sous-tâches (le nom de la liste précède l'élément quand il y en a plusieurs).
      const many = card.checklists.length > 1
      for (const cl of card.checklists) {
        for (const item of cl.items) {
          const title = (many && cl.name ? `${cl.name} : ${item.name}` : item.name).slice(0, 255)
          if (!title.trim()) continue
          const sub = await api<Created>(`/api/boards/tasks/${task.id}/subtasks`, { method: 'POST', body: { title } })
          if (item.done) await api(`/api/boards/tasks/${sub.id}`, { method: 'PUT', body: { is_completed: true } })
        }
      }
      for (const c of card.comments) await api(`/api/boards/tasks/${task.id}/comments`, { method: 'POST', body: { content: commentHtml(c) } })
      tick()
    }
  }
  for (const m of unmatched) o.issues.push({ kind: 'member', detail: m })
  return { boardId: board.id, tasks }
}
