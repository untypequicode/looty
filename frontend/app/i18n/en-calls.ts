export default {
  calls: {
    join: 'Join call', start: 'Start call', leave: 'Leave', inCall: 'In call', count: { one: '{count} person in the call', other: '{count} people in the call' },
    joinHint: 'Voice call with the people who can open this item. Audio goes directly between participants.',
    bar: 'Voice call', mute: 'Mute', unmute: 'Unmute', muteShortcut: 'Mute / unmute the microphone',
    microphone: 'Microphone', defaultMic: 'Default microphone', settings: 'Call settings',
    volume: 'Volume of {name}', muted: 'Muted', speaking: 'Speaking', connecting: 'Connecting…', failed: 'Connection failed',
    alone: 'Waiting for others to join…', you: 'You', open: 'Open', unknown: 'Participant',
    participants: 'Participants', max: 'Up to {max} people',
    errors: {
      'mic-denied': 'Microphone access is blocked. Allow it in your browser settings, then try again.',
      'mic-missing': 'No microphone found. Plug one in, then try again.',
      'mic-busy': 'The microphone is used by another application.',
      'mic-unsupported': 'This browser or connection (HTTPS required) can’t make voice calls.',
      'mic-failed': 'The microphone can’t be used.',
      'mic-lost': 'The microphone was unplugged. You can still listen; pick another one to speak.',
      'room-full': 'This call is full.',
      'call-denied': 'You can’t join this call.',
      'call-unavailable': 'The call service is unavailable.',
    },
  },
}
