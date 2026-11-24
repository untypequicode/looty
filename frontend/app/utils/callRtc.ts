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
