export default {
  transfer: {
    importTitle: 'Importer des documents', importHere: 'Importer ici…', import: 'Importer',
    importHint: 'Exports de Notion (zip Markdown et CSV), d’Obsidian (dossier en zip), d’Outline ou de Trello (JSON du tableau), et fichiers Markdown ou HTML. L’arborescence, les images et les liens sont conservés ; ce qui n’a pas pu être repris est listé à la fin.',
    dropTitle: 'Déposez un fichier ou cliquez pour parcourir', dropHint: '.zip de Notion, d’Obsidian, d’Outline ou de Looty, .json de Trello, .md, .html — plusieurs fichiers possibles',
    found: '{docs} document(s), {cols} collection(s) / dossier(s)', foundTrello: '{lists} liste(s), {cards} carte(s)',
    importedBoard: 'Tableau créé avec {n} tâche(s)', reportTitle: '{count} élément(s) n’ont pas pu être repris tels quels',
    issue: {
      archivedCard: 'Carte archivée non importée : {name}', archivedList: 'Liste archivée non importée : {name}',
      member: 'Aucun membre de l’espace ne correspond au membre Trello « {name} » : ses cartes ne lui sont pas assignées',
      attachment: 'Pièce jointe gardée comme lien, non copiée : {name}', link: 'Lien vers une page absente de l’import : {name}',
      image: 'Image introuvable : {name}', file: 'Fichier introuvable : {name}', csv: 'Tableau non importé : {name}',
    }, destination: 'Destination', atRoot: 'Racine de l’espace', underPage: 'Sous « {title} »',
    collectionPages: 'Créer une page pour chaque collection', folderPages: 'Créer une page pour chaque dossier racine',
    imported: '{n} page(s) importée(s)', unsupported: 'Type de fichier non pris en charge', nothingFound: 'Aucun document trouvé dans ce fichier', unreadable: 'Impossible de lire ce fichier',
    exportMenu: 'Exporter…', exportPage: 'Exporter le document', exportWorkspace: 'Exporter l’espace de travail', export: 'Exporter',
    exportHint: 'Archive compatible Outline, avec les pièces jointes. Elle peut être réimportée dans Looty ou dans Outline.',
    format: 'Format', jsonHint: 'Sans perte ; recommandé pour importer dans Outline ou Looty.', markdownHint: 'Fichiers lisibles, un dossier par niveau de sous-documents.',
    scope: 'Contenu', thisPage: 'Cette page seulement', withSubpages: 'Avec {n} sous-page(s)', noSubpages: 'Avec les sous-pages (aucune)',
    summary: '{n} page(s) seront exportées avec leurs pièces jointes.', exported: '{n} page(s) exportée(s)',
    sectionTitle: 'Import et export', sectionHint: 'Reprenez vos données de Notion, Obsidian, Trello ou Outline, et sortez vos documents.',
    importRowHint: 'Depuis un export de Notion, d’Obsidian, d’Outline ou de Trello, ou des fichiers Markdown.', exportRowHint: 'Tous les documents auxquels vous avez accès, avec les pièces jointes.',
  },
}
