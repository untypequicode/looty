// node --test frontend/tests/callRtc.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { tuneOpusSdp, OPUS_PARAMS, rmsLevel, speakingNext, SPEAK_ON, SPEAK_OFF, micError, audioConstraints, isInitiator, roomName, newPeerId } from '../app/utils/callRtc.ts'

const SDP = ['v=0', 'm=audio 9 UDP/TLS/RTP/SAVPF 111 63', 'a=rtpmap:111 opus/48000/2', 'a=fmtp:111 minptime=10;useinbandfec=0;stereo=1', 'a=rtpmap:63 red/48000/2', ''].join('\r\n')

test('opus parameters are merged into the existing fmtp line', () => {
  const out = tuneOpusSdp(SDP)
  const line = out.split('\r\n').find((l) => l.startsWith('a=fmtp:111'))!
  const params = Object.fromEntries(line.slice('a=fmtp:111 '.length).split(';').map((p) => p.split('=')))
  for (const [k, v] of Object.entries(OPUS_PARAMS)) assert.equal(params[k], v, k)
  assert.equal(params.useinbandfec, '1', 'FEC forced on')
  assert.equal(params.usedtx, '1', 'DTX on')
  assert.ok(out.includes('a=rtpmap:63 red/48000/2'), 'other lines untouched')
  assert.equal(out.split('\r\n').filter((l) => l.startsWith('a=fmtp:111')).length, 1)
})

test('a missing fmtp line is created right after the rtpmap, and SDP without opus is unchanged', () => {
  const bare = ['m=audio 9 UDP/TLS/RTP/SAVPF 111', 'a=rtpmap:111 opus/48000/2', 'a=rtcp-fb:111 transport-cc'].join('\n')
  const out = tuneOpusSdp(bare).split('\n')
  assert.match(out[2]!, /^a=fmtp:111 .*usedtx=1/)
  assert.equal(out.length, 4)
  const video = 'm=video 9 RTP/AVP 96\na=rtpmap:96 VP8/90000'
  assert.equal(tuneOpusSdp(video), video)
})

test('speaking indicator has hysteresis', () => {
  assert.equal(rmsLevel([]), 0)
  assert.equal(rmsLevel(new Uint8Array(64).fill(128)), 0)
  assert.ok(rmsLevel(Uint8Array.from({ length: 64 }, (_, i) => (i % 2 ? 228 : 28))) > 0.7)
  assert.equal(speakingNext(false, SPEAK_ON - 0.001), false)
  assert.equal(speakingNext(false, SPEAK_ON + 0.001), true)
  assert.equal(speakingNext(true, (SPEAK_ON + SPEAK_OFF) / 2), true, 'stays on between the thresholds')
  assert.equal(speakingNext(true, SPEAK_OFF - 0.001), false)
})

test('microphone errors map to translated codes', () => {
  assert.equal(micError({ name: 'NotAllowedError' }), 'mic-denied')
  assert.equal(micError({ name: 'NotFoundError' }), 'mic-missing')
  assert.equal(micError({ name: 'NotReadableError' }), 'mic-busy')
  assert.equal(micError({ name: 'TypeError' }), 'mic-unsupported')
  assert.equal(micError(new Error('x')), 'mic-failed')
  assert.equal(micError(null), 'mic-failed')
})

test('constraints turn the browser voice processing on', () => {
  const c = audioConstraints('abc') as any
  assert.deepEqual([c.echoCancellation, c.noiseSuppression, c.autoGainControl, c.channelCount], [true, true, true, 1])
  assert.deepEqual(c.deviceId, { exact: 'abc' })
  assert.equal('deviceId' in (audioConstraints() as object), false)
})

test('exactly one side of a pair initiates, room names and peer ids are well formed', () => {
  assert.notEqual(isInitiator('aaa', 'bbb'), isInitiator('bbb', 'aaa'))
  assert.equal(roomName('page', 'x'), 'call:page:x')
  const id = newPeerId()
  assert.match(id, /^[A-Za-z0-9_-]{8,64}$/)
  assert.notEqual(id, newPeerId())
})

// --- Video, screen share, recovery and quality (#265, #266) ---
import {
  cameraProfile, shareProfile, canUseCamera, VIDEO_CAM_MAX, cameraConstraints, displayConstraints, cameraError, shareCancelled,
  backoffDelay, qualityOf, worstQuality, diffCounters, adaptFactor, streamRole, formatBitrate, QUALITY,
} from '../app/utils/callRtc.ts'

test('camera is modest and gets lighter as the room grows; screen share keeps the text sharp at few frames', () => {
  assert.deepEqual([cameraProfile(2).width, cameraProfile(2).height, cameraProfile(2).fps], [640, 360, 24])
  for (let n = 2; n < 6; n++) {
    assert.ok(cameraProfile(n + 1).maxBitrate <= cameraProfile(n).maxBitrate, `camera bitrate non-increasing at ${n}`)
    assert.ok(shareProfile(n + 1).maxBitrate <= shareProfile(n).maxBitrate, `share bitrate non-increasing at ${n}`)
  }
  for (let n = 2; n <= 6; n++) {
    const s = shareProfile(n)
    assert.ok(s.fps >= 5 && s.fps <= 15, `share fps in 5..15 at ${n}`)
    assert.ok(s.width >= 1600, 'share keeps a high resolution')
    // Upload budget of the worst participant (everyone shares + a camera) stays far below a typical 5 Mbit/s uplink at 2..4 people.
    if (n <= 4) assert.ok((n - 1) * (s.maxBitrate + cameraProfile(n).maxBitrate) < 5_000_000, `uplink budget at ${n}`)
  }
  assert.equal(canUseCamera(VIDEO_CAM_MAX), true)
  assert.equal(canUseCamera(VIDEO_CAM_MAX + 1), false)
})

test('capture constraints: camera at the profile size, display capture without voice processing', () => {
  const c = cameraConstraints(cameraProfile(2), 'cam1') as any
  assert.deepEqual([c.width.ideal, c.height.ideal, c.frameRate.max], [640, 360, 24])
  assert.deepEqual(c.deviceId, { exact: 'cam1' })
  assert.equal('deviceId' in (cameraConstraints(cameraProfile(2)) as object), false)
  const d = displayConstraints(shareProfile(3)) as any
  assert.equal(d.audio.echoCancellation, false)
  assert.equal(d.systemAudio, 'include')
  assert.ok(d.video.frameRate.ideal <= 15)
})

test('camera errors map to codes, cancelling the share picker is silent', () => {
  assert.equal(cameraError({ name: 'NotAllowedError' }), 'cam-denied')
  assert.equal(cameraError({ name: 'NotFoundError' }), 'cam-missing')
  assert.equal(cameraError({ name: 'NotReadableError' }), 'cam-busy')
  assert.equal(cameraError(new Error('x')), 'cam-failed')
  assert.equal(shareCancelled({ name: 'NotAllowedError' }), true)
  assert.equal(shareCancelled({ name: 'NotReadableError' }), false)
})

test('ICE restart backoff grows, is capped and jittered', () => {
  const d = [0, 1, 2, 3, 4, 5, 6, 10].map((a) => backoffDelay(a, 0.5))
  assert.deepEqual(d.slice(0, 5), [1000, 2000, 4000, 8000, 16000])
  assert.equal(d[5], 30000)
  assert.equal(d[7], 30000, 'capped at 30 s')
  assert.ok(backoffDelay(2, 0) < backoffDelay(2, 1), 'jitter spreads peers')
  assert.ok(backoffDelay(2, 1) <= 4000 * 1.15 + 1)
})

test('quality indicator follows the documented thresholds', () => {
  assert.equal(qualityOf(null), 'unknown')
  assert.equal(qualityOf({ rtt: null, loss: 0, jitter: 0 }), 'unknown')
  assert.equal(qualityOf({ rtt: 40, loss: 0, jitter: 5 }), 'good')
  assert.equal(qualityOf({ rtt: QUALITY.fairRtt + 1, loss: 0, jitter: 5 }), 'fair')
  assert.equal(qualityOf({ rtt: 40, loss: QUALITY.fairLoss + 0.01, jitter: 5 }), 'fair')
  assert.equal(qualityOf({ rtt: 40, loss: 0, jitter: QUALITY.poorJitter + 1 }), 'poor')
  assert.equal(qualityOf({ rtt: QUALITY.poorRtt + 1, loss: 0, jitter: 0 }), 'poor')
  assert.equal(worstQuality(['good', 'fair', 'unknown']), 'fair')
  assert.equal(worstQuality(['good', 'poor', 'fair']), 'poor')
  assert.equal(worstQuality([]), 'unknown')
})

test('getStats counters become bitrates and a loss ratio', () => {
  const a = { t: 1000, bytesSent: 0, bytesReceived: 0, packetsReceived: 0, packetsLost: 0 }
  const b = { t: 3000, bytesSent: 50_000, bytesReceived: 25_000, packetsReceived: 90, packetsLost: 10 }
  assert.deepEqual(diffCounters(null, b), { up: 0, down: 0, loss: 0 })
  const d = diffCounters(a, b)
  assert.equal(d.up, 200_000)
  assert.equal(d.down, 100_000)
  assert.equal(d.loss, 0.1)
  assert.deepEqual(diffCounters(b, b), { up: 0, down: 0, loss: 0 }, 'same timestamp: no division by zero')
  assert.equal(diffCounters(b, { ...b, t: 4000, bytesSent: 0 }).up, 0, 'a counter reset never gives a negative rate')
  assert.equal(formatBitrate(850_000), '850 kbit/s')
  assert.equal(formatBitrate(1_500_000), '1.5 Mbit/s')
})

test('simulated lossy and high latency link: video cap shrinks fast, recovers slowly, never below 20 %', () => {
  let f = 1
  const trace: number[] = []
  // 6 polls of 10 % loss, then 3 polls of 250 ms latency, then a clean link.
  for (let i = 0; i < 6; i++) { f = adaptFactor(f, { rtt: 80, loss: 0.1 }); trace.push(f) }
  assert.ok(trace[0]! < 1 && trace.every((v, i) => i === 0 || v <= trace[i - 1]!), 'monotonic decrease')
  assert.equal(f, 0.2, 'floor')
  for (let i = 0; i < 3; i++) f = adaptFactor(f, { rtt: 250, loss: 0 })
  assert.ok(f < 0.2 * 1.01 && f >= 0.2, 'latency alone keeps shrinking, not below the floor')
  let polls = 0
  while (f < 1 && polls < 100) { f = adaptFactor(f, { rtt: 30, loss: 0 }); polls++ }
  assert.equal(f, 1)
  assert.ok(polls > 10, 'recovery is slower than the drop')
})

test('received streams are classified by the ids announced in the room', () => {
  const ann = { cam: 'C1', share: 'S1' }
  assert.equal(streamRole('S1', ann), 'share')
  assert.equal(streamRole('C1', ann), 'cam')
  assert.equal(streamRole('V1', ann), 'voice')
  assert.equal(streamRole('C1', { cam: '', share: '' }), 'voice', 'camera off: nothing announced')
})
