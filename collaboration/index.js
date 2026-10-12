const { Server } = require('@hocuspocus/server')
const { Database } = require('@hocuspocus/extension-database')
const { Pool } = require('pg')
const Y = require('yjs')

const pool = new Pool({
  connectionString: process.env.DATABASE_URL || 'postgresql://postgres:postgres@localhost:5432/looty_db',
})

// URL interne du backend Rust, utilisée pour vérifier les droits d'accès aux documents.
const BACKEND_URL = (process.env.BACKEND_URL || 'http://backend:8080').replace(/\/$/, '')

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
// Fichier texte d'un projet (mini-site, LaTeX) : document « file:<id> », texte dans Y.Text('content').
const FILE_PREFIX = 'file:'
const fileId = (name) => (name.startsWith(FILE_PREFIX) && UUID_RE.test(name.slice(FILE_PREFIX.length)) ? name.slice(FILE_PREFIX.length) : null)

/** État Yjs initial d'un fichier jamais ouvert en direct : son texte. */
function textState(text) {
  const doc = new Y.Doc()
  doc.getText('content').insert(0, text || '')
  const state = Buffer.from(Y.encodeStateAsUpdate(doc))
  doc.destroy()
  return state
}

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

function stateText(state) {
  const doc = new Y.Doc()
  Y.applyUpdate(doc, state)
  const text = doc.getText('content').toString()
  doc.destroy()
  return text
}

/**
 * Formules LaTeX et diagrammes Mermaid : leur source était un attribut (la dernière version
 * enregistrée écrasait les autres), c'est désormais le texte du nœud, fusionné caractère par
 * caractère. Conversion unique des anciens documents ; renvoie null s'il n'y a rien à faire.
 */
const SOURCE_NODES = { mathBlock: 'latex', mathInline: 'latex', mermaid: 'code' }
/** Version du format des documents : les interfaces plus anciennes sont refusées. */
const DOC_SCHEMA = 2
function convertSources(state) {
  const doc = new Y.Doc()
  try {
    Y.applyUpdate(doc, state)
    const legacy = []
    const walk = (node) => {
      if (node instanceof Y.XmlElement) {
        const key = SOURCE_NODES[node.nodeName]
        if (key && node.getAttribute(key) && node.length === 0) legacy.push([node, key])
      }
      if (node instanceof Y.XmlElement || node instanceof Y.XmlFragment) node.toArray().forEach(walk)
    }
    walk(doc.getXmlFragment('default'))
    if (!legacy.length) return null
    doc.transact(() => {
      for (const [node, key] of legacy) {
        node.insert(0, [new Y.XmlText(String(node.getAttribute(key)))])
        node.removeAttribute(key)
      }
    })
    return Buffer.from(Y.encodeStateAsUpdate(doc))
  } finally {
    doc.destroy()
  }
}

// Document Yjs vide contenant un paragraphe (évite un crash TipTap sur document vide)
const EMPTY_DOC = Buffer.from('0101ecc4b5ab070007010764656661756c74030970617261677261706800', 'hex')

/**
 * Historique des versions : une version par session d'édition (même auteur, dix minutes
 * au plus). La dernière version automatique est mise à jour tant que la session dure,
 * sinon une nouvelle est créée. L'élagage est fait par le backend (revisions.rs).
 */
async function recordRevision(pageId, state, userId) {
  try {
    const updated = await pool.query(
      `UPDATE page_revisions r SET yjs_state = $2, title = p.title, updated_at = NOW()
       FROM pages_all p
       WHERE p.id = $1 AND r.id = (
         SELECT id FROM page_revisions WHERE page_id = $1 ORDER BY updated_at DESC LIMIT 1
       ) AND r.kind = 'auto' AND r.created_by IS NOT DISTINCT FROM $3::uuid AND r.created_at > NOW() - interval '10 minutes'`,
      [pageId, state, userId],
    )
    if (updated.rowCount === 0) {
      await pool.query(
        `INSERT INTO page_revisions (page_id, yjs_state, title, kind, created_by)
         SELECT id, $2, title, 'auto', $3::uuid FROM pages_all WHERE id = $1`,
        [pageId, state, userId],
      )
    }
  } catch (e) {
    console.error('Error recording revision', pageId, e.message)
  }
}

/**
 * Restauration d'une sauvegarde (backend : NOTIFY looty_restore) : les documents en mémoire
 * sont périmés. On ferme les connexions, on oublie les documents sans les enregistrer, et on
 * refuse brièvement les connexions pour laisser les pages ouvertes se recharger.
 */
let restoredAt = 0
const loadedAt = new Map()
/** Fichiers de projet dont le contenu a été remplacé par le backend (NOTIFY looty_file_reset). */
const resetAt = new Map()

const server = new Server({
  port: Number(process.env.PORT || 8082),
  // Ferme les connexions non authentifiées rapidement
  timeout: 30000,

  /**
   * Chaque connexion à un document est autorisée par le backend, qui lit le
   * cookie de session transmis par le navigateur. Sans droit de lecture la
   * connexion est refusée ; sans droit d'écriture elle passe en lecture seule.
   */
  async onAuthenticate({ documentName, requestHeaders, requestParameters, connectionConfig, token }) {
    if (Date.now() - restoredAt < 8000) {
      throw new Error('restoring')
    }
    const fid = fileId(documentName)
    if (!fid && !UUID_RE.test(documentName)) {
      throw new Error('invalid-document')
    }
    // Interface d'avant la version du format (onglet ouvert avant une mise à jour…) : elle ne
    // sait pas lire les formules et diagrammes convertis et les supprimerait du document.
    if (!fid && Number(requestParameters?.get?.('schema') || 0) < DOC_SCHEMA) {
      throw new Error('outdated-client')
    }
    const cookie = requestHeaders.get ? requestHeaders.get('cookie') : requestHeaders.cookie
    // Jeton API optionnel (clients non navigateur), sinon cookie de session.
    const headers = {}
    if (token && token.startsWith('looty_')) headers.authorization = `Bearer ${token}`
    else if (cookie) headers.cookie = cookie
    else throw new Error('unauthenticated')
    let res
    try {
      const url = fid ? `${BACKEND_URL}/api/projects/files/${fid}/access` : `${BACKEND_URL}/api/pages/${documentName}/access`
      res = await fetch(url, {
        headers,
      })
    } catch (e) {
      console.error('Backend unreachable for auth', e.message)
      throw new Error('auth-unavailable')
    }
    if (!res.ok) {
      throw new Error('forbidden')
    }
    const access = await res.json()
    connectionConfig.readOnly = !access.can_write
    return { user: { id: access.user_id, name: access.name } }
  },

  extensions: [
    new Database({
      fetch: async ({ documentName }) => {
        loadedAt.set(documentName, Date.now())
        const fid = fileId(documentName)
        if (fid) {
          try {
            const res = await pool.query('SELECT yjs_state, content, path FROM project_files WHERE id = $1', [fid])
            const row = res.rows[0]
            if (isNotebookPath(row?.path)) {
              const nb = notebookFromRow(row)
              if (nb.built && row) {
                // Enregistré tout de suite (si personne n'a écrit entre-temps) : la conversion n'a lieu qu'une fois.
                const saved = await pool.query('UPDATE project_files SET yjs_state = $1 WHERE id = $2 AND yjs_state IS NOT DISTINCT FROM $3', [nb.state, fid, row.yjs_state])
                if (saved.rowCount === 0) {
                  const again = await pool.query('SELECT yjs_state FROM project_files WHERE id = $1', [fid])
                  if (again.rows[0]?.yjs_state?.length > 0) return again.rows[0].yjs_state
                }
              }
              return nb.state
            }
            if (row?.yjs_state && row.yjs_state.length > 0) return row.yjs_state
            return textState(row?.content ?? '')
          } catch (e) {
            console.error('Error fetching file', fid, e.message)
            return textState('')
          }
        }
        try {
          const res = await pool.query('SELECT yjs_state FROM pages WHERE id = $1', [documentName])
          const state = res.rows[0]?.yjs_state
          if (state && state.length > 0) {
            const converted = convertSources(state)
            if (!converted) return state
            // Enregistrée tout de suite (et seulement si personne n'a écrit entre-temps) : la
            // conversion n'a lieu qu'une fois, sinon deux conversions se fusionneraient en double.
            const saved = await pool.query('UPDATE pages SET yjs_state = $1 WHERE id = $2 AND yjs_state = $3', [converted, documentName, state])
            if (saved.rowCount === 1) {
              console.log('Converted LaTeX/Mermaid sources of', documentName)
              return converted
            }
            const again = await pool.query('SELECT yjs_state FROM pages WHERE id = $1', [documentName])
            return again.rows[0]?.yjs_state ?? state
          }
        } catch (e) {
          console.error('Error fetching document', documentName, e.message)
        }
        return EMPTY_DOC
      },
      store: async ({ documentName, state, context, lastContext }) => {
        // Hocuspocus 4 : contexte de la dernière connexion ayant modifié le document.
        const userId = lastContext?.user?.id || context?.user?.id || null
        // Document chargé avant une restauration : ne jamais écraser les données restaurées.
        if ((loadedAt.get(documentName) ?? 0) < restoredAt) return
        const fid = fileId(documentName)
        if (fid) {
          // Contenu remplacé ailleurs (envoi, autre client) depuis le chargement : on n'écrase pas.
          if ((loadedAt.get(documentName) ?? 0) < (resetAt.get(documentName) ?? 0)) return
          try {
            const isNb = isNotebookPath((await pool.query('SELECT path FROM project_files WHERE id = $1', [fid])).rows[0]?.path)
            const text = isNb ? notebookText(state) : stateText(state)
            const r = await pool.query(
              'UPDATE project_files SET yjs_state = $1, content = $2, size = octet_length($2), updated_at = NOW(), updated_by = COALESCE($4::uuid, updated_by) WHERE id = $3 RETURNING project_id',
              [state, text, fid, userId],
            )
            if (r.rows[0]) await pool.query('UPDATE projects SET updated_at = NOW() WHERE id = $1', [r.rows[0].project_id])
          } catch (e) {
            console.error('Error saving file', fid, e.message)
          }
          return
        }
        try {
          await pool.query(
            'UPDATE pages SET yjs_state = $1, updated_at = NOW(), updated_by = COALESCE($3::uuid, updated_by) WHERE id = $2',
            [state, documentName, userId],
          )
        } catch (e) {
          console.error('Error saving document', documentName, e.message)
          return
        }
        await recordRevision(documentName, state, userId)
      },
    }),
  ],
})

server.listen()

;(async () => {
  try {
    const client = await pool.connect()
    await client.query('LISTEN looty_restore')
    await client.query('LISTEN looty_file_reset')
    client.on('notification', (msg) => {
      if (msg.channel === 'looty_file_reset') {
        const name = FILE_PREFIX + msg.payload
        resetAt.set(name, Date.now())
        const hp = server.hocuspocus
        hp.closeConnections(name)
        const doc = hp.documents.get(name)
        if (doc) {
          hp.documents.delete(name)
          try { doc.destroy() } catch {}
        }
        return
      }
      restoredAt = Date.now()
      const hp = server.hocuspocus
      hp.closeConnections()
      for (const [name, doc] of hp.documents) {
        hp.documents.delete(name)
        try { doc.destroy() } catch {}
      }
      loadedAt.clear()
      console.log('Backup restored: documents unloaded')
    })
  } catch (e) {
    console.error('LISTEN looty_restore failed', e.message)
  }
})()
console.log('Hocuspocus is listening on port', server.configuration?.port || 8082)
