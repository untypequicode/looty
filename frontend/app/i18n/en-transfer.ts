export default {
  transfer: {
    importTitle: 'Import documents', importHere: 'Import here…', import: 'Import',
    importHint: 'Exports from Notion (Markdown & CSV zip), Obsidian (zipped folder), Outline or Trello (board JSON), and Markdown or HTML files. Hierarchy, images and links are kept; what could not be imported is listed at the end.',
    dropTitle: 'Drop a file or click to browse', dropHint: '.zip from Notion, Obsidian, Outline or Looty, Trello .json, .md, .html — several files allowed',
    found: '{docs} document(s), {cols} collection(s) / folder(s)', foundTrello: '{lists} list(s), {cards} card(s)',
    importedBoard: 'Board created with {n} task(s)', reportTitle: '{count} item(s) could not be imported as they were',
    issue: {
      archivedCard: 'Archived card not imported: {name}', archivedList: 'Archived list not imported: {name}',
      member: 'No workspace member matches the Trello member “{name}”: their cards are not assigned',
      attachment: 'Attachment kept as a link, not copied: {name}', link: 'Link to a page that is not in the import: {name}',
      image: 'Image not found: {name}', file: 'File not found: {name}', csv: 'Table not imported: {name}',
    }, destination: 'Destination', atRoot: 'Workspace root', underPage: 'Under “{title}”',
    collectionPages: 'Create a page for each collection', folderPages: 'Create a page for each top-level folder',
    imported: '{n} page(s) imported', unsupported: 'Unsupported file type', nothingFound: 'No document found in this file', unreadable: 'This file could not be read',
    exportMenu: 'Export…', exportPage: 'Export document', exportWorkspace: 'Export workspace', export: 'Export',
    exportHint: 'Outline-compatible archive, with attachments. It can be imported back into Looty or Outline.',
    format: 'Format', jsonHint: 'Lossless; recommended to import into Outline or Looty.', markdownHint: 'Readable files, one folder per sub-document level.',
    scope: 'Content', thisPage: 'This page only', withSubpages: 'With {n} sub-page(s)', noSubpages: 'With sub-pages (none)',
    summary: '{n} page(s) will be exported along with their attachments.', exported: '{n} page(s) exported',
    sectionTitle: 'Import & export', sectionHint: 'Bring in your data from Notion, Obsidian, Trello or Outline, and move documents out.',
    importRowHint: 'From a Notion, Obsidian, Outline or Trello export, or Markdown files.', exportRowHint: 'Every document you can access, with attachments.',
  },
}
