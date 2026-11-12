// node --test frontend/tests/notebook-deps.test.mjs
import test from 'node:test'
import assert from 'node:assert/strict'
import { packageFor, missingModule, isProjectModule, parsePipArgs, parseRequirements, mergeRequirements, requirementName, normalizeName, pipMagic, isWasmImpossible } from '../public/notebook-deps.js'

test('aliases and normalization', () => {
  assert.equal(packageFor('cv2'), 'opencv-python')
  assert.equal(packageFor('Crypto'), 'pycryptodome')
  assert.equal(packageFor('chess'), 'chess')
  assert.equal(normalizeName('Foo_Bar.baz'), 'foo-bar-baz')
})

test('missing module from an error output', () => {
  const err = (evalue, ename = 'ModuleNotFoundError') => ({ output_type: 'error', ename, evalue })
  assert.equal(missingModule(err("No module named 'chess'")), 'chess')
  assert.equal(missingModule(err("No module named 'sklearn.tree'")), 'sklearn')
  assert.equal(missingModule(err("No module named 'x'", 'ImportError')), null)
  assert.equal(missingModule({ output_type: 'stream' }), null)
})

test('project modules', () => {
  assert.ok(isProjectModule('utils', ['utils.py', 'a.csv']))
  assert.ok(isProjectModule('pkg', ['pkg/mod.py']))
  assert.ok(!isProjectModule('chess', ['utils.py', 'pkg/mod.py']))
})

test('pip args', () => {
  assert.deepEqual(parsePipArgs('a b==1.2 "c>=2"'), { packages: ['a', 'b==1.2', 'c>=2'], requirements: [] })
  assert.deepEqual(parsePipArgs('-r requirements.txt'), { packages: [], requirements: ['requirements.txt'] })
  assert.deepEqual(parsePipArgs('--requirement=r.txt -U x'), { packages: ['x'], requirements: ['r.txt'] })
  assert.deepEqual(parsePipArgs('-q -i https://idx/simple --extra-index-url=u -t /tmp y'), { packages: ['y'], requirements: [] })
  assert.equal(pipMagic('  %pip install -q a'), '-q a')
  assert.equal(pipMagic('!pip install a'), 'a')
  assert.equal(pipMagic('pip install a'), null)
})

test('requirements parsing and merge', () => {
  assert.deepEqual(parseRequirements('# c\n\nnumpy>=1 # note\n-r other.txt\n--index-url x\nchess\r\n'), ['numpy>=1', 'chess'])
  assert.equal(requirementName('Foo_Bar[extra]>=1'), 'Foo_Bar')
  const r = mergeRequirements('# deps\nNumPy>=1', ['numpy', 'chess==1.0', 'Chess', 'python_dateutil'])
  assert.equal(r.text, '# deps\nNumPy>=1\nchess==1.0\npython_dateutil\n')
  assert.deepEqual(r.added, ['chess==1.0', 'python_dateutil'])
  assert.equal(mergeRequirements('chess\n', ['chess']).text, 'chess\n')
  assert.equal(mergeRequirements('', ['a']).text, 'a\n')
  assert.equal(mergeRequirements('python-dateutil\n', ['python_dateutil']).added.length, 0)
})

test('wasm impossible', () => {
  assert.ok(isWasmImpossible("Can't find a pure Python 3 wheel for 'torch'"))
  assert.ok(!isWasmImpossible('Failed to fetch'))
})
