import { reactive, ref, computed, watch, markRaw, onScopeDispose, getCurrentScope, getCurrentInstance, onActivated, onDeactivated, toValue, type MaybeRefOrGetter } from 'vue'
import * as Y from 'yjs'
import { HocuspocusProvider } from '@hocuspocus/provider'
import { api } from './api'
import { socketBase } from './useRealtime'
import {
  type CallKind, type LinkSample, type Quality, type RawCounters,
  roomName, tuneOpusSdp, rmsLevel, speakingNext, micError, audioConstraints, isInitiator, newPeerId, DEFAULT_ICE,
  AUDIO_MAX_BITRATE, cameraProfile, shareProfile, canUseCamera, cameraConstraints, displayConstraints, cameraError, shareCancelled,
  backoffDelay, SIGNALING_GRACE_MS, qualityOf, worstQuality, diffCounters, adaptFactor, streamRole,
} from '~/utils/callRtc'

/**
 * Appels vocaux (ADR 0019) : maillage WebRTC pair-à-pair, le serveur ne relaie que la signalisation
 * (salle « call:<type>:<id> » du serveur de collaboration, voir collaboration/calls.js).
 *
 * L'état est au niveau du module : l'appel survit aux changements de page et la barre d'appel
 * (components/app/CallBar.vue) est montée une fois pour toutes dans app.vue. Une coupure du
 * WebSocket de signalisation (révalidation des droits, réseau) ne coupe pas les flux audio : on
 * rejoint la salle avec le même identifiant dès que la connexion revient.
 */

export interface CallPresence { id: string; user: string; name: string; muted: boolean; cam?: string; share?: string }
export interface CallPeer {
  id: string; name: string; muted: boolean; speaking: boolean; volume: number; state: 'connecting' | 'connected' | 'failed'
  /** Identifiants des flux caméra et partage annoncés par la personne ('' = éteint). */
  cam: string; share: string
  camStream: MediaStream | null; shareStream: MediaStream | null
  /** Liaison coupée, reprise (ICE restart) en cours. */
  reconnecting: boolean
  stats: LinkSample | null; quality: Quality
}
export interface CallDevice { id: string; label: string }

export const callState = reactive({
  status: 'idle' as 'idle' | 'joining' | 'connected',
  room: null as null | { name: string; kind: CallKind; id: string; label: string; to: string },
  peers: {} as Record<string, CallPeer>,
  selfId: '',
  muted: false,
  selfSpeaking: false,
  /** Code d'erreur affiché dans la barre (calls.errors.*), null si tout va bien. */
  error: null as string | null,
  devices: [] as CallDevice[],
  inputId: '',
  max: 6,
  /** Réglages d'instance (admin, #268) lus dans /api/calls/config : appels actifs, caméra et partage d'écran autorisés. */
  enabled: true,
  video: true,
  share: true,
  /** Appuyer pour parler : micro coupé tant que la touche (code KeyboardEvent) n'est pas maintenue. Désactivé par défaut. */
  ptt: false,
  pttKey: 'Backquote',
  pttHeld: false,
  cam: false,
  sharing: false,
  cameras: [] as CallDevice[],
  cameraId: '',
  localCam: null as MediaStream | null,
  localShare: null as MediaStream | null,
  /** Réseau de signalisation coupé (l'audio continue en pair-à-pair). */
  signalingLost: false,
  /** Pire qualité parmi les liaisons : l'indicateur de la barre. */
  quality: 'unknown' as Quality,
})

// --- Salles de signalisation (une connexion par salle, partagée entre l'appel et les pastilles) ---

interface Room {
  name: string
  provider: HocuspocusProvider
  refs: number
  presence: ReturnType<typeof ref<CallPresence[]>>
  denied: ReturnType<typeof ref<boolean>>
  listeners: Set<(m: any) => void>
  retry: ReturnType<typeof setTimeout> | null
}
const rooms = new Map<string, Room>()
let collabUrl = ''
/** Adresse du serveur de collaboration (même règle que les éditeurs) ; à appeler depuis un `setup`. */
function initCollabUrl() {
  if (collabUrl || typeof window === 'undefined') return
  let configured = ''
  try { configured = (useRuntimeConfig().public.collabUrl as string) || '' } catch { /* hors contexte Nuxt : adresse par défaut */ }
  collabUrl = configured || `${socketBase()}/collab`
}

function acquire(name: string): Room {
  const known = rooms.get(name)
  if (known) { known.refs++; return known }
  const provider = new HocuspocusProvider({ url: collabUrl, name, document: new Y.Doc() })
  const room: Room = { name, provider, refs: 1, presence: ref<CallPresence[]>([]), denied: ref(false), listeners: new Set(), retry: null }
  const emit = (m: any) => room.listeners.forEach((l) => l(m))
  provider.on('stateless', ({ payload }: { payload: string }) => {
    let m: any
    try { m = JSON.parse(payload) } catch { return }
    if (m?.t === 'presence' || m?.t === 'joined') room.presence.value = m.peers
    emit(m)
  })
  provider.on('synced', () => emit({ t: '_synced' }))
  provider.on('status', ({ status }: { status: string }) => emit({ t: '_status', status }))
  provider.on('authenticationFailed', ({ reason }: { reason?: string }) => {
    // Panne passagère du backend : on réessaie ; sinon l'accès est refusé (aucune pastille, appel impossible).
    if (reason === 'auth-unavailable' || reason === 'restoring') {
      if (room.retry) clearTimeout(room.retry)
      room.retry = setTimeout(async () => { try { await provider.sendToken(); provider.startSync() } catch { /* nouvelle tentative au prochain événement */ } }, 3000)
      return
    }
    room.denied.value = true
    emit({ t: '_denied' })
  })
  rooms.set(name, room)
  return room
}

function release(room: Room) {
  if (--room.refs > 0) return
  if (room.retry) clearTimeout(room.retry)
  rooms.delete(room.name)
  try { room.provider.destroy() } catch { /* déjà fermé */ }
}

/** Participants d'un appel en cours sur une page, un projet ou un espace (avant de le rejoindre). */
export function useCallPresence(kind: MaybeRefOrGetter<CallKind>, id: MaybeRefOrGetter<string | null | undefined>) {
  initCollabUrl()
  const list = ref<CallPresence[]>([])
  const denied = ref(false)
  // Pages gardées en mémoire (onglets) : la connexion de présence ne vaut que pour la page affichée.
  const active = ref(true)
  if (getCurrentInstance()) { onDeactivated(() => (active.value = false)); onActivated(() => (active.value = true)) }
  let room: Room | null = null
  let sync: ((m: any) => void) | null = null
  const stop = () => {
    if (room && sync) room.listeners.delete(sync)
    if (room) release(room)
    room = null
    list.value = []
  }
  const watcher = watch(
    () => [toValue(kind), active.value ? toValue(id) : null] as const,
    ([k, i]) => {
      stop()
      denied.value = false
      if (!i || typeof window === 'undefined' || !collabUrl) return
      room = acquire(roomName(k, i))
      list.value = room.presence.value ?? []
      denied.value = room.denied.value ?? false
      const r = room
      sync = () => { list.value = r.presence.value ?? []; denied.value = r.denied.value ?? false }
      r.listeners.add(sync)
    },
    { immediate: true },
  )
  if (getCurrentScope()) onScopeDispose(() => { watcher(); stop() })
  return { peers: list, denied }
}

// --- Appel ---

interface Conn {
  id: string
  pc: RTCPeerConnection
  /** Négociation parfaite : la personne au plus grand identifiant cède en cas de collision d'offres. */
  polite: boolean
  makingOffer: boolean
  ignoreOffer: boolean
  forceRestart: boolean
  waited: boolean
  audio: HTMLAudioElement
  voiceId: string
  analyser: AnalyserNode | null
  source: MediaStreamAudioSourceNode | null
  pending: RTCIceCandidateInit[]
  streams: Map<string, MediaStream>
  cam: RTCRtpSender | null
  share: RTCRtpSender | null
  shareAudio: RTCRtpSender | null
  /** Reprise : tentatives d'ICE restart depuis la dernière connexion réussie, et minuterie en attente. */
  attempts: number
  timer: ReturnType<typeof setTimeout> | null
  prev: RawCounters | null
  /** Facteur (0.2..1) appliqué aux plafonds de débit vidéo de cette liaison. */
  factor: number
  applied: WeakMap<RTCRtpSender, string>
}
let local: MediaStream | null = null
let camMedia: MediaStream | null = null
let shareMedia: MediaStream | null = null
/** Flux « ancres » : leur identifiant (stable pendant l'appel) est annoncé aux autres, qui classent ainsi les flux reçus. */
let camAnchor: MediaStream | null = null
let shareAnchor: MediaStream | null = null
let audioCtx: AudioContext | null = null
let localAnalyser: AnalyserNode | null = null
let current: Room | null = null
let listener: ((m: any) => void) | null = null
let iceServers: RTCIceServer[] = DEFAULT_ICE
let meter: ReturnType<typeof setInterval> | null = null
let statsTimer: ReturnType<typeof setInterval> | null = null
let polling = false
let deviceWatcher: (() => void) | null = null
let netWatcher: (() => void) | null = null
let netDebounce: ReturnType<typeof setTimeout> | null = null
let signalingLostAt = 0
let leaving = false
const conns = new Map<string, Conn>()

/** Serveurs ICE et limite fournis par le serveur (crochet TURN : #264). */
async function loadConfig() {
  try {
    const c = await api<{ ice_servers?: RTCIceServer[]; max_participants?: number; enabled?: boolean; video?: boolean; screen_share?: boolean }>('/api/calls/config', { silent: true })
    if (c?.ice_servers?.length) iceServers = c.ice_servers
    if (c?.max_participants) callState.max = c.max_participants
    callState.enabled = c?.enabled !== false
    callState.video = c?.video !== false
    callState.share = c?.screen_share !== false
  } catch { iceServers = DEFAULT_ICE }
}

function send(msg: object) {
  try { current?.provider.sendStateless(JSON.stringify(msg)) } catch { /* connexion coupée : le join est renvoyé à la reprise */ }
}
const signal = (to: string, kind: 'offer' | 'answer' | 'ice', data: unknown) => send({ t: 'signal', to, kind, data })
const mediaState = () => ({ muted: callState.muted, cam: callState.cam && camAnchor ? camAnchor.id : '', share: callState.sharing && shareAnchor ? shareAnchor.id : '' })
const sendJoin = () => send({ t: 'join', peerId: callState.selfId, ...mediaState() })
const sendState = () => send({ t: 'state', ...mediaState() })
const participantCount = () => 1 + Object.keys(callState.peers).length

export async function refreshDevices() {
  try {
    const all = await navigator.mediaDevices.enumerateDevices()
    callState.devices = all.filter((d) => d.kind === 'audioinput' && d.deviceId).map((d, i) => ({ id: d.deviceId, label: d.label || `#${i + 1}` }))
    callState.cameras = all.filter((d) => d.kind === 'videoinput' && d.deviceId).map((d, i) => ({ id: d.deviceId, label: d.label || `#${i + 1}` }))
  } catch { callState.devices = []; callState.cameras = [] }
}

function analyserFor(stream: MediaStream): { analyser: AnalyserNode; source: MediaStreamAudioSourceNode } | null {
  try {
    audioCtx ??= new AudioContext()
    const analyser = audioCtx.createAnalyser()
    analyser.fftSize = 512
    const source = audioCtx.createMediaStreamSource(stream)
    source.connect(analyser)
    return { analyser, source }
  } catch { return null }
}
let localSource: MediaStreamAudioSourceNode | null = null
function setLocalAnalyser(stream: MediaStream) {
  try { localSource?.disconnect() } catch { /* déjà détaché */ }
  const a = analyserFor(stream)
  localAnalyser = a?.analyser ?? null
  localSource = a?.source ?? null
}
const samples = new Uint8Array(512)
function level(a: AnalyserNode | null) {
  if (!a) return 0
  a.getByteTimeDomainData(samples)
  return rmsLevel(samples)
}

function tick() {
  callState.selfSpeaking = !callState.muted && speakingNext(callState.selfSpeaking, level(localAnalyser))
  for (const [id, c] of conns) {
    const p = callState.peers[id]
    if (p) p.speaking = !p.muted && speakingNext(p.speaking, level(c.analyser))
  }
}

function peerEntry(id: string, name = ''): CallPeer {
  return (callState.peers[id] ??= {
    id, name, muted: false, speaking: false, volume: 1, state: 'connecting', cam: '', share: '', camStream: null, shareStream: null,
    reconnecting: false, stats: null, quality: 'unknown',
  })
}

function closeConn(id: string) {
  const c = conns.get(id)
  if (!c) return
  conns.delete(id)
  if (c.timer) clearTimeout(c.timer)
  c.timer = null
  const pc = c.pc
  pc.onicecandidate = pc.ontrack = pc.onconnectionstatechange = pc.onnegotiationneeded = pc.onsignalingstatechange = pc.oniceconnectionstatechange = null
  try { c.source?.disconnect() } catch { /* déjà détaché */ }
  try { pc.close() } catch { /* déjà fermée */ }
  c.audio.srcObject = null
  c.streams.clear()
}

/** Place chaque flux reçu (voix, caméra, partage) à sa place d'après les identifiants annoncés. */
function classify(id: string) {
  const c = conns.get(id)
  const p = callState.peers[id]
  if (!c || !p) return
  let voice: MediaStream | null = null
  let cam: MediaStream | null = null
  let share: MediaStream | null = null
  for (const [sid, s] of c.streams) {
    const role = streamRole(sid, p)
    if (role === 'share' && s.getVideoTracks().length) share = s
    else if (role === 'cam' && s.getVideoTracks().length) cam = s
    else if (role === 'voice' && s.getAudioTracks().length) voice ??= s
  }
  p.camStream = cam && p.cam ? markRaw(cam) : null
  p.shareStream = share && p.share ? markRaw(share) : null
  if (voice && voice.id !== c.voiceId) {
    c.voiceId = voice.id
    c.audio.srcObject = voice
    c.audio.play().catch(() => { /* reprise par resumeAudio() au prochain geste */ })
    try { c.source?.disconnect() } catch { /* déjà détaché */ }
    const a = analyserFor(voice)
    c.analyser = a?.analyser ?? null
    c.source = a?.source ?? null
  }
}

function newConn(id: string): Conn {
  closeConn(id)
  const pc = new RTCPeerConnection({ iceServers })
  const audio = new Audio()
  audio.autoplay = true
  audio.volume = peerEntry(id).volume
  const conn: Conn = {
    id, pc, polite: !isInitiator(callState.selfId, id), makingOffer: false, ignoreOffer: false, forceRestart: false, waited: false,
    audio, voiceId: '', analyser: null, source: null, pending: [], streams: new Map(), cam: null, share: null, shareAudio: null,
    attempts: 0, timer: null, prev: null, factor: 1, applied: new WeakMap(),
  }
  conns.set(id, conn)
  for (const t of local?.getAudioTracks() ?? []) pc.addTrack(t, local!)
  const cam = camMedia?.getVideoTracks()[0]
  if (cam && camAnchor) conn.cam = pc.addTrack(cam, camAnchor)
  const sv = shareMedia?.getVideoTracks()[0]
  if (sv && shareAnchor) conn.share = pc.addTrack(sv, shareAnchor)
  const sa = shareMedia?.getAudioTracks()[0]
  if (sa && shareAnchor) conn.shareAudio = pc.addTrack(sa, shareAnchor)
  pc.onicecandidate = (e) => { if (e.candidate) signal(id, 'ice', e.candidate.toJSON()) }
  pc.ontrack = (e) => {
    const stream = e.streams[0] ?? new MediaStream([e.track])
    conn.streams.set(stream.id, stream)
    classify(id)
  }
  pc.onnegotiationneeded = () => {
    // Au démarrage, la personne « polie » laisse l'autre proposer d'abord : moins de collisions.
    if (conn.polite && !pc.remoteDescription && !conn.waited) {
      conn.waited = true
      conn.timer = setTimeout(() => { conn.timer = null; if (conns.get(id) === conn && !pc.remoteDescription) void negotiate(conn) }, 400)
      return
    }
    void negotiate(conn)
  }
  pc.onsignalingstatechange = () => { if (pc.signalingState === 'stable') void applySenders(conn) }
  pc.onconnectionstatechange = () => {
    const p = callState.peers[id]
    const s = pc.connectionState
    if (s === 'connected') {
      conn.attempts = 0
      if (conn.timer) { clearTimeout(conn.timer); conn.timer = null }
      if (p) { p.state = 'connected'; p.reconnecting = false }
      void applySenders(conn)
    } else if (s === 'disconnected') {
      if (p) p.reconnecting = true
      scheduleRestart(conn, 2500)
    } else if (s === 'failed') {
      if (p) { p.reconnecting = true; if (conn.attempts >= 3) p.state = 'failed' }
      scheduleRestart(conn, conn.attempts === 0 ? 200 : backoffDelay(conn.attempts - 1))
    }
  }
  return conn
}

/** Offre (négociation parfaite) : sert au démarrage, aux changements de pistes et à l'ICE restart. */
async function negotiate(conn: Conn) {
  const { pc, id } = conn
  if (conns.get(id) !== conn || pc.signalingState === 'closed') return
  try {
    conn.makingOffer = true
    const restart = conn.forceRestart
    conn.forceRestart = false
    const o = await pc.createOffer(restart ? { iceRestart: true } : undefined)
    if (pc.signalingState !== 'stable') return
    await pc.setLocalDescription({ type: 'offer', sdp: tuneOpusSdp(o.sdp ?? '') })
    signal(id, 'offer', pc.localDescription)
  } catch (e) { console.warn('call: offer failed', e) } finally { conn.makingOffer = false }
}

function restartIce(conn: Conn) {
  if (conns.get(conn.id) !== conn) return
  if (conn.pc.signalingState === 'stable' && typeof conn.pc.restartIce === 'function') conn.pc.restartIce() // déclenche negotiationneeded
  else { conn.forceRestart = true; void negotiate(conn) }
}

/** ICE restart avec attente croissante ; toutes les 4 tentatives sans succès, la connexion est reconstruite. */
function scheduleRestart(conn: Conn, delay: number) {
  if (conn.timer || !current) return
  conn.timer = setTimeout(() => {
    conn.timer = null
    if (conns.get(conn.id) !== conn || !current || conn.pc.connectionState === 'connected') return
    conn.attempts++
    if (conn.attempts % 4 === 0) { rebuild(conn.id); return }
    restartIce(conn)
    scheduleRestart(conn, backoffDelay(conn.attempts))
  }, delay)
}

/** Connexion neuve vers la même personne (la signalisation resynchronise les deux côtés). */
function rebuild(id: string) {
  const old = conns.get(id)
  if (!old || !callState.peers[id]) return
  const attempts = old.attempts
  closeConn(id)
  const c = newConn(id)
  c.attempts = attempts
  scheduleRestart(c, backoffDelay(attempts))
}

async function onSignal(m: { from: string; kind: string; data: any }, retry = true) {
  const { from } = m
  if (!current) return
  peerEntry(from)
  const conn = conns.get(from) ?? newConn(from)
  const { pc } = conn
  try {
    if (m.kind === 'ice') {
      if (pc.remoteDescription) {
        try { await pc.addIceCandidate(m.data) } catch (e) { if (!conn.ignoreOffer) throw e }
      } else conn.pending.push(m.data)
      return
    }
    const collision = m.kind === 'offer' && (conn.makingOffer || pc.signalingState !== 'stable')
    conn.ignoreOffer = !conn.polite && collision
    if (conn.ignoreOffer) return
    if (m.kind === 'answer' && pc.signalingState !== 'have-local-offer') return
    await pc.setRemoteDescription(m.data) // « polite » : annulation implicite de sa propre offre
    for (const c of conn.pending.splice(0)) await pc.addIceCandidate(c)
    if (m.kind === 'offer') {
      const a = await pc.createAnswer()
      await pc.setLocalDescription({ type: 'answer', sdp: tuneOpusSdp(a.sdp ?? '') })
      signal(from, 'answer', pc.localDescription)
    }
  } catch (e) {
    // Offre incompatible avec l'état de la connexion (l'autre côté a reconstruit la sienne) : on repart de zéro, une fois.
    if (m.kind === 'offer' && retry && conns.get(from) === conn) {
      closeConn(from)
      await onSignal(m, false)
      return
    }
    console.warn('call: signal failed', m.kind, e)
  }
}

/** Débit plafonné et priorités : l'audio d'abord, la vidéo se dégrade avant lui. */
async function applySenders(conn: Conn) {
  const n = participantCount()
  const cam = cameraProfile(n)
  const share = shareProfile(n)
  for (const s of conn.pc.getSenders()) {
    if (!s.track) continue
    const kind = s === conn.cam ? 'cam' : s === conn.share ? 'share' : s === conn.shareAudio ? 'shareAudio' : s.track.kind === 'audio' ? 'voice' : ''
    if (!kind) continue
    const profile = kind === 'cam' ? cam : share
    const key = `${kind}:${n}:${conn.factor.toFixed(2)}`
    if (conn.applied.get(s) === key) continue
    try {
      const p = s.getParameters()
      if (!p.encodings?.length) p.encodings = [{}]
      const e = p.encodings[0] as any
      if (kind === 'voice' || kind === 'shareAudio') {
        e.maxBitrate = kind === 'voice' ? AUDIO_MAX_BITRATE : 128_000
        e.networkPriority = kind === 'voice' ? 'high' : 'medium'
        e.priority = kind === 'voice' ? 'high' : 'medium'
      } else {
        e.maxBitrate = Math.round(profile.maxBitrate * conn.factor)
        e.maxFramerate = profile.fps
        e.networkPriority = 'low'
        e.priority = 'low'
        // Texte : on garde la netteté et on perd des images ; caméra : compromis.
        ;(p as any).degradationPreference = kind === 'share' ? 'maintain-resolution' : 'balanced'
      }
      await s.setParameters(p)
      conn.applied.set(s, key)
    } catch { /* non pris en charge ou connexion en cours de négociation : réessayé au prochain événement */ }
  }
}

function applyAll() {
  for (const c of conns.values()) void applySenders(c)
  // Prise de vue plus légère quand la salle grandit.
  const cam = camMedia?.getVideoTracks()[0]
  if (cam) void cam.applyConstraints(cameraConstraints(cameraProfile(participantCount()))).catch(() => {})
}

// --- Qualité de liaison (getStats) ---

async function readStats(conn: Conn): Promise<LinkSample | null> {
  const report = await conn.pc.getStats()
  let rtt: number | null = null
  let jitter = 0
  let remoteLoss = 0
  const raw: RawCounters = { t: Date.now(), bytesSent: 0, bytesReceived: 0, packetsReceived: 0, packetsLost: 0 }
  let selected: string | undefined
  report.forEach((s: any) => { if (s.type === 'transport' && s.selectedCandidatePairId) selected = s.selectedCandidatePairId })
  report.forEach((s: any) => {
    if (s.type === 'candidate-pair' && (s.id === selected || (!selected && s.nominated && s.state === 'succeeded')) && typeof s.currentRoundTripTime === 'number') rtt = s.currentRoundTripTime * 1000
    else if (s.type === 'inbound-rtp') {
      raw.bytesReceived += s.bytesReceived ?? 0
      raw.packetsReceived += s.packetsReceived ?? 0
      raw.packetsLost += s.packetsLost ?? 0
      if (s.kind === 'audio' && typeof s.jitter === 'number') jitter = Math.max(jitter, s.jitter * 1000)
    } else if (s.type === 'outbound-rtp') raw.bytesSent += s.bytesSent ?? 0
    else if (s.type === 'remote-inbound-rtp') {
      if (rtt == null && typeof s.roundTripTime === 'number') rtt = s.roundTripTime * 1000
      remoteLoss = Math.max(remoteLoss, s.fractionLost ?? 0)
    }
  })
  const d = diffCounters(conn.prev, raw)
  conn.prev = raw
  return { rtt, loss: Math.max(d.loss, remoteLoss), jitter, up: d.up, down: d.down }
}

/** Toutes les 2 s : mesure chaque liaison, ajuste le débit vidéo, met à jour l'indicateur. */
async function pollStats() {
  if (polling || callState.status === 'idle') return
  polling = true
  try {
    for (const [id, c] of [...conns]) {
      const p = callState.peers[id]
      if (!p || c.pc.connectionState === 'closed') continue
      let s: LinkSample | null = null
      try { s = await readStats(c) } catch { continue }
      if (!s || conns.get(id) !== c) continue
      p.stats = s
      p.quality = c.pc.connectionState === 'connected' ? qualityOf(s) : 'unknown'
      if (c.cam?.track || c.share?.track) {
        const f = adaptFactor(c.factor, s)
        if (Math.abs(f - c.factor) > 0.04 || (f === 1 && c.factor !== 1)) { c.factor = f; void applySenders(c) }
      }
    }
    callState.quality = worstQuality(Object.values(callState.peers).map((p) => p.quality))
  } finally { polling = false }
}

/** Changement de réseau (Wi-Fi vers 4G, retour en ligne) : toutes les liaisons repartent en ICE restart. */
function onNetworkChange() {
  if (netDebounce || !current) return
  netDebounce = setTimeout(() => {
    netDebounce = null
    for (const c of conns.values()) { c.attempts = 0; restartIce(c) }
  }, 1000)
}

/** Aligne les connexions sur la liste des participants du serveur. */
function reconcile(list: CallPresence[]) {
  const ids = new Set<string>()
  for (const p of list) {
    if (p.id === callState.selfId) continue
    ids.add(p.id)
    const e = peerEntry(p.id, p.name)
    e.name = p.name
    e.muted = p.muted
    e.cam = p.cam ?? ''
    e.share = p.share ?? ''
    if (!conns.has(p.id)) newConn(p.id)
    classify(p.id)
  }
  for (const id of Object.keys(callState.peers)) {
    if (ids.has(id)) continue
    closeConn(id)
    delete callState.peers[id]
  }
  if (callState.cam && !canUseCamera(participantCount())) { stopCamera(); callState.error = 'video-limit' }
  applyAll()
}

function onMessage(m: any) {
  switch (m?.t) {
    case '_synced':
      if (callState.selfId) sendJoin()
      break
    case '_status':
      if (m.status === 'disconnected') { signalingLostAt ||= Date.now(); callState.signalingLost = true }
      else if (m.status === 'connected') callState.signalingLost = false
      break
    case '_denied':
      void fail('call-denied')
      break
    case 'joined': {
      callState.status = 'connected'
      callState.signalingLost = false
      if (m.max) callState.max = m.max
      // Coupure longue : le serveur nous avait retiré, les autres ont fermé leurs connexions vers nous.
      const longDrop = signalingLostAt > 0 && Date.now() - signalingLostAt > SIGNALING_GRACE_MS
      signalingLostAt = 0
      if (longDrop) for (const id of [...conns.keys()]) closeConn(id)
      reconcile(m.peers)
      // Courte coupure : les messages perdus ont pu bloquer une négociation, on relance les liaisons non établies.
      if (!longDrop) for (const c of conns.values()) if (c.pc.connectionState !== 'connected') restartIce(c)
      break
    }
    case 'presence':
      if (callState.status === 'connected') reconcile(m.peers)
      break
    case 'signal':
      void onSignal(m)
      break
    case 'error':
      void fail(m.code === 'room-full' ? 'room-full' : 'call-unavailable')
      break
  }
}

async function fail(code: string) {
  await leave()
  callState.error = code
}

async function captureMic(deviceId?: string) {
  return navigator.mediaDevices.getUserMedia({ audio: audioConstraints(deviceId) })
}

function watchTrack(track: MediaStreamTrack) {
  // Micro débranché : on reprend le micro par défaut, sinon l'appel continue en écoute seule.
  track.onended = async () => {
    if (!local || leaving) return
    try { await switchInput('') } catch { callState.error = 'mic-lost'; callState.muted = true; sendState() }
  }
}

export async function joinCall(kind: CallKind, id: string, label: string, to: string) {
  if (callState.status !== 'idle') {
    if (callState.room?.id === id && callState.room.kind === kind) return
    await leave()
  }
  callState.error = null
  initCollabUrl()
  if (!navigator.mediaDevices?.getUserMedia || typeof RTCPeerConnection === 'undefined') { callState.error = 'mic-unsupported'; return }
  callState.status = 'joining'
  try {
    const [stream] = await Promise.all([captureMic(callState.inputId || undefined), loadConfig()])
    local = stream
    if (!callState.enabled) { stopStream(local); local = null; callState.status = 'idle'; callState.error = 'calls-disabled'; return }
  } catch (e) {
    callState.status = 'idle'
    callState.error = micError(e)
    return
  }
  leaving = false
  callState.room = { name: roomName(kind, id), kind, id, label, to }
  callState.selfId = newPeerId()
  callState.muted = false
  callState.cam = false
  callState.sharing = false
  callState.peers = {}
  callState.quality = 'unknown'
  signalingLostAt = 0
  camAnchor = new MediaStream()
  shareAnchor = new MediaStream()
  setLocalAnalyser(local)
  const track = local.getAudioTracks()[0]
  if (track) watchTrack(track)
  void refreshDevices()
  const onChange = () => { void refreshDevices() }
  navigator.mediaDevices.addEventListener?.('devicechange', onChange)
  deviceWatcher = () => navigator.mediaDevices.removeEventListener?.('devicechange', onChange)
  const conn = (navigator as any).connection as EventTarget | undefined
  window.addEventListener('online', onNetworkChange)
  conn?.addEventListener?.('change', onNetworkChange)
  netWatcher = () => { window.removeEventListener('online', onNetworkChange); conn?.removeEventListener?.('change', onNetworkChange) }
  current = acquire(callState.room.name)
  listener = onMessage
  current.listeners.add(listener)
  meter = setInterval(tick, 120)
  statsTimer = setInterval(() => { void pollStats() }, 2000)
  // La connexion de signalisation est peut-être déjà synchronisée (pastille ouverte sur la page).
  if (current.provider.synced) sendJoin()
  if (current.denied.value) void fail('call-denied')
  window.addEventListener('pagehide', onPageHide)
  loadPtt()
  window.addEventListener('keydown', onPttDown)
  window.addEventListener('keyup', onPttUp)
  window.addEventListener('blur', onPttBlur)
  if (callState.ptt) applyMute(true)
}

// --- Appuyer pour parler (optionnel, désactivé par défaut) ---

const PTT_STORE = 'looty.call.ptt'
function loadPtt() {
  try {
    const v = JSON.parse(localStorage.getItem(PTT_STORE) || '{}')
    callState.ptt = v.on === true
    if (typeof v.key === 'string' && /^[A-Za-z0-9]{1,24}$/.test(v.key)) callState.pttKey = v.key
  } catch { /* stockage indisponible : réglage par défaut */ }
}
function savePtt() { try { localStorage.setItem(PTT_STORE, JSON.stringify({ on: callState.ptt, key: callState.pttKey })) } catch { /* stockage indisponible */ } }
function applyMute(muted: boolean) {
  if (callState.muted === muted) return
  callState.muted = muted
  local?.getAudioTracks().forEach((t) => { t.enabled = !muted })
  sendState()
}
export function setPtt(on: boolean) {
  callState.ptt = on
  savePtt()
  if (on && callState.status !== 'idle') applyMute(true)
}
export function setPttKey(code: string) {
  if (!/^[A-Za-z0-9]{1,24}$/.test(code)) return
  callState.pttKey = code
  savePtt()
}
const typing = (t: EventTarget | null) => t instanceof HTMLElement && (t.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(t.tagName))
function onPttDown(e: KeyboardEvent) {
  if (!callState.ptt || callState.status === 'idle' || e.code !== callState.pttKey || e.ctrlKey || e.metaKey || e.altKey || typing(e.target)) return
  e.preventDefault()
  if (e.repeat) return
  callState.pttHeld = true
  applyMute(false)
}
function onPttUp(e: KeyboardEvent) {
  if (!callState.ptt || e.code !== callState.pttKey || !callState.pttHeld) return
  callState.pttHeld = false
  applyMute(true)
}
function onPttBlur() {
  if (!callState.ptt || !callState.pttHeld) return
  callState.pttHeld = false
  applyMute(true)
}

function onPageHide() { send({ t: 'leave' }) }

const stopStream = (s: MediaStream | null) => s?.getTracks().forEach((t) => { t.onended = null; t.stop() })

export async function leave() {
  if (leaving) return
  leaving = true
  window.removeEventListener('pagehide', onPageHide)
  window.removeEventListener('keydown', onPttDown)
  window.removeEventListener('keyup', onPttUp)
  window.removeEventListener('blur', onPttBlur)
  callState.pttHeld = false
  send({ t: 'leave' })
  if (meter) clearInterval(meter)
  if (statsTimer) clearInterval(statsTimer)
  if (netDebounce) clearTimeout(netDebounce)
  meter = statsTimer = netDebounce = null
  deviceWatcher?.()
  netWatcher?.()
  deviceWatcher = netWatcher = null
  for (const id of [...conns.keys()]) closeConn(id)
  stopStream(local)
  stopStream(camMedia)
  stopStream(shareMedia)
  local = camMedia = shareMedia = camAnchor = shareAnchor = null
  try { localSource?.disconnect() } catch { /* déjà détaché */ }
  localSource = null
  localAnalyser = null
  try { await audioCtx?.close() } catch { /* déjà fermé */ }
  audioCtx = null
  if (current) {
    if (listener) current.listeners.delete(listener)
    release(current)
  }
  current = null
  listener = null
  callState.status = 'idle'
  callState.room = null
  callState.peers = {}
  callState.selfId = ''
  callState.selfSpeaking = false
  callState.cam = false
  callState.sharing = false
  callState.localCam = null
  callState.localShare = null
  callState.signalingLost = false
  callState.quality = 'unknown'
  signalingLostAt = 0
  leaving = false
}

export function toggleMute() {
  if (callState.status === 'idle') return
  callState.muted = !callState.muted
  local?.getAudioTracks().forEach((t) => { t.enabled = !callState.muted })
  sendState()
}

/** Change de micro sans renégocier : la piste est remplacée sur chaque connexion. */
export async function switchInput(deviceId: string) {
  const stream = await captureMic(deviceId || undefined)
  const track = stream.getAudioTracks()[0]!
  track.enabled = !callState.muted
  watchTrack(track)
  for (const c of conns.values()) {
    const sender = c.pc.getSenders().find((s) => s.track?.kind === 'audio' && s !== c.shareAudio)
    await sender?.replaceTrack(track)
  }
  stopStream(local)
  local = stream
  setLocalAnalyser(stream)
  callState.inputId = deviceId
  callState.error = null
  void refreshDevices()
}

export async function chooseInput(deviceId: string) {
  try { await switchInput(deviceId) } catch (e) { callState.error = micError(e) }
}

// --- Caméra et partage d'écran (#265) ---

/** Pose une piste sur chaque connexion : un émetteur existant est réutilisé (pas de renégociation), sinon il est créé. */
async function setTrack(slot: 'cam' | 'share' | 'shareAudio', track: MediaStreamTrack | null) {
  const anchor = slot === 'cam' ? camAnchor : shareAnchor
  for (const c of conns.values()) {
    const sender = c[slot]
    if (sender) { try { await sender.replaceTrack(track) } catch { /* connexion fermée */ } }
    else if (track && anchor) c[slot] = c.pc.addTrack(track, anchor) // négociation parfaite : negotiationneeded
    c.applied = new WeakMap()
    void applySenders(c)
  }
}

export async function startCamera(deviceId?: string) {
  if (callState.status === 'idle') return
  if (!callState.video) { callState.error = 'video-disabled'; return }
  if (!canUseCamera(participantCount())) { callState.error = 'video-limit'; return }
  callState.error = null
  let stream: MediaStream
  try {
    const id = deviceId ?? (callState.cameraId || undefined)
    stream = await navigator.mediaDevices.getUserMedia({ video: cameraConstraints(cameraProfile(participantCount()), id) })
  } catch (e) { callState.error = cameraError(e); return }
  if (!current) { stopStream(stream); return }
  const track = stream.getVideoTracks()[0]!
  track.contentHint = 'motion'
  track.onended = () => { if (camMedia === stream) void stopCamera() }
  const old = camMedia
  camMedia = stream
  callState.localCam = markRaw(stream)
  callState.cam = true
  if (deviceId !== undefined) callState.cameraId = deviceId
  await setTrack('cam', track)
  stopStream(old)
  sendState()
  void refreshDevices()
}

export async function stopCamera() {
  stopStream(camMedia)
  camMedia = null
  callState.localCam = null
  callState.cam = false
  await setTrack('cam', null)
  sendState()
}

export const toggleCamera = () => (callState.cam ? stopCamera() : startCamera())
export async function chooseCamera(deviceId: string) {
  callState.cameraId = deviceId
  if (callState.cam) await startCamera(deviceId)
}

export const canShareScreen = () => typeof navigator !== 'undefined' && !!navigator.mediaDevices?.getDisplayMedia && (window as unknown as { __lootyCaps?: { screenShare?: boolean } }).__lootyCaps?.screenShare !== false // Android (WebView) : pas de partage d'écran

export async function startShare() {
  if (callState.status === 'idle' || !canShareScreen()) return
  if (!callState.share) { callState.error = 'share-disabled'; return }
  callState.error = null
  let stream: MediaStream
  try { stream = await navigator.mediaDevices.getDisplayMedia(displayConstraints(shareProfile(participantCount())) as DisplayMediaStreamOptions) } catch (e) {
    if (!shareCancelled(e)) callState.error = 'share-failed'
    return
  }
  if (!current) { stopStream(stream); return }
  const video = stream.getVideoTracks()[0]
  if (!video) { stopStream(stream); return }
  video.contentHint = 'detail' // texte : netteté avant fluidité
  video.onended = () => { if (shareMedia === stream) void stopShare() }
  const old = shareMedia
  shareMedia = stream
  callState.localShare = markRaw(stream)
  callState.sharing = true
  await setTrack('share', video)
  await setTrack('shareAudio', stream.getAudioTracks()[0] ?? null)
  stopStream(old)
  sendState()
}

export async function stopShare() {
  stopStream(shareMedia)
  shareMedia = null
  callState.localShare = null
  callState.sharing = false
  await setTrack('share', null)
  await setTrack('shareAudio', null)
  sendState()
}

export function setVolume(id: string, volume: number) {
  const v = Math.min(1, Math.max(0, volume))
  const p = callState.peers[id]
  if (p) p.volume = v
  const c = conns.get(id)
  if (c) c.audio.volume = v
}

/** Lecture bloquée par le navigateur (onglet en arrière-plan, pas de geste) : reprise sur demande. */
export function resumeAudio() {
  void audioCtx?.resume()
  for (const c of conns.values()) void c.audio.play().catch(() => {})
}

export const inCall = computed(() => callState.status !== 'idle')
export const peerList = computed(() => Object.values(callState.peers))
export const isInRoom = (kind: CallKind, id: string) => callState.room?.kind === kind && callState.room.id === id && callState.status !== 'idle'
