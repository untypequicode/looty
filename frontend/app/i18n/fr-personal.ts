export default {
  errors: {
    personal_space: 'Un espace perso reste privé : on ne peut ni y inviter quelqu’un, ni le quitter, ni le supprimer.',
    move_restricted_subpages: 'Certaines sous-pages ne vous sont pas modifiables : impossible de déplacer cette page avec elles.',
  },
  move: {
    action: 'Déplacer vers un autre espace…', title: 'Déplacer « {name} »', hint: 'La page change d’espace avec toutes ses sous-pages, son projet, son canevas, ses commentaires et son historique.',
    to: 'Vers', where: 'Emplacement', root: 'À la racine des documents', searchPage: 'Rechercher une page', submit: 'Déplacer',
    note: 'Les partages individuels sont retirés, et les liens vers des éléments de l’ancien espace (tableaux, événements) ne suivent pas.',
    noSpace: 'Aucun autre espace où vous pouvez créer des documents.', open: 'Ouvrir',
    done: { one: 'Page déplacée vers {name}', other: '{count} pages déplacées vers {name}' }, dropHere: 'Déposer pour déplacer ici',
  },
  personal: {
    title: 'Espace perso', new: 'Créer dans mon espace perso', all: 'Tout voir', createFirst: 'Créer un document perso',
    open: 'Mon espace perso', private: 'Privé', hint: 'Visible de vous seul, depuis tous vos espaces : documents, tableaux, projets, calendrier, mail, drive.',
    start: 'Commencer dans mon espace perso',
  },
}
