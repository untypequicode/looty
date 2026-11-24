// node --test frontend/tests/notebookExport.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { notebookToPython, notebookToHtml, notebookToIpynb, embedImages, type ExportCell } from '../app/editor/notebookExport.ts'

const PNG = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=='
const cells: ExportCell[] = [
  { type: 'markdown', source: '# Titre\nUn texte' },
  { type: 'code', source: '%pip install numpy\nimport numpy as np\nprint(1)\n', count: 2, outputs: [{ output_type: 'stream', name: 'stdout', text: ['1<2\n'] }] },
  { type: 'raw', source: 'brut' },
]

test('python export uses percent cells, comments text and magics', () => {
  assert.equal(notebookToPython(cells), '# %% [markdown]\n# # Titre\n# Un texte\n\n# %%\n# %pip install numpy\nimport numpy as np\nprint(1)\n\n# %% [raw]\n# brut\n')
})

test('html export escapes code and output, inlines images and sanitizes HTML outputs', () => {
  const html = notebookToHtml([
    ...cells,
    { type: 'code', source: '<b>', outputs: [
      { output_type: 'display_data', data: { 'image/png': PNG, 'text/plain': 'x' } },
      { output_type: 'display_data', data: { 'text/html': '<script>1</script><i>ok</i>' } },
      { output_type: 'error', ename: 'E', evalue: 'v', traceback: ['\u001b[31mboom\u001b[0m'] },
    ] },
  ], { title: 'A <b>', lang: 'fr', renderMarkdown: (s) => `<p>${s}</p>`, sanitize: (h) => h.replace(/<script>.*?<\/script>/g, '') })
  assert.ok(html.includes('<title>A &lt;b&gt;</title>'))
  assert.ok(html.includes('<pre class="code">&lt;b&gt;</pre>'))
  assert.ok(html.includes('1&lt;2'))
  assert.ok(html.includes(`src="data:image/png;base64,${PNG}"`))
  assert.ok(html.includes('<i>ok</i>') && !html.includes('<script>'))
  assert.ok(html.includes('boom') && !html.includes('[31m'))
})

test('project images become attachments, others are left alone', async () => {
  const asked: string[] = []
  const r = await embedImages(
    'a ![un](images/a.png) b ![deux](images/sub/a.png "t") ![ext](https://x.org/y.png) ![manque](images/zz.png) ![un](images/a.png)',
    null,
    async (p) => { asked.push(p); return p.endsWith('zz.png') ? null : { mime: 'image/png', base64: PNG } },
  )
  assert.deepEqual(asked, ['images/a.png', 'images/sub/a.png', 'images/zz.png'])
  assert.equal(r.source, 'a ![un](attachment:a.png) b ![deux](attachment:a-2.png "t") ![ext](https://x.org/y.png) ![manque](images/zz.png) ![un](attachment:a.png)')
  assert.deepEqual(Object.keys(r.attachments!), ['a.png', 'a-2.png'])
  assert.deepEqual(await embedImages('rien', null, async () => null), { source: 'rien', attachments: null })
})

test('ipynb export keeps metadata and attaches images to text cells only', () => {
  const out = JSON.parse(notebookToIpynb({ metadata: { kernelspec: { name: 'python3' } } }, [
    { type: 'markdown', source: 'a\nb', attachments: { 'a.png': { 'image/png': PNG } } },
    { type: 'code', source: 'x', count: 1, outputs: [], attachments: { 'a.png': { 'image/png': PNG } } },
  ], ['m1', 'c1']))
  assert.deepEqual(out.metadata, { kernelspec: { name: 'python3' } })
  assert.deepEqual(out.cells[0].source, ['a\n', 'b'])
  assert.ok(out.cells[0].attachments['a.png'])
  assert.ok(!('attachments' in out.cells[1]))
  assert.equal(out.cells[1].execution_count, 1)
  assert.equal(out.nbformat, 4)
})

test('html export honours R Markdown chunk options and hides the YAML header', () => {
  const out = [{ output_type: 'stream', name: 'stdout', text: ['RES\n'] }, { output_type: 'error', ename: 'E', evalue: 'ERR', traceback: [] }]
  const html = notebookToHtml([
    { type: 'raw', source: 'title: Secret', metadata: { looty: { rmd: { kind: 'yaml' } } } },
    { type: 'code', source: 'SHOWN <- 1', outputs: [], metadata: { looty: { rmd: { header: 'r, echo=FALSE' } } } },
    { type: 'code', source: 'HIDDEN <- 1', outputs: out, metadata: { looty: { rmd: { header: 'r, include=FALSE' } } } },
    { type: 'code', source: 'quiet()', outputs: out, metadata: { looty: { rmd: { header: 'r, results="hide"' } } } },
  ], { title: 'T', lang: 'en', renderMarkdown: (s) => s, sanitize: (h) => h })
  assert.ok(!html.includes('Secret') && !html.includes('SHOWN') && !html.includes('HIDDEN'))
  assert.ok(html.includes('quiet()') && html.includes('ERR') && !html.includes('RES'))
})

test('R export keeps lines starting with ! or %', () => {
  assert.equal(notebookToPython([{ type: 'code', source: '!x\n' }], 'r'), '# %%\n!x\n')
})
