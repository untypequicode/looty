// node --test frontend/tests/chatFiles.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { checkFile, dropIntent, fileKind, isExpired, isReadable, MAX_FILE_BYTES, pageAttachment, pastedFiles, uploadErrorKey } from '../app/utils/chatFiles.ts'

test('formats lisibles par l’assistant', () => {
  for (const n of ['cours.pdf', 'Rapport.DOCX', 'expose.pptx', 'notes.md', 'data.csv', 'main.rs', 'App.vue']) assert.ok(isReadable(n), n)
  for (const n of ['photo.png', 'archive.zip', 'film.mp4', 'tableur.xlsx', 'sans-extension']) assert.ok(!isReadable(n), n)
  assert.ok(isReadable('sans-extension', 'text/plain'))
  assert.ok(isReadable('download', 'application/pdf'))
})

test('contrôles avant envoi : format, taille, fichier vide', () => {
  assert.equal(checkFile({ name: 'a.pdf', size: 1000 }), null)
  assert.equal(checkFile({ name: 'a.png', size: 1000 }), 'unsupported')
  assert.equal(checkFile({ name: 'a.pdf', size: MAX_FILE_BYTES + 1 }), 'too_large')
  assert.equal(checkFile({ name: 'a.pdf', size: MAX_FILE_BYTES }), null)
  assert.equal(checkFile({ name: 'a.txt', size: 0 }), 'empty')
})

test('icône selon le type de fichier', () => {
  assert.equal(fileKind('a.pdf'), 'pdf')
  assert.equal(fileKind('a.docx'), 'doc')
  assert.equal(fileKind('a.pptx'), 'slides')
  assert.equal(fileKind('a.csv'), 'data')
  assert.equal(fileKind('a.py'), 'code')
  assert.equal(fileKind('a.txt'), 'text')
})

test('nature d’un glisser : fichiers, page de l’arbre ou rien', () => {
  assert.equal(dropIntent(['Files']), 'files')
  assert.equal(dropIntent(['text/plain', 'Files']), 'files')
  assert.equal(dropIntent(['application/x-looty-page']), 'page')
  assert.equal(dropIntent(['text/plain']), 'none')
  assert.equal(dropIntent(undefined), 'none')
})

test('une page ou un entraînement lâché devient une pièce jointe', () => {
  assert.deepEqual(pageAttachment({ ws: 'w', id: 'p', title: 'Cours' }), { kind: 'page', id: 'p', title: 'Cours', ws: 'w' })
  assert.deepEqual(pageAttachment({ ws: 'w', id: 's', study: true }), { kind: 'study', id: 's', title: '', ws: 'w' })
})

test('collage : seuls les fichiers sont gardés', () => {
  const f = { name: 'x.pdf' } as File
  const items = [{ kind: 'string', getAsFile: () => null }, { kind: 'file', getAsFile: () => f }, { kind: 'file', getAsFile: () => null }]
  assert.deepEqual(pastedFiles(items), [f])
  assert.deepEqual(pastedFiles(undefined), [])
})

test('péremption d’un fichier', () => {
  const now = Date.parse('2026-06-01T12:00:00Z')
  assert.ok(isExpired('2026-06-01T11:59:59Z', now))
  assert.ok(!isExpired('2026-06-01T12:00:01Z', now))
  assert.ok(!isExpired(null, now))
})

test('messages d’erreur de dépôt', () => {
  assert.equal(uploadErrorKey('file_no_text'), 'ai.files.err.file_no_text')
  assert.equal(uploadErrorKey('too_large'), 'ai.files.err.file_too_large')
  assert.equal(uploadErrorKey('unsupported'), 'ai.files.err.file_unsupported')
  assert.equal(uploadErrorKey('boom'), 'ai.files.err.other')
})
