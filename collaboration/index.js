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
        try {
          const res = await pool.query('SELECT yjs_state FROM pages WHERE id = $1', [documentName])
          const state = res.rows[0]?.yjs_state
          if (state && state.length > 0) return state
        } catch (e) {
          console.error('Error fetching document', documentName, e.message)
        }
        return EMPTY_DOC
      },
      store: async ({ documentName, state, context }) => {
        try {
          await pool.query(
            'UPDATE pages SET yjs_state = $1, updated_at = NOW(), updated_by = COALESCE($3::uuid, updated_by) WHERE id = $2',
            [state, documentName, context?.user?.id || null],
          )
        } catch (e) {
          console.error('Error saving document', documentName, e.message)
        }
      },
    }),
  ],
})

server.listen()
console.log('Hocuspocus is listening on port', server.configuration?.port || 8082)
