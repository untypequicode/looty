// Importer depuis Notion, Obsidian et Trello : les parties sans interface ni appel au serveur
// (noms de fichiers Notion, liens [[wiki]], CSV, export JSON de Trello). Les tests sont dans
// frontend/tests/importers.test.ts ; l'assemblage est dans editor/outline.ts et editor/trelloImport.ts.

/** Ce qui n'a pas pu être repris, pour le rapport d'import. `kind` désigne le message, `detail` l'élément. */
export interface ImportIssue { kind: 'archivedCard' | 'archivedList' | 'member' | 'attachment' | 'link' | 'image' | 'file' | 'csv'; detail: string }

// --- Notion ------------------------------------------------------------------------------------

const NOTION_ID = /\s+[0-9a-f]{32}$/i

/** « Page d'accueil 4a2f…(32 hex) » -> « Page d'accueil » (Notion ajoute l'identifiant à chaque nom). */
export const stripNotionId = (name: string) => name.replace(NOTION_ID, '').trim() || name

/** Vrai si des chemins portent des identifiants Notion (dossier ou fichier se terminant par 32 caractères hexadécimaux). */
export function hasNotionIds(paths: string[]): boolean {
  return paths.some((p) => p.split('/').some((seg) => NOTION_ID.test(seg.replace(/\.[a-z0-9]+$/i, ''))))
}

// --- Markdown : en-tête YAML et liens Obsidian ---------------------------------------------------

/** Retire l'en-tête YAML (---) ; renvoie son `title:` s'il y en a un. */
export function splitFrontMatter(md: string): { body: string; title: string | null } {
  const m = /^﻿?---[ \t]*\r?\n([\s\S]*?)\r?\n(?:---|\.\.\.)[ \t]*(?:\r?\n|$)/.exec(md)
  if (!m) return { body: md, title: null }
  const t = /^title:[ \t]*(.+?)[ \t]*$/m.exec(m[1]!)?.[1]?.replace(/^(["'])(.*)\1$/, '$2').trim()
  return { body: md.slice(m[0].length), title: t || null }
}

/** Chemin relatif de `to` depuis le dossier `fromDir` (sans « / » initial), avec « ../ » si besoin. */
export function relativePath(fromDir: string, to: string): string {
  const a = fromDir.split('/').filter(Boolean)
  const b = to.split('/').filter(Boolean)
  let i = 0
  while (i < a.length && i < b.length - 1 && a[i] === b[i]) i++
  return [...a.slice(i).map(() => '..'), ...b.slice(i)].join('/')
}

const urlPath = (p: string) => p.split('/').map((s) => encodeURIComponent(s).replace(/[()]/g, (c) => (c === '(' ? '%28' : '%29'))).join('/')
const IMAGE = /\.(png|jpe?g|gif|webp|svg|avif|bmp)$/i

/**
 * `[[Page]]`, `[[Page|texte]]`, `[[Page#Titre]]`, `![[image.png]]` et `![[Page]]` deviennent des liens Markdown
 * relatifs, que l'import sait ensuite résoudre. `resolve` donne le chemin relatif du fichier cité (ou null).
 * Le code (blocs et en ligne) n'est pas touché.
 */
export function wikiToMarkdown(md: string, resolve: (target: string) => string | null, issues: ImportIssue[]): string {
  const convert = (chunk: string) =>
    chunk.replace(/(`[^`\n]*`)|(!?)\[\[([^\]\n|]*)(?:\|([^\]\n]*))?\]\]/g, (whole, code: string | undefined, bang: string, rawTarget: string, alias?: string) => {
      if (code) return code
      const target = rawTarget.split(/[#^]/)[0]!.trim()
      const heading = rawTarget.includes('#') ? rawTarget.slice(rawTarget.indexOf('#') + 1).trim() : ''
      const size = alias != null && /^\d+(x\d+)?$/.test(alias.trim())
      const label = (!size && alias?.trim()) || (target ? target.split('/').pop()! : heading) || whole
      // Lien vers un titre du même document : on garde le texte.
      if (!target) return label
      const path = resolve(target)
      if (!path) {
        issues.push({ kind: bang && IMAGE.test(target) ? 'image' : 'link', detail: target })
        return label
      }
      const url = urlPath(path)
      if (bang && IMAGE.test(path)) return `![${size ? target.split('/').pop() : label}](${url})`
      return `[${label}](${url})`
    })
  // Les blocs de code (``` ou ~~~) passent tels quels.
  return md
    .split(/^((?:```|~~~)[^\n]*\n[\s\S]*?\n(?:```|~~~)[ \t]*$)/m)
    .map((part, i) => (i % 2 ? part : convert(part)))
    .join('')
}

// --- CSV (bases Notion) -----------------------------------------------------------------------------

/** CSV (RFC 4180) : guillemets doublés, retours à la ligne dans les cellules, BOM ignoré. */
export function parseCsv(text: string): string[][] {
  const rows: string[][] = []
  let row: string[] = []
  let cell = ''
  let quoted = false
  const src = text.replace(/^﻿/, '')
  for (let i = 0; i < src.length; i++) {
    const c = src[i]!
    if (quoted) {
      if (c === '"' && src[i + 1] === '"') { cell += '"'; i++ }
      else if (c === '"') quoted = false
      else cell += c
    } else if (c === '"') quoted = true
    else if (c === ',') { row.push(cell); cell = '' }
    else if (c === '\n' || c === '\r') {
      if (c === '\r' && src[i + 1] === '\n') i++
      row.push(cell); cell = ''
      rows.push(row); row = []
    } else cell += c
  }
  if (cell || row.length) { row.push(cell); rows.push(row) }
  return rows.filter((r) => r.some((c) => c.trim()))
}

const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')

/** Table HTML d'un CSV : première ligne en en-tête. */
export function csvToHtml(rows: string[][]): string {
  if (!rows.length) return ''
  const width = Math.max(...rows.map((r) => r.length))
  const cells = (r: string[], tag: string) => `<tr>${Array.from({ length: width }, (_, i) => `<${tag}><p>${esc(r[i] ?? '')}</p></${tag}>`).join('')}</tr>`
  return `<table><tbody>${cells(rows[0]!, 'th')}${rows.slice(1).map((r) => cells(r, 'td')).join('')}</tbody></table>`
}

// --- Trello -----------------------------------------------------------------------------------------

export interface TrelloCard {
  name: string
  desc: string
  /** Date ISO ou null. */
  due: string | null
  done: boolean
  labels: { name: string; color: string }[]
  checklists: { name: string; items: { name: string; done: boolean }[] }[]
  members: { fullName: string; username: string; email: string }[]
  attachments: { name: string; url: string }[]
  comments: { author: string; date: string; text: string }[]
}
export interface TrelloPlan {
  name: string
  lists: { name: string; cards: TrelloCard[] }[]
  issues: ImportIssue[]
}

/** Couleurs des étiquettes Trello (les variantes « _dark » / « _light » prennent la couleur de base). */
const TRELLO_COLORS: Record<string, string> = {
  green: '#4bce97', yellow: '#e2b203', orange: '#fea362', red: '#f87168', purple: '#9f8fef', blue: '#579dff',
  sky: '#6cc3e0', lime: '#94c748', pink: '#e774bb', black: '#8590a2',
}
export const trelloColor = (c: string | null | undefined) => TRELLO_COLORS[(c ?? '').split('_')[0]!] ?? '#8590a2'

export function isTrelloExport(data: any): boolean {
  return !!data && typeof data === 'object' && Array.isArray(data.lists) && Array.isArray(data.cards) && typeof data.name === 'string'
}

const byPos = (a: any, b: any) => (Number(a?.pos) || 0) - (Number(b?.pos) || 0)

/** Export JSON d'un tableau Trello (menu du tableau > Imprimer et exporter > JSON). */
export function parseTrello(data: any): TrelloPlan {
  const issues: ImportIssue[] = []
  const labelOf = new Map<string, { name: string; color: string }>((data.labels ?? []).map((l: any) => [l.id, { name: String(l.name || l.color || ''), color: String(l.color ?? '') }]))
  const memberOf = new Map<string, any>((data.members ?? []).map((m: any) => [m.id, m]))
  const checklistsOf = new Map<string, any[]>()
  for (const cl of (data.checklists ?? []).slice().sort(byPos)) (checklistsOf.get(cl.idCard) ?? checklistsOf.set(cl.idCard, []).get(cl.idCard)!).push(cl)
  const commentsOf = new Map<string, TrelloCard['comments']>()
  for (const a of (data.actions ?? []).slice().sort((x: any, y: any) => String(x.date).localeCompare(String(y.date)))) {
    if (a.type !== 'commentCard' || !a.data?.card?.id || !a.data?.text) continue
    const list = commentsOf.get(a.data.card.id) ?? commentsOf.set(a.data.card.id, []).get(a.data.card.id)!
    list.push({ author: a.memberCreator?.fullName || a.memberCreator?.username || '', date: String(a.date ?? ''), text: String(a.data.text) })
  }
  const lists: TrelloPlan['lists'] = []
  for (const l of (data.lists ?? []).slice().sort(byPos)) {
    if (l.closed) { issues.push({ kind: 'archivedList', detail: String(l.name) }); continue }
    const cards: TrelloCard[] = []
    for (const c of (data.cards ?? []).filter((x: any) => x.idList === l.id).sort(byPos)) {
      if (c.closed) { issues.push({ kind: 'archivedCard', detail: String(c.name) }); continue }
      cards.push({
        name: String(c.name || '—'),
        desc: String(c.desc ?? ''),
        due: c.due ? String(c.due) : null,
        done: !!c.dueComplete,
        labels: (c.idLabels ?? []).map((id: string) => labelOf.get(id)).filter(Boolean) as TrelloCard['labels'],
        checklists: (checklistsOf.get(c.id) ?? []).map((cl) => ({
          name: String(cl.name ?? ''),
          items: (cl.checkItems ?? []).slice().sort(byPos).map((it: any) => ({ name: String(it.name ?? ''), done: it.state === 'complete' })),
        })),
        members: (c.idMembers ?? []).map((id: string) => memberOf.get(id)).filter(Boolean).map((m: any) => ({ fullName: String(m.fullName ?? ''), username: String(m.username ?? ''), email: String(m.email ?? '') })),
        attachments: (c.attachments ?? []).filter((a: any) => a?.url).map((a: any) => ({ name: String(a.name || a.url), url: String(a.url) })),
        comments: commentsOf.get(c.id) ?? [],
      })
    }
    lists.push({ name: String(l.name || '—'), cards })
  }
  return { name: String(data.name || 'Trello'), lists, issues }
}

const norm = (s: string) => s.normalize('NFKD').replace(/[̀-ͯ]/g, '').toLowerCase().replace(/\s+/g, ' ').trim()

/** Membre Looty correspondant à un membre Trello : par e-mail, puis nom complet, puis identifiant Trello = début de l'e-mail. */
export function matchMember(m: TrelloCard['members'][number], members: { user_id: string; name: string; email: string }[]): string | null {
  const email = m.email.trim().toLowerCase()
  if (email) { const hit = members.find((x) => x.email.toLowerCase() === email); if (hit) return hit.user_id }
  if (m.fullName) { const hit = members.find((x) => norm(x.name) === norm(m.fullName)); if (hit) return hit.user_id }
  if (m.username) { const hit = members.find((x) => x.email.split('@')[0]!.toLowerCase() === m.username.toLowerCase()); if (hit) return hit.user_id }
  return null
}

/** Texte d'un commentaire Trello en HTML : auteur et date en tête, texte échappé. */
export function commentHtml(c: TrelloCard['comments'][number]): string {
  const when = c.date ? c.date.slice(0, 10) : ''
  const head = [c.author, when].filter(Boolean).join(' · ')
  return `${head ? `<p><strong>${esc(head)}</strong></p>` : ''}${c.text.split(/\n{2,}/).map((p) => `<p>${esc(p).replace(/\n/g, '<br>')}</p>`).join('')}`
}
