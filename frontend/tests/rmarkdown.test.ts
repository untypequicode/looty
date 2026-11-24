// node --test frontend/tests/rmarkdown.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { rmdToNotebook, notebookToRmd, notebookLanguage, parseChunkHeader, chunkRender } from '../app/editor/rmarkdown.ts'

const DOC = `---
title: "Analyse"
output: html_document
---

# Intro

Du texte avec \`code\`.

\`\`\`{r setup, include=FALSE}
library(ggplot2)
\`\`\`

\`\`\`{r plot, echo=FALSE, fig.width=6}
plot(cars)
\`\`\`

\`\`\`{bash}
ls -l
\`\`\`

Fin.
`

const round = (text: string, flavor?: 'rmd' | 'qmd') => {
  const doc = rmdToNotebook(text, { flavor })
  return notebookToRmd({ metadata: doc.metadata }, doc.cells)
}

test('Rmd is split into yaml, markdown, R chunks and foreign chunks', () => {
  const doc = rmdToNotebook(DOC)
  assert.deepEqual(doc.cells.map((c) => c.type), ['raw', 'markdown', 'code', 'code', 'raw', 'markdown'])
  assert.equal(doc.cells[0]!.source, 'title: "Analyse"\noutput: html_document')
  assert.equal(doc.cells[2]!.metadata!.looty.rmd.header, 'r setup, include=FALSE')
  assert.equal(doc.cells[3]!.source, 'plot(cars)')
  assert.equal(doc.metadata.kernelspec.language, 'R')
  assert.equal(notebookLanguage({ metadata: doc.metadata }), 'r')
})

test('round trip is lossless on canonical and odd documents', () => {
  const docs = [
    DOC,
    '',
    '\n',
    'Juste du texte',
    'Juste du texte\n\n\n',
    '\n\n# Titre\n\n\n\n```{r}\n\n```\n\n```{r}\nx <- 1\n\n```\n',
    '---\n---\n\ntexte\n',
    '---\ntitle: x\n...\n\ntexte\n',
    'texte\n```{r}\nx\n```\nsuite\n',
    '````{r}\n```\nx\n```\n````\n',
    '```\n```{r}\nnon exécuté\n```\n```\n\napres\n',
    '```{r}\nunclosed\n',
    '```{r, echo=FALSE}\r\nx\r\n```\r\n\r\ntexte\r\n',
    '# Titre\n\n```{python}\n#| echo: false\nprint(1)\n```\n',
  ]
  for (const d of docs) assert.equal(round(d), d, JSON.stringify(d))
})

test('quarto documents default to Python and keep chunk option lines', () => {
  const doc = rmdToNotebook('# T\n', { flavor: 'qmd' })
  assert.equal(notebookLanguage({ metadata: doc.metadata }), 'python')
  const py = rmdToNotebook('```{python}\n#| echo: false\n1+1\n```\n', { flavor: 'qmd' })
  assert.equal(py.cells[0]!.type, 'code')
  assert.equal(py.cells[0]!.source, '#| echo: false\n1+1')
})

test('notebook cells without chunk metadata are written as chunks of the notebook language', () => {
  const out = notebookToRmd({ metadata: { kernelspec: { language: 'R', name: 'ir' } } }, [
    { type: 'markdown', source: '# Titre\n' },
    { type: 'code', source: 'x <- "```"', outputs: [{ output_type: 'stream', name: 'stdout', text: 'hidden' }] },
  ])
  assert.equal(out, '# Titre\n\n````{r}\nx <- "```"\n````\n')
})

test('chunk headers and render options', () => {
  assert.deepEqual(parseChunkHeader('r label, echo=FALSE, opts=c(1,2)'), { engine: 'r', label: 'label', options: { echo: 'FALSE', opts: 'c(1,2)' } })
  assert.deepEqual(parseChunkHeader('python'), { engine: 'python', label: '', options: {} })
  assert.deepEqual(chunkRender('r, echo=FALSE', 'x'), { echo: false, include: true, hideResults: false })
  assert.deepEqual(chunkRender('r, include=F', 'x').include, false)
  assert.equal(chunkRender('r, results="hide"', 'x').hideResults, true)
  assert.equal(chunkRender(undefined, '#| echo: false\n#| output: false\nx').echo, false)
  assert.equal(chunkRender(undefined, '#| echo: false\n#| output: false\nx').hideResults, true)
  assert.equal(chunkRender(undefined, 'x\n#| echo: false').echo, true)
})
