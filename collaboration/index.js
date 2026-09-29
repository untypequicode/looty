const { Server } = require('@hocuspocus/server')
const { Database } = require('@hocuspocus/extension-database')
const { Pool } = require('pg')

const pool = new Pool({
  connectionString: process.env.DATABASE_URL || 'postgresql://untype:untype@localhost:5432/looty',
})

const server = new Server({
  port: 8082,
  extensions: [
    new Database({
      fetch: async ({ documentName }) => {
        try {
          const res = await pool.query('SELECT yjs_state FROM pages WHERE id = $1', [documentName])
          if (res.rows.length > 0 && res.rows[0].yjs_state && res.rows[0].yjs_state.length > 0) {
            return res.rows[0].yjs_state
          }
        } catch (e) {
          console.error('Error fetching document', documentName, e)
        }
        // Return a default empty Yjs document with a paragraph
        return Buffer.from('0101ecc4b5ab070007010764656661756c74030970617261677261706800', 'hex')
      },
      store: async ({ documentName, state }) => {
        try {
          await pool.query('UPDATE pages SET yjs_state = $1, updated_at = NOW() WHERE id = $2', [state, documentName])
        } catch (e) {
          console.error('Error saving document', documentName, e)
        }
      },
    }),
  ],
})

server.listen()
console.log('Hocuspocus is listening on port 8082')
