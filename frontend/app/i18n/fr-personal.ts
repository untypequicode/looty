export default {
  errors: {
    personal_not_author: 'Seule la personne qui l’a créé peut le rendre personnel.',
    personal_parent: 'Il est rangé dans un élément personnel : il le reste tant que celui-ci l’est.',
    personal_item: 'Élément personnel : personne d’autre ne peut y être assigné.',
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
  personalItem: {
    badge: 'Personnel', title: 'Personnel : visible de vous seul, même par les administrateurs de l’espace',
    create: 'Personnel', createHint: 'Visible de vous seul (même pas des administrateurs).',
    newPersonalDoc: 'Document personnel', newPersonalDocHint: 'Rangé ici, mais visible de vous seul',
    makePersonal: 'Rendre personnel', makeShared: 'Partager avec l’espace',
    makePersonalTitle: 'Rendre « {name} » personnel ?',
    makePersonalHint: {
      page: 'Les autres membres ne verront plus ce document ni ses sous-documents ; ses partages individuels sont retirés.',
      board: 'Les autres membres ne verront plus ce tableau ni ses tâches ; vous restez seul assigné.',
      task: 'Les autres membres ne verront plus cette tâche ni ses sous-tâches ; vous restez seul assigné.',
      project: 'Les autres membres ne verront plus ce projet.',
      file: 'Les autres membres ne verront plus cet élément du Drive (ni son contenu, pour un dossier).',
    },
    nowPersonal: 'Désormais personnel', nowShared: 'Désormais partagé avec l’espace',
    filter: 'Personnels', filterHint: 'Seulement mes éléments personnels',
    moved: 'Vos éléments personnels de cet espace sont dans votre espace perso.',
  },
}
