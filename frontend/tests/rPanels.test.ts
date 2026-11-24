// node --test frontend/tests/rPanels.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { parseEnv, outputText, plotsOf, rString, rHelpCode, rPreviewCode, helpTopic, historyStep, lineOrSelection, R_ENV_CODE, plotSrc, base64Bytes } from '../app/editor/rPanels.ts'

const U = '\u001f'
const E = '\u001e'

test('parseEnv reads one record per variable, sorted by name', () => {
  const text = `zz${U}numeric${U}3${U}112 bytes${U} num [1:3] 1 2 3${U}0${E}df${U}data.frame${U}3 x 2${U}1.1 Kb${U}'data.frame'${U}1${E}`
  const vars = parseEnv(text)
  assert.deepEqual(vars.map((v) => v.name), ['df', 'zz'])
  assert.equal(vars[0]!.frame, true)
  assert.equal(vars[0]!.dims, '3 x 2')
  assert.equal(vars[1]!.type, 'numeric')
})

test('parseEnv ignores empty or broken records', () => {
  assert.deepEqual(parseEnv(''), [])
  assert.deepEqual(parseEnv(`broken${U}only${E}\n`), [])
})

test('rString escapes quotes, backslashes and newlines so names are never injected as code', () => {
  assert.equal(rString('a"b'), '"a\\"b"')
  assert.equal(rString('a\\b\nc'), '"a\\\\b\\nc"')
  assert.ok(rPreviewCode('x"); system("id"); ("').includes('"x\\"); system(\\"id\\"); (\\""'))
  assert.ok(rHelpCode('mean').includes('utils::help("mean")'))
  assert.ok(R_ENV_CODE.includes('globalenv()'))
})

test('helpTopic strips the question mark and quotes', () => {
  assert.equal(helpTopic('?mean'), 'mean')
  assert.equal(helpTopic('  ??lm '), 'lm')
  assert.equal(helpTopic('"+"'), '+')
  assert.equal(helpTopic('stats::lm'), 'stats::lm')
})

test('outputText keeps stdout and results, not stderr or errors', () => {
  const out = outputText([
    { output_type: 'stream', name: 'stdout', text: 'a' }, { output_type: 'stream', name: 'stderr', text: 'WARN' },
    { output_type: 'execute_result', data: { 'text/plain': ['b'] } }, { output_type: 'error', ename: 'E', evalue: 'x', traceback: [] },
  ] as any)
  assert.equal(out, 'ab')
})

test('plotsOf lists PNG, JPEG and SVG outputs in order, svg kept for export', () => {
  const list = plotsOf('cell-1', [
    { output_type: 'stream', name: 'stdout', text: 'x' },
    { output_type: 'display_data', data: { 'image/png': 'AA\nBB', 'image/svg+xml': '<svg/>' } },
    { output_type: 'display_data', data: { 'image/svg+xml': ['<svg>', '</svg>'] } },
    { output_type: 'execute_result', data: { 'text/plain': 'no image' } },
  ] as any)
  assert.equal(list.length, 2)
  assert.equal(list[0]!.mime, 'image/png')
  assert.equal(list[0]!.data, 'AABB')
  assert.equal(list[0]!.svg, '<svg/>')
  assert.equal(list[1]!.mime, 'image/svg+xml')
  assert.equal(list[1]!.data, '<svg></svg>')
  assert.equal(list[0]!.key, 'cell-1#1')
  assert.ok(plotSrc(list[0]!).startsWith('data:image/png;base64,'))
  assert.ok(plotSrc(list[1]!).startsWith('data:image/svg+xml;utf8,'))
})

test('base64Bytes decodes PNG bytes', () => {
  assert.deepEqual([...base64Bytes('iVBO')], [0x89, 0x50, 0x4e])
})

test('console history walks back and forth, ending on an empty prompt', () => {
  const h = ['a', 'b', 'c']
  let s = historyStep(h, -1, -1)
  assert.deepEqual(s, { index: 2, text: 'c' })
  s = historyStep(h, s.index, -1)
  assert.deepEqual(s, { index: 1, text: 'b' })
  s = historyStep(h, 0, -1)
  assert.deepEqual(s, { index: 0, text: 'a' })
  s = historyStep(h, 2, 1)
  assert.deepEqual(s, { index: -1, text: '' })
  assert.deepEqual(historyStep([], -1, -1), { index: -1, text: '' })
})

test('lineOrSelection gives the cursor line, or the selection', () => {
  const doc = 'x <- 1\ny <- 2\nz <- 3'
  assert.equal(lineOrSelection(doc, 9, 9), 'y <- 2')
  assert.equal(lineOrSelection(doc, 0, 0), 'x <- 1')
  assert.equal(lineOrSelection(doc, 14, 14), 'z <- 3')
  assert.equal(lineOrSelection(doc, 0, 13), 'x <- 1\ny <- 2')
  assert.equal(lineOrSelection(doc, 13, 0), 'x <- 1\ny <- 2')
})
