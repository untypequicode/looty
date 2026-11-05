const Y = require('yjs')

/**
 * Notebooks Jupyter (.ipynb) : document Yjs structuré, une cellule par Y.Map (type, source
 * en Y.Text, sorties en JSON, numéro d'exécution), pour que deux personnes modifient la
 * même notebook sans s'écraser. Le fichier .ipynb standard est reconstruit à chaque
 * enregistrement (téléchargement, export, aperçu).
 */
const isNotebookPath = (p) => /\.ipynb$/i.test(p || '')
const newCellId = () => Math.random().toString(36).slice(2, 10)

function notebookFromJson(text) {
  let nb = null
  try { nb = JSON.parse(text || '') } catch { /* fichier vide ou invalide : notebook vide */ }
  const cells = Array.isArray(nb?.cells) ? nb.cells : [{ cell_type: 'code', source: '' }]
  const meta = { metadata: nb?.metadata && typeof nb.metadata === 'object' ? nb.metadata : {}, nbformat: 4, nbformat_minor: 5 }
  meta.metadata.kernelspec ||= { name: 'python3', display_name: 'Python 3 (Pyodide)', language: 'python' }
  meta.metadata.language_info ||= { name: 'python' }
  return { cells, meta }
}

function notebookState(text) {
  const { cells, meta } = notebookFromJson(text)
  const doc = new Y.Doc()
  doc.transact(() => {
    const arr = doc.getArray('cells')
    for (const c of cells.slice(0, 2000)) {
      const m = new Y.Map()
      const t = new Y.Text()
      t.insert(0, Array.isArray(c.source) ? c.source.join('') : String(c.source ?? ''))
      m.set('id', String(c.id || newCellId()).slice(0, 64))
      m.set('type', c.cell_type === 'markdown' ? 'markdown' : c.cell_type === 'raw' ? 'raw' : 'code')
      m.set('source', t)
      m.set('outputs', JSON.stringify(Array.isArray(c.outputs) ? c.outputs : []))
      m.set('count', typeof c.execution_count === 'number' ? c.execution_count : null)
      m.set('meta', JSON.stringify(c.metadata && typeof c.metadata === 'object' ? c.metadata : {}))
      // Pièces jointes Jupyter (images collées dans JupyterLab) : gardées telles quelles.
      if (c.attachments && typeof c.attachments === 'object' && Object.keys(c.attachments).length) m.set('attachments', JSON.stringify(c.attachments))
      arr.push([m])
    }
    doc.getMap('meta').set('nb', JSON.stringify(meta))
  })
  const state = Buffer.from(Y.encodeStateAsUpdate(doc))
  doc.destroy()
  return state
}

/** Document Yjs → fichier .ipynb (nbformat 4.5, indentation d'un espace comme Jupyter). */
function notebookText(state) {
  const doc = new Y.Doc()
  Y.applyUpdate(doc, state)
  let meta = { metadata: {}, nbformat: 4, nbformat_minor: 5 }
  try { meta = { ...meta, ...JSON.parse(doc.getMap('meta').get('nb') || '{}') } } catch { /* métadonnées par défaut */ }
  const parse = (v, d) => { try { return JSON.parse(v) } catch { return d } }
  const cells = doc.getArray('cells').toArray().map((m) => {
    const type = m.get('type') || 'code'
    const source = (m.get('source')?.toString() ?? '').split(/(?<=\n)/)
    const base = { cell_type: type, id: m.get('id') || newCellId(), metadata: parse(m.get('meta'), {}), source }
    const attachments = parse(m.get('attachments'), null)
    // nbformat : pièces jointes sur les cellules Markdown et brutes seulement.
    if (type !== 'code' && attachments && typeof attachments === 'object' && Object.keys(attachments).length) base.attachments = attachments
    return type === 'code' ? { ...base, execution_count: m.get('count') ?? null, outputs: parse(m.get('outputs'), []) } : base
  })
  doc.destroy()
  return JSON.stringify({ cells, metadata: meta.metadata, nbformat: 4, nbformat_minor: 5 }, null, 1) + '\n'
}

/** État Yjs d'un notebook : celui enregistré s'il est déjà structuré, sinon construit depuis le fichier. */
function notebookFromRow(row) {
  if (row?.yjs_state && row.yjs_state.length > 0) {
    const doc = new Y.Doc()
    Y.applyUpdate(doc, row.yjs_state)
    const structured = doc.getArray('cells').length > 0 || doc.getMap('meta').has('nb')
    // Ouvert auparavant comme fichier texte : son texte devient le notebook.
    const legacy = doc.getText('content').toString()
    doc.destroy()
    if (structured) return { state: row.yjs_state, built: false }
    return { state: notebookState(legacy || row.content), built: true }
  }
  return { state: notebookState(row?.content), built: true }
}

module.exports = { isNotebookPath, notebookState, notebookText, notebookFromRow }
