export default {
  errors: {
    personal_not_author: 'Only the person who created it can make it personal.',
    personal_parent: 'It is filed in a personal item: it stays personal as long as that one is.',
    personal_item: 'Personal item: nobody else can be assigned.',
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
  personalItem: {
    badge: 'Personal', title: 'Personal: visible only to you, even to the workspace admins',
    create: 'Personal', createHint: 'Visible only to you (not even to admins).',
    newPersonalDoc: 'Personal document', newPersonalDocHint: 'Filed here, but visible only to you',
    makePersonal: 'Make personal', makeShared: 'Share with the workspace',
    makePersonalTitle: 'Make “{name}” personal?',
    makePersonalHint: {
      page: 'Other members will no longer see this document or its sub-documents; its individual shares are removed.',
      board: 'Other members will no longer see this board or its tasks; you stay the only assignee.',
      task: 'Other members will no longer see this task or its sub-tasks; you stay the only assignee.',
      project: 'Other members will no longer see this project.',
      file: 'Other members will no longer see this Drive item (nor its content, for a folder).',
    },
    nowPersonal: 'Now personal', nowShared: 'Now shared with the workspace',
    filter: 'Personal', filterHint: 'Only my personal items',
    moved: 'Your personal items from this workspace are in your personal space.',
  },
}
