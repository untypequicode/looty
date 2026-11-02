export default {
  errors: { announcement_too_long: 'Cette annonce est trop longue.' },
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
}
