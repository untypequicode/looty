export default {
  spell: {
    title: 'Écriture', check: 'Correcteur orthographique', checkHint: 'Souligne les fautes dans les documents, les descriptions et les commentaires.',
    lang: 'Langue de vérification', langHint: 'Dictionnaire utilisé par le navigateur ou l’application de bureau.', auto: 'Langue de l’interface',
  },
  notepad: {
    title: 'Bloc-notes', open: 'Bloc-notes', hint: 'Vos notes personnelles, les mêmes sur toutes les pages et tous vos appareils. Vous seul les voyez.',
    placeholder: 'Notez quelque chose…', saved: 'Enregistré', saving: 'Enregistrement…', close: 'Fermer le bloc-notes', updatedElsewhere: 'Modifié sur un autre appareil',
  },
  calColor: { change: 'Changer la couleur', reset: 'Couleur par défaut' },
  pageMention: { pages: 'Documents', people: 'Personnes', none: 'Aucun document trouvé', linkPage: 'Lier un document' },
  itemComments: {
    title: 'Commentaires', empty: 'Aucun commentaire. Sélectionnez du texte dans le document pour commenter un passage.', resolve: 'Résoudre', reopen: 'Rouvrir', resolved: 'Résolu',
    reply: 'Répondre', replyPlaceholder: 'Répondre… (@ pour mentionner)', showResolved: 'Afficher {n} résolu(s)', hideResolved: 'Masquer les résolus', commentSelection: 'Commenter',
  },
  history: {
    title: 'Historique des versions', empty: 'Aucune version enregistrée pour l’instant.', latest: 'Version actuelle', restore: 'Restaurer cette version', restoreTitle: 'Restaurer cette version ?',
    restoreHint: 'Le document reviendra à son état du {time}. Le contenu actuel est d’abord enregistré comme version : rien n’est perdu.',
    restored: 'Version restaurée', beforeRestore: 'Avant restauration', saveVersion: 'Enregistrer une version nommée', namePrompt: 'Nom de cette version', namedDefault: 'Version nommée',
    saved: 'Version enregistrée', rename: 'Nommer cette version', hint: 'Une version est enregistrée à chaque session d’édition.',
  },
  exportFormats: { htmlHint: 'Pages web lisibles dans n’importe quel navigateur, avec images et formules.', pdfHint: 'Imprimer ou enregistrer en PDF (pages à la suite).', print: 'Imprimer / PDF', printPdf: 'Imprimer ou enregistrer en PDF' },
  reader: {
    title: 'Vue de lecture', open: 'Vue de lecture', leave: 'Retour au document', edit: 'Modifier', outline: 'Sommaire', onThisPage: 'Sur cette page', settings: 'Réglages du texte',
    size: 'Taille du texte', smaller: 'Plus petit', larger: 'Plus grand', font: 'Police', fonts: { serif: 'Serif', sans: 'Sans', legible: 'Lisible' },
    width: 'Largeur des lignes', widths: { narrow: 'Étroite', normal: 'Normale', wide: 'Large' }, spacing: 'Interligne', tight: 'Serré', normal: 'Normal', loose: 'Aéré',
    theme: 'Thème', themes: { auto: 'Comme l’application', light: 'Clair', sepia: 'Sépia', dark: 'Sombre' },
    previous: 'Précédent', next: 'Suivant', updated: 'Mis à jour {time}',
  },
}
