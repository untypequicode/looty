/**
 * Logique pure des appels (ADR 0019) : noms de salle, réglage Opus voix, indicateur de parole,
 * erreurs du micro, qui lance l'offre. Sans navigateur, donc testable seule (tests/callRtc.test.ts).
 */

export type CallKind = 'page' | 'project' | 'workspace'

export const roomName = (kind: CallKind, id: string) => `call:${kind}:${id}`

/** Paramètres Opus pour la voix : correction d'erreur (FEC), silence non transmis (DTX), mono, 32 kbit/s. */
export const OPUS_PARAMS: Record<string, string> = {
  useinbandfec: '1',
  usedtx: '1',
  stereo: '0',
  'sprop-stereo': '0',
  maxaveragebitrate: '32000',
  maxplaybackrate: '48000',
  minptime: '10',
}

/** Ajoute les paramètres voix à la ligne fmtp d'Opus (la ligne est créée si le navigateur n'en met pas). */
export function tuneOpusSdp(sdp: string): string {
  const nl = sdp.includes('\r\n') ? '\r\n' : '\n'
  const lines = sdp.split(nl)
  const rtpmap = lines.findIndex((l) => /^a=rtpmap:\d+ opus\/48000/i.test(l))
  if (rtpmap < 0) return sdp
  const pt = lines[rtpmap]!.match(/^a=rtpmap:(\d+) /)![1]!
  const fmtp = lines.findIndex((l) => l.startsWith(`a=fmtp:${pt} `))
  const merged = new Map<string, string>()
  if (fmtp >= 0) {
    for (const part of lines[fmtp]!.slice(`a=fmtp:${pt} `.length).split(';')) {
      const [k, v] = part.trim().split('=')
      if (k) merged.set(k, v ?? '')
    }
  }
  for (const [k, v] of Object.entries(OPUS_PARAMS)) merged.set(k, v)
  const line = `a=fmtp:${pt} ${[...merged].map(([k, v]) => (v === '' ? k : `${k}=${v}`)).join(';')}`
  if (fmtp >= 0) lines[fmtp] = line
  else lines.splice(rtpmap + 1, 0, line)
  return lines.join(nl)
}

/** Niveau (0..1) d'un échantillon temporel de l'analyseur audio (octets centrés sur 128). */
export function rmsLevel(samples: ArrayLike<number>): number {
  if (!samples.length) return 0
  let sum = 0
  for (let i = 0; i < samples.length; i++) {
    const v = (samples[i]! - 128) / 128
    sum += v * v
  }
  return Math.sqrt(sum / samples.length)
}

/** Seuils différents pour s'allumer et s'éteindre : l'indicateur ne clignote pas sur une syllabe. */
export const SPEAK_ON = 0.04
export const SPEAK_OFF = 0.02
export function speakingNext(prev: boolean, level: number): boolean {
  return prev ? level > SPEAK_OFF : level > SPEAK_ON
}

export type MicError = 'mic-denied' | 'mic-missing' | 'mic-busy' | 'mic-unsupported' | 'mic-failed'

/** Erreur de getUserMedia -> code traduit dans l'interface (calls.errors.*). */
export function micError(e: unknown): MicError {
  const name = (e as { name?: string } | null)?.name
  if (name === 'NotAllowedError' || name === 'SecurityError' || name === 'PermissionDeniedError') return 'mic-denied'
  if (name === 'NotFoundError' || name === 'OverconstrainedError' || name === 'DevicesNotFoundError') return 'mic-missing'
  if (name === 'NotReadableError' || name === 'AbortError' || name === 'TrackStartError') return 'mic-busy'
  if (name === 'TypeError') return 'mic-unsupported'
  return 'mic-failed'
}

/** Contraintes micro : traitements du navigateur activés (écho, bruit, gain), un seul canal. */
export function audioConstraints(deviceId?: string | null): MediaTrackConstraints {
  return {
    echoCancellation: true,
    noiseSuppression: true,
    autoGainControl: true,
    channelCount: 1,
    ...(deviceId ? { deviceId: { exact: deviceId } } : {}),
  }
}

/** Entre deux participants, un seul lance l'offre (le plus petit identifiant) : pas de collision. */
export const isInitiator = (me: string, other: string) => me < other

export function newPeerId(): string {
  const bytes = new Uint8Array(12)
  crypto.getRandomValues(bytes)
  return [...bytes].map((b) => b.toString(16).padStart(2, '0')).join('')
}

/** Serveurs ICE par défaut si la configuration du serveur est indisponible. */
export const DEFAULT_ICE: RTCIceServer[] = [{ urls: ['stun:stun.l.google.com:19302'] }]

// --- Video, screen share and connection quality (ADR 0019, #265 and #266) ---

/** Mesh limits: every camera is uploaded N-1 times, so cameras stop above this many people. */
export const VIDEO_CAM_MAX = 4
/** Screen share stays allowed up to the room cap, at a lower frame rate and bitrate when the room grows. */
export const AUDIO_MAX_BITRATE = 40_000

export interface VideoProfile { width: number; height: number; fps: number; maxBitrate: number }

/** Camera: modest by default (640x360 at 24 fps), lower when more people have to receive it. */
export function cameraProfile(participants: number): VideoProfile {
  if (participants <= 2) return { width: 640, height: 360, fps: 24, maxBitrate: 600_000 }
  if (participants <= 3) return { width: 480, height: 270, fps: 20, maxBitrate: 350_000 }
  return { width: 320, height: 180, fps: 15, maxBitrate: 200_000 }
}

/** Screen share is tuned for text: sharp, few frames per second, the bitrate shrinks with the audience. */
export function shareProfile(participants: number): VideoProfile {
  if (participants <= 2) return { width: 1920, height: 1080, fps: 15, maxBitrate: 1_500_000 }
  if (participants <= 4) return { width: 1920, height: 1080, fps: 10, maxBitrate: 900_000 }
  return { width: 1600, height: 900, fps: 5, maxBitrate: 600_000 }
}

export const canUseCamera = (participants: number) => participants <= VIDEO_CAM_MAX

export function cameraConstraints(profile: VideoProfile, deviceId?: string | null): MediaTrackConstraints {
  return {
    width: { ideal: profile.width },
    height: { ideal: profile.height },
    frameRate: { ideal: profile.fps, max: profile.fps },
    ...(deviceId ? { deviceId: { exact: deviceId } } : {}),
  }
}

export function displayConstraints(profile: VideoProfile): DisplayMediaStreamOptions & Record<string, unknown> {
  return {
    video: { width: { max: profile.width }, height: { max: profile.height }, frameRate: { ideal: profile.fps, max: Math.max(profile.fps, 15) } },
    // Voice processing would mangle system sound (music, videos): off for the shared audio.
    audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: false },
    systemAudio: 'include',
    surfaceSwitching: 'include',
    selfBrowserSurface: 'exclude',
  }
}

export type CameraError = 'cam-denied' | 'cam-missing' | 'cam-busy' | 'cam-failed'
export function cameraError(e: unknown): CameraError {
  const name = (e as { name?: string } | null)?.name
  if (name === 'NotAllowedError' || name === 'SecurityError' || name === 'PermissionDeniedError') return 'cam-denied'
  if (name === 'NotFoundError' || name === 'OverconstrainedError' || name === 'DevicesNotFoundError') return 'cam-missing'
  if (name === 'NotReadableError' || name === 'AbortError' || name === 'TrackStartError') return 'cam-busy'
  return 'cam-failed'
}

/** Cancelling the browser picker is not an error worth showing. */
export const shareCancelled = (e: unknown) => ['NotAllowedError', 'AbortError'].includes((e as { name?: string } | null)?.name ?? '')

/** Reconnection delay in ms: 1 s, 2 s, 4 s ... capped at 30 s, with a little jitter so peers do not collide. */
export function backoffDelay(attempt: number, random = Math.random()): number {
  const base = Math.min(30_000, 1000 * 2 ** Math.max(0, attempt))
  return Math.round(base * (0.85 + 0.3 * random))
}

/** After a signaling drop longer than this, the server has removed us (grace 15 s): rebuild the connections. */
export const SIGNALING_GRACE_MS = 12_000

export interface LinkSample { rtt: number | null; loss: number; jitter: number; up: number; down: number }
export type Quality = 'good' | 'fair' | 'poor' | 'unknown'

/** Thresholds documented in the calls page: below fair is comfortable, above poor is hard to talk over. */
export const QUALITY = { fairRtt: 150, poorRtt: 300, fairLoss: 0.03, poorLoss: 0.08, fairJitter: 30, poorJitter: 60 }

export function qualityOf(s: Pick<LinkSample, 'rtt' | 'loss' | 'jitter'> | null): Quality {
  if (!s || s.rtt == null) return 'unknown'
  if (s.rtt > QUALITY.poorRtt || s.loss > QUALITY.poorLoss || s.jitter > QUALITY.poorJitter) return 'poor'
  if (s.rtt > QUALITY.fairRtt || s.loss > QUALITY.fairLoss || s.jitter > QUALITY.fairJitter) return 'fair'
  return 'good'
}

const QUALITY_ORDER: Quality[] = ['unknown', 'good', 'fair', 'poor']
export const worstQuality = (list: Quality[]): Quality => list.reduce<Quality>((w, q) => (QUALITY_ORDER.indexOf(q) > QUALITY_ORDER.indexOf(w) ? q : w), 'unknown')

export interface RawCounters { t: number; bytesSent: number; bytesReceived: number; packetsReceived: number; packetsLost: number }

/** Turns two cumulative counters snapshots into rates (bits/s) and a loss ratio over the interval. */
export function diffCounters(prev: RawCounters | null, cur: RawCounters): { up: number; down: number; loss: number } {
  if (!prev || cur.t <= prev.t) return { up: 0, down: 0, loss: 0 }
  const s = (cur.t - prev.t) / 1000
  const received = Math.max(0, cur.packetsReceived - prev.packetsReceived)
  const lost = Math.max(0, cur.packetsLost - prev.packetsLost)
  return {
    up: Math.max(0, Math.round(((cur.bytesSent - prev.bytesSent) * 8) / s)),
    down: Math.max(0, Math.round(((cur.bytesReceived - prev.bytesReceived) * 8) / s)),
    loss: received + lost > 0 ? lost / (received + lost) : 0,
  }
}

/** Multiplier (0.2..1) applied to the video bitrate caps of one link: shrinks fast on loss, grows back slowly. Audio is never touched. */
export function adaptFactor(prev: number, s: Pick<LinkSample, 'rtt' | 'loss'>): number {
  if (s.loss > QUALITY.poorLoss || (s.rtt ?? 0) > QUALITY.poorRtt) return Math.max(0.2, prev * 0.6)
  if (s.loss > QUALITY.fairLoss || (s.rtt ?? 0) > QUALITY.fairRtt) return Math.max(0.2, prev * 0.85)
  return Math.min(1, prev * 1.1)
}

export const formatBitrate = (bps: number) => (bps >= 1_000_000 ? `${(bps / 1e6).toFixed(1)} Mbit/s` : `${Math.round(bps / 1000)} kbit/s`)

/** Which stream of a remote peer is what: by the stream ids it announced through the signaling room. */
export function streamRole(streamId: string, announced: { cam?: string; share?: string }): 'cam' | 'share' | 'voice' {
  if (announced.share && streamId === announced.share) return 'share'
  if (announced.cam && streamId === announced.cam) return 'cam'
  return 'voice'
}
