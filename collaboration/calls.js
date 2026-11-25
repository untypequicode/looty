/**
 * Salles d'appel (ADR 0019) : signalisation WebRTC seule, en mémoire. Aucun média ne passe ici.
 *
 * Une salle est un document Hocuspocus « call:<page|project|workspace>:<uuid> » : les droits sont
 * ceux du contenu (voir backend/src/calls.rs) et les messages sont des messages « stateless » JSON.
 * Chaque connexion à la salle est un observateur (elle reçoit la liste des participants, pour la
 * pastille « en appel ») ; elle devient participante avec « join ».
 *
 * - Le participant a un identifiant choisi par le client (stable pendant l'appel) : si la connexion
 *   WebSocket tombe (révalidation des droits, coupure réseau), il garde sa place `graceMs` et
 *   la reprend en rejoignant avec le même identifiant, sans couper les flux audio pair-à-pair.
 * - L'expéditeur d'un message est toujours déduit de la connexion, jamais du contenu.
 * - Limites : taille d'un message, débit par connexion, nombre de participants par salle.
 */
const PEER_RE = /^[A-Za-z0-9_-]{8,64}$/
const ROOM_RE = /^call:(page|project|workspace):[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
const SIGNAL_KINDS = new Set(['offer', 'answer', 'ice'])

const DEFAULTS = {
  maxParticipants: 6,
  maxBytes: 32 * 1024,
  graceMs: 15_000,
  // Jeton par connexion : rafale de 120 messages (les candidats ICE arrivent en salves), puis 60/s.
  burst: 120,
  perSecond: 60,
}

const isCallRoom = (name) => typeof name === 'string' && ROOM_RE.test(name)
/** « call:page:<uuid> » -> { kind, id } */
function parseRoom(name) {
  if (!isCallRoom(name)) return null
  const [, kind, id] = name.split(':')
  return { kind, id }
}

/** État média annoncé par un participant : micro coupé, identifiants des flux caméra et partage ('' = éteint). */
const streamId = (v) => (typeof v === 'string' && /^[\w{}-]{1,64}$/.test(v) ? v : '')
const mediaState = (msg) => ({ muted: !!msg.muted, cam: streamId(msg.cam), share: streamId(msg.share) })

class CallRooms {
  constructor(options = {}, hooks = {}) {
    this.opts = { ...DEFAULTS, ...options }
    this.log = hooks.log || (() => {})
    this.setTimer = hooks.setTimer || ((fn, ms) => { const t = setTimeout(fn, ms); t.unref?.(); return t })
    this.clearTimer = hooks.clearTimer || clearTimeout
    this.now = hooks.now || (() => Date.now())
    /** @type {Map<string, {conns: Map<string, {send: Function, user: any, tokens: number, at: number}>, peers: Map<string, any>}>} */
    this.rooms = new Map()
  }

  room(name) {
    let r = this.rooms.get(name)
    if (!r) { r = { conns: new Map(), peers: new Map() }; this.rooms.set(name, r) }
    return r
  }

  /** Une connexion arrive dans la salle : elle reçoit l'état courant. */
  attach(name, connId, send, user) {
    const r = this.room(name)
    if (r.conns.has(connId)) return
    r.conns.set(connId, { send, user, tokens: this.opts.burst, at: this.now() })
    this.emit(send, { t: 'presence', peers: this.snapshot(r) })
  }

  /** La connexion se ferme : son participant garde sa place `graceMs`, puis est retiré. */
  detach(name, connId) {
    const r = this.rooms.get(name)
    if (!r) return
    r.conns.delete(connId)
    for (const [id, p] of r.peers) {
      if (p.connId !== connId) continue
      p.connId = null
      p.timer = this.setTimer(() => this.remove(name, id, 'timeout'), this.opts.graceMs)
    }
    this.gc(name)
  }

  /** Message reçu d'une connexion. Renvoie false s'il a été refusé (le journal dit pourquoi). */
  handle(name, connId, payload) {
    const r = this.rooms.get(name)
    const conn = r?.conns.get(connId)
    if (!r || !conn) return false
    if (typeof payload !== 'string' || payload.length > this.opts.maxBytes) {
      this.log(`call: oversized or invalid message room=${name} conn=${connId}`)
      return false
    }
    if (!this.take(conn)) {
      this.log(`call: rate limit room=${name} conn=${connId}`)
      return false
    }
    let msg
    try { msg = JSON.parse(payload) } catch { return false }
    if (!msg || typeof msg !== 'object') return false
    const mine = this.peerOf(r, connId)
    switch (msg.t) {
      case 'join': return this.join(name, r, connId, conn, msg)
      case 'leave':
        if (mine) this.remove(name, mine, 'leave')
        return true
      case 'state':
        if (!mine) return false
        Object.assign(r.peers.get(mine), mediaState(msg))
        this.broadcast(r)
        return true
      case 'signal': {
        if (!mine || !SIGNAL_KINDS.has(msg.kind) || typeof msg.to !== 'string') return false
        const target = r.peers.get(msg.to)
        const out = target?.connId && r.conns.get(target.connId)
        if (!out || msg.to === mine) return false
        this.emit(out.send, { t: 'signal', from: mine, kind: msg.kind, data: msg.data })
        return true
      }
      default: return false
    }
  }

  join(name, r, connId, conn, msg) {
    const id = msg.peerId
    if (typeof id !== 'string' || !PEER_RE.test(id)) return false
    const known = r.peers.get(id)
    if (known && known.user.id !== conn.user.id) {
      this.log(`call: peer id collision room=${name}`)
      return false
    }
    // Une connexion ne tient qu'une place (un onglet = un participant).
    const already = this.peerOf(r, connId)
    if (already && already !== id) this.remove(name, already, 'switch')
    if (!known && r.peers.size >= this.opts.maxParticipants) {
      this.emit(conn.send, { t: 'error', code: 'room-full', max: this.opts.maxParticipants })
      this.log(`call: room full room=${name}`)
      return true
    }
    if (known?.timer) this.clearTimer(known.timer)
    r.peers.set(id, { connId, user: conn.user, ...mediaState(msg), timer: null, joinedAt: known?.joinedAt ?? this.now() })
    this.emit(conn.send, { t: 'joined', you: id, peers: this.snapshot(r), max: this.opts.maxParticipants })
    this.broadcast(r)
    return true
  }

  remove(name, id, reason) {
    const r = this.rooms.get(name)
    const p = r?.peers.get(id)
    if (!p) return
    if (p.timer) this.clearTimer(p.timer)
    r.peers.delete(id)
    this.broadcast(r)
    this.gc(name)
    this.log(`call: peer left room=${name} reason=${reason}`)
  }

  peerOf(r, connId) {
    for (const [id, p] of r.peers) if (p.connId === connId) return id
    return null
  }

  snapshot(r) {
    return [...r.peers].map(([id, p]) => ({ id, user: p.user.id, name: p.user.name, muted: p.muted, cam: p.cam, share: p.share }))
  }

  broadcast(r) {
    const msg = { t: 'presence', peers: this.snapshot(r) }
    for (const c of r.conns.values()) this.emit(c.send, msg)
  }

  emit(send, msg) {
    try { send(JSON.stringify(msg)) } catch { /* connexion fermée : le nettoyage suit */ }
  }

  take(conn) {
    const t = this.now()
    conn.tokens = Math.min(this.opts.burst, conn.tokens + ((t - conn.at) / 1000) * this.opts.perSecond)
    conn.at = t
    if (conn.tokens < 1) return false
    conn.tokens -= 1
    return true
  }

  gc(name) {
    const r = this.rooms.get(name)
    if (r && !r.conns.size && !r.peers.size) this.rooms.delete(name)
  }

  /** Pour la mesure et les tests : nombre de salles et de participants en mémoire. */
  stats() {
    let peers = 0
    for (const r of this.rooms.values()) peers += r.peers.size
    return { rooms: this.rooms.size, peers }
  }
}

module.exports = { CallRooms, isCallRoom, parseRoom, PEER_RE, DEFAULTS }
