const { Server } = require('@hocuspocus/server')
const { Database } = require('@hocuspocus/extension-database')
const { Pool } = require('pg')

const pool = new Pool({
  connectionString: process.env.DATABASE_URL || 'postgresql://postgres:postgres@localhost:5432/looty_db',
})

// URL interne du backend Rust, utilisée pour vérifier les droits d'accès aux documents.
const BACKEND_URL = (process.env.BACKEND_URL || 'http://backend:8080').replace(/\/$/, '')

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

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

const server = new Server({
  port: Number(process.env.PORT || 8082),
  // Ferme les connexions non authentifiées rapidement
  timeout: 30000,

  /**
   * Chaque connexion à un document est autorisée par le backend, qui lit le
   * cookie de session transmis par le navigateur. Sans droit de lecture la
   * connexion est refusée ; sans droit d'écriture elle passe en lecture seule.
   */
  async onAuthenticate({ documentName, requestHeaders, connectionConfig, token }) {
    if (Date.now() - restoredAt < 8000) {
      throw new Error('restoring')
    }
    if (!UUID_RE.test(documentName)) {
      throw new Error('invalid-document')
    }
    const cookie = requestHeaders.get ? requestHeaders.get('cookie') : requestHeaders.cookie
    // Jeton API optionnel (clients non navigateur), sinon cookie de session.
    const headers = {}
    if (token && token.startsWith('looty_')) headers.authorization = `Bearer ${token}`
    else if (cookie) headers.cookie = cookie
    else throw new Error('unauthenticated')
    let res
    try {
      res = await fetch(`${BACKEND_URL}/api/pages/${documentName}/access`, {
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
        try {
          const res = await pool.query('SELECT yjs_state FROM pages WHERE id = $1', [documentName])
          const state = res.rows[0]?.yjs_state
          if (state && state.length > 0) return state
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
    client.on('notification', () => {
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
