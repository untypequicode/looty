// node --test frontend/tests/chatMath.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { extractMath } from '../app/editor/chatMathParse.ts'

const tex = (s: string) => extractMath(s).parts.map((p) => [p.tex, p.display])

test('inline and display delimiters', () => {
  assert.deepEqual(tex('a $x^2$ b'), [['x^2', false]])
  assert.deepEqual(tex('a $$x^2$$ b'), [['x^2', true]])
  assert.deepEqual(tex('a \\(x\\) b'), [['x', false]])
  assert.deepEqual(tex('a \\[x\\] b'), [['x', true]])
  assert.deepEqual(tex('$$\na\n$$'), [['a', true]])
})

test('prices and lone dollar stay text', () => {
  assert.equal(extractMath('Ça coûte 5 $ ou $5 et $10').parts.length, 0)
  assert.equal(extractMath('un seul $ ici').parts.length, 0)
  assert.equal(extractMath('$5 and $10').text, '$5 and $10')
})

test('escaped dollar', () => {
  const r = extractMath('prix \\$5 et \\$6 puis $x$')
  assert.deepEqual(r.parts.map((p) => p.tex), ['x'])
  assert.ok(r.text.startsWith('prix \\$5 et \\$6'))
})

test('unclosed formula stays raw while streaming', () => {
  for (const s of ['a $x^2', 'a $$x^2', 'a \\(x', 'a \\[x', 'a $$x$']) {
    const r = extractMath(s)
    assert.equal(r.parts.length, 0, s)
    assert.equal(r.text, s)
  }
})

test('code is untouched', () => {
  assert.equal(extractMath('`$a$` et $b$').parts.length, 1)
  assert.equal(extractMath('```\n$a$ $$b$$\n```\n$c$').parts.map((p) => p.tex).join(), 'c')
  assert.equal(extractMath('```js\n$a$ \\(b\\)').parts.length, 0)
  assert.equal(extractMath('~~~\n$a$\n~~~').parts.length, 0)
})

test('placeholders replace formulas', () => {
  const r = extractMath('x $a$ y $$b$$')
  assert.equal(r.text, `x ${r.parts[0]!.placeholder} y ${r.parts[1]!.placeholder}`)
})
