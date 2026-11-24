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
