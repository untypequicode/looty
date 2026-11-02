export default {
  errors: {
    announcement_too_long: 'Cette annonce est trop longue.', poll_closed: 'Ce sondage est clos.', poll_single_choice: 'Un seul choix possible dans ce sondage.',
    poll_invalid_option: 'Option de sondage invalide.', poll_option_count: 'Un sondage a de 2 à 10 options.', poll_invalid_question: 'La question du sondage est vide ou trop longue.',
  },
  announcements: {
    title: 'Annonces', new: 'Nouvelle annonce', edit: 'Modifier l’annonce', titleLabel: 'Titre', titlePlaceholder: 'Changement de salle, date du partiel…',
    bodyPlaceholder: 'Le détail de l’annonce…', pinned: 'Épingler en haut', pinnedHint: 'Reste en tête de liste et sur l’accueil.',
    expires: 'Expire le', expiresHint: 'Ensuite, seules les personnes qui gèrent les annonces la voient encore.', noExpiry: 'Jamais',
    publish: 'Publier', save: 'Enregistrer', delete: 'Supprimer l’annonce', deleteConfirm: 'Supprimer « {title} » ? Les membres ne la verront plus.',
    empty: 'Aucune annonce pour l’instant.', emptyHint: 'Les informations importantes de l’espace apparaîtront ici.', emptyManage: 'Publiez une information pour tous les membres : elle leur est notifiée.',
    markAllRead: 'Tout marquer comme lu', unread: 'Nouveau', expired: 'Expirée', pinnedBadge: 'Épinglée', by: 'Par {name}', edited: 'modifiée', published: 'Annonce publiée',
  },
  dashboard: {
    widgets: { announcements: 'Annonces' },
    widgetHints: { announcements: 'Les annonces épinglées et récentes de l’espace' },
    empty: { announcements: 'Aucune annonce.' },
    notif: { announcement: 'a publié une annonce :' },
  },
  inbox: { kinds: { announcement: '{name} a publié une annonce' } },
  settings: { notify: { ev: { announcement: 'Annonces de l’espace' }, evHint: { announcement: 'Une annonce est publiée dans un de vos espaces.' } } },
  rolesUi: { flags: { announcements: 'Publier des annonces', announcementsHint: 'Publier, modifier et supprimer les annonces de l’espace (notifiées à tous les membres).' } },
  polls: {
    add: 'Ajouter un sondage', addHint: 'Les membres votent directement dans l’annonce.', question: 'Question', questionPlaceholder: 'Quel soir pour la sortie de promo ?',
    optionPlaceholder: 'Option {n}', addOption: 'Ajouter une option', multiple: 'Plusieurs choix possibles',
    anonymousLabel: 'Vote anonyme', anonymousHint: 'Seuls les totaux sont visibles, jamais qui a voté quoi.',
    hideResults: 'Résultats après le vote', hideResultsHint: 'Les totaux restent cachés à qui n’a pas encore voté, jusqu’à la clôture.',
    closesLabel: 'Clôture', locked: 'Des votes ont été reçus : seules la question et la clôture peuvent encore changer.',
    singleHint: 'Un seul choix', multipleHint: 'Plusieurs choix possibles', anonymous: 'Anonyme', closed: 'Clos', closesAt: 'Clôture le {date}',
    voters: { zero: 'Aucun vote', one: '1 votant', other: '{count} votants' }, resultsAfterVote: 'résultats visibles après votre vote',
    vote: 'Voter', change: 'Changer mon vote', retract: 'Retirer mon vote', close: 'Clore le sondage', voted: 'Voté', toVote: 'Sondage',
  },
}
