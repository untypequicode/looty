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
// Canevas d'une page (tableau blanc) : document « canvas:<page> », éléments dans Y.Map('elements').
const CANVAS_PREFIX = 'canvas:'
const canvasId = (name) => (name.startsWith(CANVAS_PREFIX) && UUID_RE.test(name.slice(CANVAS_PREFIX.length)) ? name.slice(CANVAS_PREFIX.length) : null)

/** Nombre d'éléments présents et textes / liens (recherche, graphe) d'un canevas. */
function canvasSummary(state) {
  const doc = new Y.Doc()
  Y.applyUpdate(doc, state)
  let count = 0
  const parts = []
  for (const el of doc.getMap('elements').values()) {
    if (!el || el.isDeleted) continue
    count++
    if (typeof el.text === 'string' && el.text) parts.push(el.text)
    if (typeof el.link === 'string' && el.link) parts.push(el.link)
  }
  doc.destroy()
  return { count, text: parts.join('\n').slice(0, 200000) }
}

/** État Yjs initial d'un fichier jamais ouvert en direct : son texte. */
function textState(text) {
  const doc = new Y.Doc()
  doc.getText('content').insert(0, text || '')
  const state = Buffer.from(Y.encodeStateAsUpdate(doc))
  doc.destroy()
  return state
}

const { isNotebookPath, notebookText, notebookFromRow } = require('./notebook')

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
const DOC_SCHEMA = 3
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
const accessUrl = (documentName) => {
  const fid = fileId(documentName)
  // Canevas : les droits de sa page.
  const page = canvasId(documentName) ?? documentName
  return fid ? `${BACKEND_URL}/api/projects/files/${fid}/access` : `${BACKEND_URL}/api/pages/${page}/access`
}

/**
 * Droits revérifiés pendant les connexions : une personne retirée de l'espace ou bloquée
 * sur la page est déconnectée, une autre passée en lecture seule ne peut plus écrire.
 * Appelé quand le backend signale un changement de droits (NOTIFY looty_access), et
 * régulièrement en filet de sécurité.
 */
let revalidating = null
let revalidateAgain = false
async function revalidate() {
  if (revalidating) { revalidateAgain = true; return revalidating }
  revalidating = (async () => {
    do {
      revalidateAgain = false
      for (const [name, doc] of server.hocuspocus.documents) {
        for (const conn of doc.getConnections()) {
          const auth = conn.context?.auth
          if (!auth) continue
          let res
          try {
            res = await fetch(accessUrl(name), { headers: auth })
          } catch {
            continue // Backend injoignable : on garde la connexion, on réessaiera.
          }
          if (res.status === 401 || res.status === 403 || res.status === 404) {
            conn.close({ code: 4403, reason: 'forbidden' })
          } else if (res.ok) {
            const access = await res.json().catch(() => null)
            if (access) conn.readOnly = !access.can_write
          }
        }
      }
    } while (revalidateAgain)
    revalidating = null
  })()
  return revalidating
}
let revalidateTimer = null
function scheduleRevalidate() {
  clearTimeout(revalidateTimer)
  revalidateTimer = setTimeout(() => revalidate().catch((e) => console.error('Access revalidation failed', e.message)), 500)
}
setInterval(scheduleRevalidate, 120_000).unref()

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
    const cid = canvasId(documentName)
    if (!fid && !cid && !UUID_RE.test(documentName)) {
      throw new Error('invalid-document')
    }
    // Interface d'avant la version du format (onglet ouvert avant une mise à jour…) : elle ne
    // sait pas lire les formules et diagrammes convertis et les supprimerait du document.
    if (!fid && !cid && Number(requestParameters?.get?.('schema') || 0) < DOC_SCHEMA) {
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
      res = await fetch(accessUrl(documentName), { headers })
    } catch (e) {
      console.error('Backend unreachable for auth', e.message)
      throw new Error('auth-unavailable')
    }
    if (!res.ok) {
      throw new Error('forbidden')
    }
    const access = await res.json()
    connectionConfig.readOnly = !access.can_write
    // Les en-têtes d'authentification restent dans le contexte pour revérifier les droits
    // pendant la connexion (voir revalidate).
    return { user: { id: access.user_id, name: access.name }, auth: headers }
  },

  extensions: [
    new Database({
      fetch: async ({ documentName }) => {
        loadedAt.set(documentName, Date.now())
        const cid = canvasId(documentName)
        if (cid) {
          // Erreur de lecture : la connexion échoue (jamais un canevas vide enregistré à la place).
          const res = await pool.query('SELECT canvas_state FROM pages WHERE id = $1', [cid])
          const state = res.rows[0]?.canvas_state
          return state && state.length > 0 ? state : null
        }
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
            // Jamais un document vide à la place : son enregistrement effacerait le fichier.
            console.error('Error fetching file', fid, e.message)
            throw e
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
          // Base injoignable : la connexion échoue (le client réessaie) plutôt que d'ouvrir un
          // document vide dont l'enregistrement remplacerait la page.
          console.error('Error fetching document', documentName, e.message)
          throw e
        }
        return EMPTY_DOC
      },
      store: async ({ documentName, state, context, lastContext }) => {
        // Hocuspocus 4 : contexte de la dernière connexion ayant modifié le document.
        const userId = lastContext?.user?.id || context?.user?.id || null
        // Document chargé avant une restauration : ne jamais écraser les données restaurées.
        if ((loadedAt.get(documentName) ?? 0) < restoredAt) return
        const cid = canvasId(documentName)
        if (cid) {
          try {
            const { count, text } = canvasSummary(state)
            await pool.query(
              'UPDATE pages SET canvas_state = $1, canvas_count = $2, canvas_text = $3, canvas_updated_at = NOW(), updated_by = COALESCE($5::uuid, updated_by) WHERE id = $4',
              [state, count, text, cid, userId],
            )
          } catch (e) {
            console.error('Error saving canvas', cid, e.message)
          }
          return
        }
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

function onNotification(msg) {
  const hp = server.hocuspocus
  if (msg.channel === 'looty_access') {
    scheduleRevalidate()
    return
  }
  if (msg.channel === 'looty_file_reset') {
    const name = FILE_PREFIX + msg.payload
    resetAt.set(name, Date.now())
    hp.closeConnections(name)
    const doc = hp.documents.get(name)
    if (doc) {
      hp.documents.delete(name)
      try { doc.destroy() } catch {}
    }
    return
  }
  restoredAt = Date.now()
  hp.closeConnections()
  for (const [name, doc] of hp.documents) {
    hp.documents.delete(name)
    try { doc.destroy() } catch {}
  }
  loadedAt.clear()
  console.log('Backup restored: documents unloaded')
}

// Connexion inactive du pool perdue (redémarrage de PostgreSQL) : signalée, sans arrêter le serveur.
pool.on('error', (e) => console.error('PostgreSQL pool error', e.message))

/** Écoute des notifications du backend, reprise automatiquement si la connexion tombe. */
async function listen() {
  let client
  try {
    client = await pool.connect()
    await client.query('LISTEN looty_restore')
    await client.query('LISTEN looty_file_reset')
    await client.query('LISTEN looty_access')
  } catch (e) {
    console.error('LISTEN failed, retrying in 5 s', e.message)
    client?.release(true)
    setTimeout(listen, 5000)
    return
  }
  client.on('notification', onNotification)
  client.once('error', (e) => {
    console.error('LISTEN connection lost, reconnecting', e.message)
    client.release(true)
    setTimeout(listen, 1000)
  })
}
listen()
console.log('Hocuspocus is listening on port', server.configuration?.port || 8082)
