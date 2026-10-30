export default {
  errors: {
    personal_space: 'A personal space stays private: you can’t invite anyone, leave it or delete it.',
    move_restricted_subpages: 'You can’t edit some of its sub-pages, so this page can’t be moved with them.',
  },
  move: {
    action: 'Move to another workspace…', title: 'Move “{name}”', hint: 'The page moves with all its sub-pages, its project, canvas, comments and history.',
    to: 'To', where: 'Location', root: 'At the root of the documents', searchPage: 'Search a page', submit: 'Move',
    note: 'Individual shares are removed, and links to items of the old workspace (boards, events) don’t follow.',
    noSpace: 'No other workspace where you can create documents.', open: 'Open',
    done: { one: 'Page moved to {name}', other: '{count} pages moved to {name}' }, dropHere: 'Drop to move here',
  },
  personal: {
    title: 'Personal space', new: 'Create in my personal space', all: 'See all', createFirst: 'Create a personal document',
    open: 'My personal space', private: 'Private', hint: 'Only visible to you, from all your workspaces: documents, boards, projects, calendar, mail, drive.',
    start: 'Start in my personal space',
  },
}
