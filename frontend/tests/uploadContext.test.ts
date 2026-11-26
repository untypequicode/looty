// node --test frontend/tests/uploadContext.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { uploadContext, uploadUrl } from '../app/utils/uploadContext.ts'

const WS = '11111111-1111-4111-8111-111111111111'
const PAGE = '22222222-2222-4222-8222-222222222222'

test('an upload from a page is bound to that page, from a space to the space, elsewhere to nobody', () => {
  assert.deepEqual(uploadContext(`/w/${WS}/doc/${PAGE}`), { page: PAGE })
  assert.deepEqual(uploadContext(`/w/${WS}/read/${PAGE}`), { page: PAGE })
  assert.deepEqual(uploadContext(`/shared/${PAGE}`), { page: PAGE })
  assert.deepEqual(uploadContext(`/w/${WS}/drive`), { workspace: WS })
  assert.deepEqual(uploadContext('/settings'), {})
  assert.deepEqual(uploadContext('/w/not-a-uuid/doc/x'), {})
})

test('only the generic upload route takes the context', () => {
  assert.equal(uploadUrl('/api/upload', `/w/${WS}/doc/${PAGE}`), `/api/upload?page=${PAGE}`)
  assert.equal(uploadUrl('/api/upload', '/'), '/api/upload')
  assert.equal(uploadUrl('/api/boards/tasks/x/attachments', `/w/${WS}/doc/${PAGE}`), '/api/boards/tasks/x/attachments')
})

test('a personal upload (notepad) is never bound to the open page', () => {
  assert.equal(uploadUrl('/api/upload', `/w/${WS}/doc/${PAGE}`, true), '/api/upload?personal=1')
})

test('images of a public presentation carry the link token', async () => {
  const { withShare } = await import('../app/utils/uploadContext.ts')
  assert.equal(withShare('/uploads/a.png', PAGE), `/uploads/a.png?share=${PAGE}`)
  assert.equal(withShare('/uploads/a.png?w=1', PAGE), `/uploads/a.png?w=1&share=${PAGE}`)
  assert.equal(withShare(`/uploads/a.png?share=${PAGE}`, PAGE), `/uploads/a.png?share=${PAGE}`)
  assert.equal(withShare('https://x.org/a.png', PAGE), 'https://x.org/a.png')
  assert.equal(withShare('/uploads/a.png', null), '/uploads/a.png')
})
