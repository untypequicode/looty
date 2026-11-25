// node --test frontend/tests/webrEngine.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { parseCapture, runR, queryR, rCall, rQuote, parseCompletion, parsePackages, rCompleteCode, R_HARNESS } from '../app/editor/webrEngine.ts'

const REC = (kind: string, ...f: string[]) => `\x1e${kind}\x1f${f.join('\x1f')}\x1d`

/** Fausse instance WebR : un abri dont captureR renvoie une capture donnée. */
function fakeWebr(cap: any | Error, seen: { code?: string; opts?: any; purged?: number } = {}) {
  return {
    Shelter: class {
      constructor() { return Promise.resolve(this) }
      async captureR(code: string, opts: any) { seen.code = code; seen.opts = opts; if (cap instanceof Error) throw cap; return cap }
      async purge() { seen.purged = (seen.purged ?? 0) + 1 }
    },
  } as any
}

test('rQuote escapes quotes, backslashes and newlines; rCall never interpolates raw code', () => {
  assert.equal(rQuote('a"b\\c\nd'), '"a\\"b\\\\c\\nd"')
  assert.equal(rCall('x <- "1"'), '.looty$run("x <- \\"1\\"")')
})

test('plain stdout lines are joined, stderr stays separate and ordered', () => {
  const out = parseCapture([{ type: 'stdout', data: 'a' }, { type: 'stdout', data: '[1] 2' }, { type: 'stderr', data: 'warn' }, { type: 'stdout', data: 'b' }])
  assert.deepEqual(out.map((o) => [o.name, o.text]), [['stdout', 'a\n[1] 2\n'], ['stderr', 'warn\n'], ['stdout', 'b\n']])
})

test('data frame record becomes an HTML + text result, text around it is kept', () => {
  const out = parseCapture([
    { type: 'stdout', data: 'before' },
    { type: 'stdout', data: REC('H', '<table><tr><td>1</td></tr></table>', '  a\n1 1') },
    { type: 'stdout', data: 'after' },
  ])
  assert.equal(out.length, 3)
  assert.equal(out[1]!.output_type, 'execute_result')
  assert.equal(out[1]!.data!['text/html'], '<table><tr><td>1</td></tr></table>')
  assert.equal(out[1]!.data!['text/plain'], '  a\n1 1')
  assert.equal(out[2]!.text, 'after\n')
})

test('error record becomes an error output with a traceback', () => {
  const out = parseCapture([{ type: 'stdout', data: REC('E', 'Error', 'boom', '1. f()\nError in f() : boom') }])
  assert.equal(out.length, 1)
  assert.equal(out[0]!.output_type, 'error')
  assert.equal(out[0]!.ename, 'Error')
  assert.equal(out[0]!.evalue, 'boom')
  assert.deepEqual(out[0]!.traceback, ['1. f()', 'Error in f() : boom'])
})

test('runR sends the harness call with plot capture, converts images to PNG and flags errors', async () => {
  const seen: any = {}
  const w = fakeWebr({ output: [{ type: 'stdout', data: '[1] 2' }], images: ['img1', 'img2'] }, seen)
  const r = await runR(w, '1+1', async (i) => `png:${i}`)
  assert.equal(r.ok, true)
  assert.match(seen.code, /^\.looty\$run\("1\+1"\)$/)
  assert.equal(seen.opts.captureConditions, false)
  assert.ok(seen.opts.captureGraphics.width > 0)
  assert.equal(seen.purged, 1)
  const imgs = r.outputs.filter((o) => o.output_type === 'display_data')
  assert.deepEqual(imgs.map((o) => o.data!['image/png']), ['png:img1', 'png:img2'])

  const bad = await runR(fakeWebr({ output: [{ type: 'stdout', data: REC('E', 'Error', 'x', 'x') }], images: [] }), 'stop("x")', async () => '')
  assert.equal(bad.ok, false)
})

test('runR turns a WebR failure into an error output instead of throwing', async () => {
  const r = await runR(fakeWebr(new Error('worker died')), '1', async () => '')
  assert.equal(r.ok, false)
  assert.equal(r.outputs[0]!.evalue, 'worker died')
})

test('a broken image is skipped, the rest of the outputs remain', async () => {
  const r = await runR(fakeWebr({ output: [{ type: 'stdout', data: 'ok' }], images: ['bad'] }), 'plot(1)', async () => { throw new Error('no canvas') })
  assert.equal(r.ok, true)
  assert.equal(r.outputs.length, 1)
})

test('queryR returns plain text without graphics or harness', async () => {
  const seen: any = {}
  const r = await queryR(fakeWebr({ output: [{ type: 'stdout', data: 'x' }, { type: 'stdout', data: 'y' }], images: [] }, seen), 'cat("hi")')
  assert.equal(r.ok, true)
  assert.equal(r.outputs[0]!.text, 'x\ny')
  assert.equal(seen.code, 'cat("hi")')
  assert.equal(seen.opts.captureGraphics, false)
})

test('completion code quotes the line; the reply parses to prefix and items', () => {
  assert.match(rCompleteCode('pri"nt'), /"pri\\"nt"/)
  const c = parseCompletion('3\x1fprint\x1eprint.default\x1epaste0(')
  assert.equal(c.prefix, 3)
  assert.deepEqual(c.items.map((i) => i.label), ['print', 'print.default', 'paste0('])
  assert.deepEqual(parseCompletion(''), { prefix: 0, items: [] })
})

test('r-packages.txt: one name per line, comments and junk ignored, no duplicates', () => {
  assert.deepEqual(parsePackages('# libs\ndplyr\r\nggplot2 # plots\n\nbad name\n--x\ndplyr\ndata.table\n'), ['dplyr', 'ggplot2', 'data.table'])
})

test('the harness defines run, table and the install.packages shim', () => {
  for (const s of ['.looty$run <-', '.looty$table <-', 'install.packages = function', 'withVisible']) assert.ok(R_HARNESS.includes(s), s)
})
