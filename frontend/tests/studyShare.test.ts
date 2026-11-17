import { test } from 'node:test'
import assert from 'node:assert/strict'
import { isStudyShareRedirect } from '../app/utils/studyShare.ts'

test('a study share link is recognised, with or without other parameters', () => {
  assert.equal(isStudyShareRedirect('/study/3f2a?token=abc'), true)
  assert.equal(isStudyShareRedirect('/study/3f2a?mode=edit&token=abc#x'), true)
})

test('other paths are not share links', () => {
  assert.equal(isStudyShareRedirect('/study/3f2a'), false)
  assert.equal(isStudyShareRedirect('/study?token=abc'), false)
  assert.equal(isStudyShareRedirect('/w/1/doc/2?token=abc'), false)
  assert.equal(isStudyShareRedirect('//evil.example/study/1?token=a'), false)
  assert.equal(isStudyShareRedirect(undefined), false)
  assert.equal(isStudyShareRedirect(['/study/1?token=a']), false)
})
