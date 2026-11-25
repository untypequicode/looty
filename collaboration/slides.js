// Présentations (ADR 0023) : document Yjs « slides:<page> » (meta, slides). Ce module garde la
// version du format (SLIDES_SCHEMA, avec frontend/app/slides/model.ts) et calcule le résumé
// enregistré avec l'état : nombre de diapositives et plan textuel (recherche, graphe, export).
const Y = require('yjs')

/** Version du format des présentations : les interfaces plus anciennes sont refusées. */
const SLIDES_SCHEMA = 1
const SLIDES_PREFIX = 'slides:'
const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
const slidesId = (name) => (name.startsWith(SLIDES_PREFIX) && UUID_RE.test(name.slice(SLIDES_PREFIX.length)) ? name.slice(SLIDES_PREFIX.length) : null)

/** Texte d'un nœud XML (paragraphes séparés par des retours à la ligne). */
function nodeText(node) {
  if (node instanceof Y.XmlText) return node.toDelta().map((d) => (typeof d.insert === 'string' ? d.insert : '')).join('')
  if (node instanceof Y.XmlFragment) {
    const inner = node.toArray().map(nodeText).join('')
    return node instanceof Y.XmlElement ? `${inner}\n` : inner
  }
  return ''
}

/** Nombre de diapositives et plan textuel (une rubrique par diapositive), borné pour la base. */
function slidesSummary(state) {
  const doc = new Y.Doc()
  try {
    Y.applyUpdate(doc, state)
    const rows = []
    doc.getMap('slides').forEach((s, id) => {
      if (s instanceof Y.Map) rows.push({ id, pos: Number(s.get('pos')) || 0, s })
    })
    rows.sort((a, b) => a.pos - b.pos || (a.id < b.id ? -1 : 1))
    const out = rows.map(({ s }, i) => {
      const parts = []
      const els = []
      const m = s.get('elements')
      if (m instanceof Y.Map) m.forEach((e, id) => { if (e instanceof Y.Map) els.push({ id, z: Number(e.get('z')) || 0, e }) })
      els.sort((a, b) => a.z - b.z || (a.id < b.id ? -1 : 1))
      for (const { e } of els) {
        const t = e.get('text')
        if (t instanceof Y.XmlFragment) {
          const text = nodeText(t).replace(/\n{2,}/g, '\n').trim()
          if (text) parts.push(text)
        }
        const alt = e.get('alt')
        if (typeof alt === 'string' && alt) parts.push(alt)
      }
      const notes = s.get('notes')
      const n = notes instanceof Y.Text ? notes.toString() : ''
      if (n) parts.push(n)
      return `## ${i + 1}\n${parts.join('\n')}`
    })
    return { count: rows.length, text: out.join('\n\n').slice(0, 200000) }
  } finally {
    doc.destroy()
  }
}

/**
 * Identifiant de la première diapositive d'une présentation neuve : le même que INITIAL_SLIDE_ID de
 * frontend/app/slides/model.ts. Deux personnes qui ouvrent un document neuf en même temps écrivent
 * alors la même clé, et une seule diapositive subsiste.
 */
const INITIAL_SLIDE_ID = '00000000-0000-4000-8000-000000000001'

/**
 * État d'une présentation neuve (méta et une diapositive vide), fabriqué par le serveur : tout le monde
 * reçoit le même document de départ au lieu de l'initialiser chacun de son côté (même contenu que
 * initDoc de frontend/app/slides/model.ts).
 */
function initialSlidesState() {
  const doc = new Y.Doc()
  try {
    doc.transact(() => {
      const meta = doc.getMap('meta')
      meta.set('schema', SLIDES_SCHEMA)
      meta.set('width', 1920)
      meta.set('height', 1080)
      const s = new Y.Map()
      doc.getMap('slides').set(INITIAL_SLIDE_ID, s)
      s.set('id', INITIAL_SLIDE_ID)
      s.set('pos', 1)
      s.set('layout', 'blank')
      s.set('background', 'default')
      s.set('notes', new Y.Text())
      s.set('elements', new Y.Map())
    })
    return Buffer.from(Y.encodeStateAsUpdate(doc))
  } finally {
    doc.destroy()
  }
}

module.exports = { SLIDES_SCHEMA, INITIAL_SLIDE_ID, slidesId, slidesSummary, initialSlidesState }
