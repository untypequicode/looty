// node --test frontend/tests/importers.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import {
  stripNotionId, hasNotionIds, splitFrontMatter, relativePath, wikiToMarkdown, parseCsv, csvToHtml,
  isTrelloExport, parseTrello, matchMember, trelloColor, commentHtml, type ImportIssue,
} from '../app/utils/importers.ts'

test('Notion names lose their identifier', () => {
  assert.equal(stripNotionId('Projet Alpha 4a2f0c9e8b7d41a3b5c6d7e8f9a0b1c2'), 'Projet Alpha')
  assert.equal(stripNotionId('Sans identifiant'), 'Sans identifiant')
  assert.equal(stripNotionId('4a2f0c9e8b7d41a3b5c6d7e8f9a0b1c2'), '4a2f0c9e8b7d41a3b5c6d7e8f9a0b1c2')
  assert.ok(hasNotionIds(['Export/Page 4a2f0c9e8b7d41a3b5c6d7e8f9a0b1c2.md']))
  assert.ok(hasNotionIds(['Sujet 4a2f0c9e8b7d41a3b5c6d7e8f9a0b1c2/Sous page.md']))
  assert.ok(!hasNotionIds(['notes/idee.md', 'journal/2026-11-24.md']))
})

test('front matter is removed and its title kept', () => {
  assert.deepEqual(splitFrontMatter('---\ntitle: "Mon cours"\ntags: [a]\n---\n# Corps\n'), { body: '# Corps\n', title: 'Mon cours' })
  assert.deepEqual(splitFrontMatter('# Pas d’en-tête\n---\nx'), { body: '# Pas d’en-tête\n---\nx', title: null })
  assert.equal(splitFrontMatter('---\ntags: [a]\n---\ntexte').title, null)
})

test('relative paths climb out of the current folder', () => {
  assert.equal(relativePath('', 'a/b.md'), 'a/b.md')
  assert.equal(relativePath('a', 'a/b.md'), 'b.md')
  assert.equal(relativePath('a/c', 'a/b.md'), '../b.md')
  assert.equal(relativePath('x/y', 'z.md'), '../../z.md')
})

test('wiki links become relative Markdown links, code untouched', () => {
  const files: Record<string, string> = { 'page b': 'notes/Page B.md', 'photo.png': 'files/photo 1.png' }
  const issues: ImportIssue[] = []
  const resolve = (t: string) => files[t.toLowerCase()] ?? null
  const md = [
    'Voir [[Page B]] et [[Page B|ce texte]] puis [[Page B#Intro]].',
    '![[photo.png]] ![[photo.png|300]] ![[Absent.png]] [[Fantôme]] [[#Titre local]]',
    '`[[Page B]]` en code',
    '```',
    '[[Page B]]',
    '```',
  ].join('\n')
  const out = wikiToMarkdown(md, resolve, issues)
  assert.match(out, /Voir \[Page B\]\(notes\/Page%20B\.md\) et \[ce texte\]\(notes\/Page%20B\.md\) puis \[Page B\]\(notes\/Page%20B\.md\)\./)
  assert.match(out, /!\[photo\.png\]\(files\/photo%201\.png\) !\[photo\.png\]\(files\/photo%201\.png\)/)
  assert.match(out, /Absent\.png Fantôme Titre local/)
  assert.ok(out.includes('`[[Page B]]` en code'))
  assert.ok(out.includes('```\n[[Page B]]\n```'))
  assert.deepEqual(issues, [{ kind: 'image', detail: 'Absent.png' }, { kind: 'link', detail: 'Fantôme' }])
})

test('CSV with quotes and line breaks becomes a table', () => {
  const rows = parseCsv('﻿Nom,Note\r\n"Dupont, A.","a ""b""\nsuite"\r\nMartin,\r\n\r\n')
  assert.deepEqual(rows, [['Nom', 'Note'], ['Dupont, A.', 'a "b"\nsuite'], ['Martin', '']])
  const html = csvToHtml([['A', 'B'], ['<x>']])
  assert.ok(html.startsWith('<table>') && html.includes('<th><p>A</p></th>') && html.includes('<td><p>&lt;x&gt;</p></td><td><p></p></td>'))
  assert.equal(csvToHtml([]), '')
})

const board = {
  name: 'Projet',
  labels: [{ id: 'l1', name: 'Urgent', color: 'red_dark' }, { id: 'l2', name: '', color: 'green' }],
  members: [{ id: 'm1', fullName: 'Ana Dupont', username: 'anad' }],
  lists: [
    { id: 'L2', name: 'Fait', pos: 2 },
    { id: 'L1', name: 'À faire', pos: 1 },
    { id: 'L3', name: 'Vieille liste', pos: 3, closed: true },
  ],
  cards: [
    { id: 'c2', idList: 'L1', name: 'Deuxième', pos: 20, desc: '', idLabels: [], idMembers: [], idChecklists: [] },
    {
      id: 'c1', idList: 'L1', name: 'Première', pos: 10, desc: 'Du **texte**', due: '2026-12-01T10:00:00.000Z', dueComplete: true,
      idLabels: ['l1', 'l2'], idMembers: ['m1'], attachments: [{ name: 'spec.pdf', url: 'https://example.org/spec.pdf' }],
    },
    { id: 'c3', idList: 'L2', name: 'Archivée', pos: 1, closed: true },
  ],
  checklists: [
    { id: 'k2', idCard: 'c1', name: 'B', pos: 2, checkItems: [{ name: 'y', state: 'incomplete', pos: 1 }] },
    { id: 'k1', idCard: 'c1', name: 'A', pos: 1, checkItems: [{ name: 'second', state: 'complete', pos: 2 }, { name: 'first', state: 'incomplete', pos: 1 }] },
  ],
  actions: [
    { type: 'commentCard', date: '2026-11-02T08:00:00Z', data: { text: 'plus tard', card: { id: 'c1' } }, memberCreator: { fullName: 'Ben' } },
    { type: 'commentCard', date: '2026-11-01T08:00:00Z', data: { text: 'avant', card: { id: 'c1' } }, memberCreator: { username: 'ana' } },
    { type: 'updateCard', date: '2026-11-03T08:00:00Z', data: { card: { id: 'c1' } } },
  ],
}

test('a Trello export is recognised and read in order', () => {
  assert.ok(isTrelloExport(board) && !isTrelloExport({ collection: {}, documents: [] }) && !isTrelloExport(null))
  const plan = parseTrello(board)
  assert.equal(plan.name, 'Projet')
  assert.deepEqual(plan.lists.map((l) => [l.name, l.cards.map((c) => c.name)]), [['À faire', ['Première', 'Deuxième']], ['Fait', []]])
  const c = plan.lists[0]!.cards[0]!
  assert.equal(c.due, '2026-12-01T10:00:00.000Z')
  assert.ok(c.done)
  assert.deepEqual(c.labels, [{ name: 'Urgent', color: 'red_dark' }, { name: 'green', color: 'green' }])
  assert.deepEqual(c.checklists.map((k) => [k.name, k.items.map((i) => [i.name, i.done])]), [['A', [['first', false], ['second', true]]], ['B', [['y', false]]]])
  assert.deepEqual(c.attachments, [{ name: 'spec.pdf', url: 'https://example.org/spec.pdf' }])
  assert.deepEqual(c.comments.map((x) => [x.author, x.text]), [['ana', 'avant'], ['Ben', 'plus tard']])
  assert.deepEqual(plan.issues, [{ kind: 'archivedCard', detail: 'Archivée' }, { kind: 'archivedList', detail: 'Vieille liste' }])
})

test('Trello members are matched to workspace members', () => {
  const members = [{ user_id: 'u1', name: 'Ana Dupont', email: 'ana.d@example.org' }, { user_id: 'u2', name: 'Éloïse Martin', email: 'eloise@example.org' }]
  assert.equal(matchMember({ fullName: 'Autre', username: '', email: 'ANA.D@example.org' }, members), 'u1')
  assert.equal(matchMember({ fullName: 'eloise martin', username: '', email: '' }, members), 'u2')
  assert.equal(matchMember({ fullName: 'Inconnue', username: 'eloise', email: '' }, members), 'u2')
  assert.equal(matchMember({ fullName: 'Inconnue', username: 'zz', email: '' }, members), null)
})

test('label colours and comments', () => {
  assert.equal(trelloColor('green_light'), '#4bce97')
  assert.equal(trelloColor(null), '#8590a2')
  assert.equal(commentHtml({ author: 'A<b>', date: '2026-11-01T08:00:00Z', text: 'x\n\n<y>' }), '<p><strong>A&lt;b&gt; · 2026-11-01</strong></p><p>x</p><p>&lt;y&gt;</p>')
})
