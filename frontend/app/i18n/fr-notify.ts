export default {
  errors: {
    smtp_not_configured: 'L’envoi d’e-mails n’est pas configuré', smtp_incomplete: 'Le serveur et l’adresse d’expédition sont requis',
    smtp_invalid_from: 'Adresse d’expédition invalide', smtp_invalid_recipient: 'Adresse du destinataire invalide',
    smtp_send_failed: 'Le serveur SMTP a refusé l’e-mail', smtp_tls: 'Erreur TLS avec le serveur SMTP', smtp_message: 'Impossible de construire l’e-mail',
    push_invalid_endpoint: 'Le service push de ce navigateur n’est pas autorisé', push_invalid_keys: 'Abonnement push invalide', push_too_many: 'Trop d’appareils (20 max.)',
    invalid_event: 'Type de notification inconnu', invalid_value: 'Valeur invalide',
  },
  inbox: { kinds: { due_soon: 'Échéance proche : {title}', completed: '{name} a terminé {title}' } },
  settings: {
    nav: { notifications: 'Notifications', email: 'E-mail' },
    notify: {
      channels: 'Canaux', channelsHint: 'Les notifications apparaissent toujours dans votre boîte de réception. Choisissez où les recevoir en plus.',
      emailLabel: 'Notifications par e-mail', emailHint: 'Envoyées à {email}', emailUnavailable: 'L’envoi d’e-mails n’est pas encore configuré sur cette instance.',
      pushLabel: 'Notifications push', pushHint: 'Sur votre ordinateur ou votre téléphone, même quand Looty est fermé.',
      pushIos: 'Sur iPhone et iPad, les notifications demandent l’application sur l’écran d’accueil : dans Safari, touchez Partager → « Sur l’écran d’accueil », ouvrez Looty depuis là, puis revenez ici (iOS 16.4 ou plus).', pushUnsupported: 'Indisponible dans ce navigateur (une connexion https sécurisée est nécessaire).', pushDenied: 'Bloquées dans les réglages du site de ce navigateur.',
      pushOnHere: 'Activées sur cet appareil', enableHere: 'Activer sur cet appareil', pushEnabled: 'Notifications push activées sur cet appareil',
      enableDesktop: 'Notifier quand Looty est ouvert', desktopOn: 'Quand Looty est ouvert',
      appLabel: 'Notifications de l’application', appHint: 'Notifications du système envoyées par l’application de bureau, même fenêtre fermée (aussi dans le menu de l’icône Looty).',
      events: 'Ce que vous recevez', eventsHint: 'Choisissez un canal pour chaque type d’événement.', email: 'E-mail', push: 'Push',
      ev: {
        task_assigned: 'Assignation à une tâche', task_mention: 'Mention dans une tâche', doc_mention: 'Mention dans un document',
        task_comment: 'Commentaires sur mes tâches', task_due: 'Rappels d’échéance', task_completed: 'Mes tâches terminées par quelqu’un d’autre',
      },
      evHint: {
        task_assigned: 'Quelqu’un vous assigne une tâche.', task_mention: 'Quelqu’un vous @mentionne dans un commentaire de tâche.', doc_mention: 'Quelqu’un vous @mentionne dans une page du wiki.',
        task_comment: 'Nouveaux commentaires sur les tâches qui vous sont assignées.', task_due: 'Tâches assignées arrivant à échéance dans les 24 heures.', task_completed: 'Quelqu’un termine une tâche qui vous est assignée.',
      },
      noDeviceNote: 'Le push n’est envoyé qu’aux appareils où vous l’avez activé.',
      emailDelayNote: 'Les e-mails partent quelques minutes plus tard, et ne sont pas envoyés si vous avez déjà lu la notification dans Looty.',
      devices: 'Appareils', devicesHint: 'Navigateurs qui reçoivent vos notifications push.', noDevices: 'Aucun appareil pour l’instant.',
      thisDevice: 'Cet appareil', test: 'Envoyer un test', testSent: 'Test envoyé à {n} appareil(s)', testNone: 'Aucun appareil n’a pu être joint',
    },
    mail: {
      title: 'E-mail (SMTP)', description: 'Serveur utilisé pour envoyer les notifications par e-mail. Chaque utilisateur choisit ce qu’il reçoit dans ses réglages.',
      test: 'Envoyer un e-mail de test', testSent: 'E-mail de test envoyé à {email}',
      envActive: 'La configuration e-mail provient actuellement de l’environnement du serveur (variables SMTP_*). Activez ce formulaire pour la remplacer.',
      enabled: 'Envoyer des e-mails', enabledHint: 'Utiliser les réglages ci-dessous.', host: 'Serveur SMTP', security: 'Sécurité',
      securityHint: 'STARTTLS sur le port 587, TLS sur le port 465.', none: 'Aucune', port: 'Port', username: 'Nom d’utilisateur', password: 'Mot de passe',
      passwordKept: 'Laisser vide pour conserver le mot de passe actuel.', from: 'Adresse d’expédition', fromHint: 'Doit être autorisée par votre serveur SMTP.',
      fromName: 'Nom de l’expéditeur', delay: 'Délai des e-mails (minutes)', delayHint: 'Laisse le temps de lire la notification dans l’app ; 0 envoie immédiatement.',
    },
  },
}
