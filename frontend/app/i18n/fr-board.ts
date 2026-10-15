export default {
  attachments: { title: 'Pièces jointes', drop: 'Déposez des fichiers ici ou cliquez pour importer', deleteTitle: 'Supprimer cette pièce jointe ?' },
  board: {
    addColumn: 'Ajouter une colonne', addTask: 'Ajouter une tâche', columnName: 'Nom de la colonne', display: 'Affichage',
    automations: 'Automatisations', settings: 'Paramètres du tableau', doneColumn: 'Colonne « Terminé »',
    doneColumnHint: 'Les tâches terminées y sont déplacées automatiquement, et retournent à leur colonne si on les rouvre.',
    noDoneColumn: 'Aucune', autoValidate: 'Valider en déplaçant', autoValidateHint: 'Déposer une tâche dans la colonne « Terminé » la marque comme terminée.',
    applySubtasks: 'Appliquer aux sous-tâches', applySubtasksHint: 'Les sous-tâches terminées suivent aussi la règle de déplacement.',
    markAsDone: 'Colonne « Terminé »', newLabel: 'Nouveau label…', noLabels: 'Aucun label sur ce tableau',
    emptyColumn: 'Aucune tâche', quickAddHint: 'pour créer · Échap pour fermer',
    notFound: 'Tableau introuvable', notFoundHint: 'Il a peut-être été supprimé ou vous n’y avez pas accès.',
    views: { board: 'Tableau', list: 'Liste', timeline: 'Chronologie' },
  },
  comments: {
    activity: 'Activité', showHistory: 'Historique', placeholder: 'Laisser un commentaire… (@ pour mentionner)', send: 'Commenter',
    edited: 'modifié', deleteTitle: 'Supprimer ce commentaire ?',
  },
  filters: {
    clear: 'Tout effacer', searchTasks: 'Rechercher une tâche…', completed: 'Tâches terminées', doneShow: 'Afficher', doneHide: 'Masquer', doneMine: 'Masquer les miennes',
    showSubtasks: 'Afficher les sous-tâches comme cartes', showing: '{shown} sur {total}', unassigned: 'Non assignée',
    due: { overdue: 'En retard', today: 'Aujourd’hui', week: 'Cette semaine', none: 'Sans échéance' },
  },
  history: {
    created: 'a créé la tâche', moved: 'a déplacé de {from} vers {to}', completed: 'a terminé la tâche', reopened: 'a rouvert la tâche',
    assigned: 'a assigné {name}', unassigned: 'a retiré {name}', labelAdded: 'a ajouté le label {name}', labelRemoved: 'a retiré le label {name}',
    attached: 'a joint {name}', renamed: 'a renommé en « {title} »', description: 'a modifié la description',
    priority: 'a changé la priorité : {p}', due: 'a fixé l’échéance au {date}', dueRemoved: 'a retiré l’échéance', updated: 'a modifié la tâche',
    restore: 'Restaurer cette version', restoreTitle: 'Restaurer cette version ?', restoreMessage: 'La valeur actuelle sera remplacée (l’opération reste dans l’historique).',
  },
  task: {
    addSubtask: 'Ajouter une sous-tâche', subtasks: 'Sous-tâches', subtaskPlaceholder: 'Titre de la sous-tâche… (Entrée)',
    assignees: 'Assignés', copyId: 'Copier l’identifiant', createdAgo: 'Créée {time}', updatedAgo: 'Modifiée {time}', noDate: 'Aucune date',
    pending: 'En attente', validated: 'Validé', perPerson: 'Validation par personne', perPersonHint: 'Chaque assigné valide de son côté.',
    priority: 'Priorité', reopen: 'Rouvrir', requireAll: 'Tous doivent valider', startDate: 'Début', status: 'Statut', unassigned: 'Non assignée',
  },
  timeline: {
    days: 'Jours', weeks: 'Semaines', hint: 'Glissez une barre pour la déplacer, ses bords pour la redimensionner.',
    unscheduled: 'Non planifiées', clickToSchedule: 'Cliquez pour planifier à cette date',
  },
}
