export default {
  errors: {
    invalid_provider: 'Fournisseur inconnu', api_key_required: 'Une clé d’API est nécessaire pour ce fournisseur',
    base_url_required: 'L’adresse du serveur est obligatoire', invalid_base_url: 'Adresse invalide (https:// et adresse publique, sauf si un administrateur autorise les réseaux privés avec AI_ALLOW_PRIVATE)',
    too_many_connections: 'Limite de 20 connexions atteinte', no_connection: 'Choisissez d’abord une connexion IA',
    api_key_unreadable: 'La clé enregistrée est illisible : saisissez-la à nouveau', invalid_mode: 'Mode d’écriture invalide',
    invalid_expiry: 'Durée invalide', invalid_message: 'Message invalide', invalid_context: 'Contexte invalide',
    too_many_tool_rounds: 'L’assistant a tourné en rond avec ses outils, réessayez',
  },
  ai: {
    title: 'Assistant IA',
    settings: {
      title: 'Connexions IA',
      description: 'Reliez vos propres comptes d’IA. Les clés sont chiffrées sur ce serveur et ne servent qu’à vos conversations.',
      add: 'Ajouter une connexion', edit: 'Modifier la connexion', none: 'Aucune connexion pour le moment.',
      privacy: 'L’assistant ne lit que ce que vous pouvez consulter et n’écrit jamais sans votre autorisation explicite, page par page.',
      provider: 'Fournisseur', label: 'Nom', labelPlaceholder: 'ex. Compte du travail', model: 'Modèle', baseUrl: 'Adresse du serveur', optional: 'facultatif',
      apiKey: 'Clé d’API', keepKey: 'Laisser vide pour conserver la clé actuelle',
      deleteTitle: 'Supprimer « {name} » ?', deleteMessage: 'Les conversations qui l’utilisent gardent leur historique mais demanderont une autre connexion.',
    },
    chat: {
      new: 'Nouvelle conversation', untitled: 'Sans titre', connection: 'Connexion', context: 'Contexte', history: 'Historique',
      ctx: { page: 'Cette page', project: 'Ce projet', workspace: 'Cet espace', none: 'Global' },
      noConnection: 'Ajoutez une connexion IA dans les réglages pour commencer.', setUp: 'Ouvrir les réglages',
      readOnlyHint: 'Lecture seule : l’assistant lit les pages auxquelles vous avez accès. Pour qu’il écrive, choisissez une page ci-dessous.',
      placeholder: 'Posez une question… (Entrée pour envoyer)', send: 'Envoyer', stop: 'Arrêter',
      writeTarget: 'Page modifiable', readOnly: 'lecture seule', workspace: 'Espace', page: 'Page', modeLabel: 'Mode', expiry: 'Durée', allow: 'Autoriser',
      mode: { suggest: 'Suggérer', append: 'Ajouter', replace: 'Remplacer' },
      modeHint: {
        suggest: 'Suggérer : l’assistant propose une modification que vous acceptez ou refusez.',
        append: 'Ajouter : l’assistant peut ajouter du contenu à la fin de la page. Une version précédente est conservée.',
        replace: 'Remplacer : l’assistant peut réécrire toute la page. Une version précédente est conservée.',
      },
      suggestion: 'Modification proposée', accept: 'Accepter', reject: 'Refuser', applied: 'Modification appliquée (version précédente gardée dans l’historique)',
    },
  },
  nav: { aiAssistant: 'Assistant IA' },
}
