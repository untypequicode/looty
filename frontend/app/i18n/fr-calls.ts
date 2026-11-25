export default {
  calls: {
    join: 'Rejoindre l’appel', start: 'Lancer un appel', leave: 'Quitter', inCall: 'En appel', count: { one: '{count} personne dans l’appel', other: '{count} personnes dans l’appel' },
    joinHint: 'Appel vocal avec les personnes qui peuvent ouvrir cet élément. L’audio passe directement entre les participants.',
    bar: 'Appel vocal', mute: 'Couper le micro', unmute: 'Réactiver le micro', muteShortcut: 'Couper / réactiver le micro',
    microphone: 'Microphone', defaultMic: 'Microphone par défaut', settings: 'Réglages de l’appel',
    volume: 'Volume de {name}', muted: 'Micro coupé', speaking: 'Parle', connecting: 'Connexion…', failed: 'Connexion échouée',
    alone: 'En attente d’autres participants…', you: 'Vous', open: 'Ouvrir', unknown: 'Participant',
    participants: 'Participants', max: 'Jusqu’à {max} personnes',
    errors: {
      'mic-denied': 'L’accès au micro est bloqué. Autorisez-le dans les réglages du navigateur, puis réessayez.',
      'mic-missing': 'Aucun micro trouvé. Branchez-en un, puis réessayez.',
      'mic-busy': 'Le micro est utilisé par une autre application.',
      'mic-unsupported': 'Ce navigateur ou cette connexion (HTTPS requis) ne permet pas les appels vocaux.',
      'mic-failed': 'Le micro est inutilisable.',
      'mic-lost': 'Le micro a été débranché. Vous pouvez toujours écouter ; choisissez-en un autre pour parler.',
      'room-full': 'Cet appel est complet.',
      'call-denied': 'Vous ne pouvez pas rejoindre cet appel.',
      'call-unavailable': 'Le service d’appel est indisponible.',
    },
  },
}
