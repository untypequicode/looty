// node --test frontend/tests/assistantDraft.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import {
  createDraftStore, parseDraft, withAttachment, emptyDraft, attachmentKey, attachmentPayload, isMoodleKind,
  moodleCourseAttachment, moodleFileAttachment, MAX_ATTACHMENTS, DRAFT_KEY, draftKeyFor, type DraftStorage,
} from '../app/utils/assistantDraft.ts'

const memory = (): DraftStorage & { data: Map<string, string> } => {
  const data = new Map<string, string>()
  return { data, getItem: (k) => data.get(k) ?? null, setItem: (k, v) => { data.set(k, v) } }
}
const page = (id: string) => ({ kind: 'page' as const, id, title: `Page ${id}`, ws: 'w1' })

test('un brouillon illisible ou mal formé redevient vide', () => {
  assert.deepEqual(parseDraft(null), emptyDraft())
  assert.deepEqual(parseDraft('pas du json'), emptyDraft())
  assert.deepEqual(parseDraft('42'), emptyDraft())
  const d = parseDraft(JSON.stringify({ text: 'bonjour', attachments: [page('1'), { kind: 'inconnu', id: 'x', title: 't' }, { kind: 'page', id: 3, title: 't' }, null] }))
  assert.equal(d.text, 'bonjour')
  assert.deepEqual(d.attachments.map((a) => a.id), ['1'])
})

test('une puce ne s’ajoute qu’une fois et pas au-delà de dix', () => {
  let d = emptyDraft()
  assert.equal(withAttachment(d, page('1')).result, 'added')
  d = withAttachment(d, page('1')).draft
  assert.equal(withAttachment(d, page('1')).result, 'duplicate')
  // Même identifiant mais autre type : une autre puce.
  assert.equal(withAttachment(d, { kind: 'study', id: '1', title: 'S' }).result, 'added')
  for (let i = 0; i < MAX_ATTACHMENTS; i++) d = withAttachment(d, page(`p${i}`)).draft
  assert.equal(d.attachments.length, MAX_ATTACHMENTS)
  assert.equal(withAttachment(d, page('autre')).result, 'full')
})

test('ajouter une puce garde le texte déjà tapé et sert aussi à un autre onglet', () => {
  const storage = memory()
  const here = createDraftStore(storage)
  here.setText('Résume le chapitre 2')
  // Un autre onglet (même stockage) envoie un cours Moodle à l'IA.
  const other = createDraftStore(storage)
  assert.equal(other.get().text, 'Résume le chapitre 2')
  assert.equal(other.add(moodleCourseAttachment('w1', { id: 7, name: 'Algèbre' })), 'added')
  assert.equal(other.get().text, 'Résume le chapitre 2')
  // L'onglet de l'Assistant relit le stockage (événement « storage ») et voit la puce, avec son texte.
  let seen = 0
  here.subscribe(() => { seen++ })
  here.reload()
  assert.equal(seen, 1)
  assert.equal(here.get().text, 'Résume le chapitre 2')
  assert.deepEqual(here.get().attachments.map(attachmentKey), ['moodle_course:w1:7'])
  // Rien n'a changé : pas de notification inutile.
  here.reload()
  assert.equal(seen, 1)
})

test('le brouillon survit à un rechargement et se vide à l’envoi', () => {
  const storage = memory()
  const a = createDraftStore(storage)
  a.setText('texte')
  a.add(page('1'))
  a.add(page('2'))
  const reloaded = createDraftStore(storage)
  assert.equal(reloaded.get().text, 'texte')
  assert.equal(reloaded.get().attachments.length, 2)
  reloaded.remove(page('1'))
  assert.deepEqual(reloaded.get().attachments.map((x) => x.id), ['2'])
  reloaded.clear()
  assert.deepEqual(JSON.parse(storage.data.get(DRAFT_KEY)!), emptyDraft())
})

test('les abonnés sont prévenus et peuvent se désabonner', () => {
  const s = createDraftStore(memory())
  const calls: string[] = []
  const off = s.subscribe((d) => calls.push(d.text))
  s.setText('a')
  s.setText('a') // inchangé
  s.setText('ab')
  off()
  s.setText('abc')
  assert.deepEqual(calls, ['a', 'ab'])
})

test('sans stockage le brouillon reste en mémoire', () => {
  const s = createDraftStore(null)
  s.setText('x')
  assert.equal(s.add(page('1')), 'added')
  assert.equal(s.get().text, 'x')
  const broken = createDraftStore({ getItem: () => null, setItem: () => { throw new Error('plein') } })
  assert.doesNotThrow(() => broken.setText('y'))
  assert.equal(broken.get().text, 'y')
})

test('cours et fichiers Moodle : identité, espace et corps envoyé au serveur', () => {
  const course = moodleCourseAttachment('w1', { id: 7, name: 'Algèbre' })
  assert.equal(course.kind, 'moodle_course')
  assert.equal(course.ws, 'w1')
  const file = moodleFileAttachment('w1', { id: 7, name: 'Algèbre' }, { name: 'poly.pdf', source: 'https://m.test/pluginfile.php/1/poly.pdf', size: 2048 })
  assert.equal(file.courseName, 'Algèbre')
  assert.notEqual(attachmentKey(course), attachmentKey(file))
  // Le serveur calcule les identifiants Moodle : le corps n'en porte pas, mais l'espace, le cours et l'adresse.
  assert.deepEqual(attachmentPayload(course), { kind: 'moodle_course', title: 'Algèbre', workspace_id: 'w1', course_id: 7, url: undefined, course_title: undefined })
  assert.deepEqual(attachmentPayload(file), { kind: 'moodle_file', title: 'poly.pdf', workspace_id: 'w1', course_id: 7, url: 'https://m.test/pluginfile.php/1/poly.pdf', course_title: 'Algèbre' })
  assert.deepEqual(attachmentPayload(page('9')), { kind: 'page', id: '9', title: 'Page 9' })
  assert.ok(isMoodleKind('moodle_file') && isMoodleKind('moodle_course') && !isMoodleKind('file'))
})

test("chaque personne a son brouillon dans un navigateur partagé", () => {
  const storage = memory()
  const alice = createDraftStore(storage, draftKeyFor("alice"))
  alice.setText("secret d'Alice")
  alice.add(page("1"))
  const bob = createDraftStore(storage, draftKeyFor("bob"))
  assert.deepEqual(bob.get(), emptyDraft())
  bob.setText("texte de Bob")
  assert.equal(createDraftStore(storage, draftKeyFor("alice")).get().text, "secret d'Alice")
  assert.notEqual(draftKeyFor("alice"), draftKeyFor("bob"))
  // L'ancienne clé commune n'est jamais relue.
  storage.setItem("looty.ai.draft", JSON.stringify({ text: "ancien", attachments: [] }))
  assert.equal(createDraftStore(storage, draftKeyFor("carol")).get().text, "")
  // Sans personne connectée (stockage nul) : rien n'est gardé.
  const anon = createDraftStore(null)
  anon.setText("x")
  assert.equal(createDraftStore(null).get().text, "")
})
