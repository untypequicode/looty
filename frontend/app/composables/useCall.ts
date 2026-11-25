import { reactive, ref, computed, watch, onScopeDispose, getCurrentScope, getCurrentInstance, onActivated, onDeactivated, toValue, type MaybeRefOrGetter } from 'vue'
import * as Y from 'yjs'
import { HocuspocusProvider } from '@hocuspocus/provider'
import { api } from './api'
import { socketBase } from './useRealtime'
import {
  type CallKind, roomName, tuneOpusSdp, rmsLevel, speakingNext, micError, audioConstraints, isInitiator, newPeerId, DEFAULT_ICE,
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

export interface CallPresence { id: string; user: string; name: string; muted: boolean }
export interface CallPeer { id: string; name: string; muted: boolean; speaking: boolean; volume: number; state: 'connecting' | 'connected' | 'failed' }
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
  pc: RTCPeerConnection
  audio: HTMLAudioElement
  analyser: AnalyserNode | null
  pending: RTCIceCandidateInit[]
}
let local: MediaStream | null = null
let audioCtx: AudioContext | null = null
let localAnalyser: AnalyserNode | null = null
let current: Room | null = null
let listener: ((m: any) => void) | null = null
let iceServers: RTCIceServer[] = DEFAULT_ICE
let meter: ReturnType<typeof setInterval> | null = null
let deviceWatcher: (() => void) | null = null
let leaving = false
const conns = new Map<string, Conn>()

/** Serveurs ICE et limite fournis par le serveur (crochet TURN : #264). */
async function loadConfig() {
  try {
    const c = await api<{ ice_servers?: RTCIceServer[]; max_participants?: number }>('/api/calls/config', { silent: true })
    if (c?.ice_servers?.length) iceServers = c.ice_servers
    if (c?.max_participants) callState.max = c.max_participants
  } catch { iceServers = DEFAULT_ICE }
}

function send(msg: object) {
  try { current?.provider.sendStateless(JSON.stringify(msg)) } catch { /* connexion coupée : le join est renvoyé à la reprise */ }
}
const signal = (to: string, kind: 'offer' | 'answer' | 'ice', data: unknown) => send({ t: 'signal', to, kind, data })
const sendJoin = () => send({ t: 'join', peerId: callState.selfId, muted: callState.muted })

export async function refreshDevices() {
  try {
    const all = await navigator.mediaDevices.enumerateDevices()
    callState.devices = all.filter((d) => d.kind === 'audioinput' && d.deviceId).map((d, i) => ({ id: d.deviceId, label: d.label || `#${i + 1}` }))
  } catch { callState.devices = [] }
}

function analyserFor(stream: MediaStream): AnalyserNode | null {
  try {
    audioCtx ??= new AudioContext()
    const a = audioCtx.createAnalyser()
    a.fftSize = 512
    audioCtx.createMediaStreamSource(stream).connect(a)
    return a
  } catch { return null }
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
  return (callState.peers[id] ??= { id, name, muted: false, speaking: false, volume: 1, state: 'connecting' })
}

function closeConn(id: string) {
  const c = conns.get(id)
  if (!c) return
  conns.delete(id)
  c.pc.onicecandidate = c.pc.ontrack = c.pc.onconnectionstatechange = null
  try { c.pc.close() } catch { /* déjà fermée */ }
  c.audio.srcObject = null
}

function newConn(id: string): Conn {
  closeConn(id)
  const pc = new RTCPeerConnection({ iceServers })
  const audio = new Audio()
  audio.autoplay = true
  audio.volume = peerEntry(id).volume
  const conn: Conn = { pc, audio, analyser: null, pending: [] }
  conns.set(id, conn)
  for (const t of local?.getAudioTracks() ?? []) pc.addTrack(t, local!)
  pc.onicecandidate = (e) => { if (e.candidate) signal(id, 'ice', e.candidate.toJSON()) }
  pc.ontrack = (e) => {
    const stream = e.streams[0] ?? new MediaStream([e.track])
    audio.srcObject = stream
    audio.play().catch(() => { /* reprise par resumeAudio() au prochain geste */ })
    conn.analyser = analyserFor(stream)
  }
  pc.onconnectionstatechange = () => {
    const p = callState.peers[id]
    if (pc.connectionState === 'connected') { if (p) p.state = 'connected'; tuneSenders(pc) }
    else if (pc.connectionState === 'failed') {
      if (p) p.state = 'failed'
      // Reprise : le plus petit identifiant relance l'offre, l'autre attend la sienne.
      closeConn(id)
      if (current && isInitiator(callState.selfId, id) && callState.peers[id]) setTimeout(() => { if (current && callState.peers[id] && !conns.has(id)) void offer(id) }, 1500)
    }
  }
  return conn
}

/** Débit plafonné et priorité haute pour la voix. */
async function tuneSenders(pc: RTCPeerConnection) {
  for (const s of pc.getSenders()) {
    if (s.track?.kind !== 'audio') continue
    try {
      const p = s.getParameters()
      if (!p.encodings?.length) p.encodings = [{}]
      p.encodings[0]!.maxBitrate = 40_000
      ;(p.encodings[0] as any).networkPriority = 'high'
      await s.setParameters(p)
    } catch { /* non pris en charge : sans importance */ }
  }
}

async function offer(id: string) {
  const { pc } = newConn(id)
  try {
    const o = await pc.createOffer()
    await pc.setLocalDescription({ type: 'offer', sdp: tuneOpusSdp(o.sdp ?? '') })
    signal(id, 'offer', pc.localDescription)
  } catch (e) { console.warn('call: offer failed', e) }
}

async function onSignal(m: { from: string; kind: string; data: any }) {
  const { from } = m
  peerEntry(from)
  try {
    if (m.kind === 'offer') {
      const conn = newConn(from)
      await conn.pc.setRemoteDescription(m.data)
      const a = await conn.pc.createAnswer()
      await conn.pc.setLocalDescription({ type: 'answer', sdp: tuneOpusSdp(a.sdp ?? '') })
      signal(from, 'answer', conn.pc.localDescription)
      for (const c of conn.pending.splice(0)) await conn.pc.addIceCandidate(c)
    } else if (m.kind === 'answer') {
      const conn = conns.get(from)
      if (!conn || conn.pc.signalingState !== 'have-local-offer') return
      await conn.pc.setRemoteDescription(m.data)
      for (const c of conn.pending.splice(0)) await conn.pc.addIceCandidate(c)
    } else if (m.kind === 'ice') {
      const conn = conns.get(from)
      if (!conn) return
      if (conn.pc.remoteDescription) await conn.pc.addIceCandidate(m.data)
      else conn.pending.push(m.data)
    }
  } catch (e) { console.warn('call: signal failed', m.kind, e) }
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
    if (!conns.has(p.id) && isInitiator(callState.selfId, p.id)) void offer(p.id)
  }
  for (const id of Object.keys(callState.peers)) {
    if (ids.has(id)) continue
    closeConn(id)
    delete callState.peers[id]
  }
}

function onMessage(m: any) {
  switch (m?.t) {
    case '_synced':
      if (callState.selfId) sendJoin()
      break
    case '_denied':
      void fail('call-denied')
      break
    case 'joined':
      callState.status = 'connected'
      if (m.max) callState.max = m.max
      reconcile(m.peers)
      break
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
    try { await switchInput('') } catch { callState.error = 'mic-lost'; callState.muted = true; send({ t: 'state', muted: true }) }
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
  } catch (e) {
    callState.status = 'idle'
    callState.error = micError(e)
    return
  }
  leaving = false
  callState.room = { name: roomName(kind, id), kind, id, label, to }
  callState.selfId = newPeerId()
  callState.muted = false
  callState.peers = {}
  localAnalyser = analyserFor(local)
  const track = local.getAudioTracks()[0]
  if (track) watchTrack(track)
  void refreshDevices()
  const onChange = () => { void refreshDevices() }
  navigator.mediaDevices.addEventListener?.('devicechange', onChange)
  deviceWatcher = () => navigator.mediaDevices.removeEventListener?.('devicechange', onChange)
  current = acquire(callState.room.name)
  listener = onMessage
  current.listeners.add(listener)
  meter = setInterval(tick, 120)
  // La connexion de signalisation est peut-être déjà synchronisée (pastille ouverte sur la page).
  if (current.provider.synced) sendJoin()
  if (current.denied.value) void fail('call-denied')
  window.addEventListener('pagehide', onPageHide)
}

function onPageHide() { send({ t: 'leave' }) }

export async function leave() {
  if (leaving) return
  leaving = true
  window.removeEventListener('pagehide', onPageHide)
  send({ t: 'leave' })
  if (meter) clearInterval(meter)
  meter = null
  deviceWatcher?.()
  deviceWatcher = null
  for (const id of [...conns.keys()]) closeConn(id)
  local?.getTracks().forEach((t) => { t.onended = null; t.stop() })
  local = null
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
  leaving = false
}

export function toggleMute() {
  if (callState.status === 'idle') return
  callState.muted = !callState.muted
  local?.getAudioTracks().forEach((t) => { t.enabled = !callState.muted })
  send({ t: 'state', muted: callState.muted })
}

/** Change de micro sans renégocier : la piste est remplacée sur chaque connexion. */
export async function switchInput(deviceId: string) {
  const stream = await captureMic(deviceId || undefined)
  const track = stream.getAudioTracks()[0]!
  track.enabled = !callState.muted
  watchTrack(track)
  for (const c of conns.values()) {
    const sender = c.pc.getSenders().find((s) => s.track?.kind === 'audio')
    await sender?.replaceTrack(track)
  }
  local?.getTracks().forEach((t) => { t.onended = null; t.stop() })
  local = stream
  localAnalyser = analyserFor(stream)
  callState.inputId = deviceId
  callState.error = null
  void refreshDevices()
}

export async function chooseInput(deviceId: string) {
  try { await switchInput(deviceId) } catch (e) { callState.error = micError(e) }
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
