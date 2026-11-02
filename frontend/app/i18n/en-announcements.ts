export default {
  errors: {
    announcement_too_long: 'This announcement is too long.', poll_closed: 'This poll is closed.', poll_single_choice: 'Only one choice is allowed in this poll.',
    poll_invalid_option: 'Invalid poll option.', poll_option_count: 'A poll has 2 to 10 options.', poll_invalid_question: 'The poll question is empty or too long.',
  },
  announcements: {
    title: 'Announcements', new: 'New announcement', edit: 'Edit announcement', titleLabel: 'Title', titlePlaceholder: 'Room change, exam date…',
    bodyPlaceholder: 'Details of the announcement…', pinned: 'Pin to the top', pinnedHint: 'Stays at the top of the list and on the home page.',
    expires: 'Expires on', expiresHint: 'After that, only people who manage announcements still see it.', noExpiry: 'Never',
    publish: 'Publish', save: 'Save', delete: 'Delete announcement', deleteConfirm: 'Delete “{title}”? Members will no longer see it.',
    empty: 'No announcements yet.', emptyHint: 'Important news for the workspace will show up here.', emptyManage: 'Post news for every member: they get notified.',
    markAllRead: 'Mark all as read', unread: 'New', expired: 'Expired', pinnedBadge: 'Pinned', by: 'By {name}', edited: 'edited', published: 'Announcement published',
  },
  dashboard: {
    widgets: { announcements: 'Announcements' },
    widgetHints: { announcements: 'Pinned and recent announcements of the workspace' },
    empty: { announcements: 'No announcements.' },
    notif: { announcement: 'posted an announcement:' },
  },
  inbox: { kinds: { announcement: '{name} posted an announcement' } },
  settings: { notify: { ev: { announcement: 'Workspace announcements' }, evHint: { announcement: 'An announcement is posted in one of your workspaces.' } } },
  rolesUi: { flags: { announcements: 'Post announcements', announcementsHint: 'Post, edit and delete the workspace’s announcements (notified to every member).' } },
  polls: {
    add: 'Add a poll', addHint: 'Members vote right in the announcement.', question: 'Question', questionPlaceholder: 'Which evening for the class outing?',
    optionPlaceholder: 'Option {n}', addOption: 'Add an option', multiple: 'Allow several choices',
    anonymousLabel: 'Anonymous vote', anonymousHint: 'Only totals are shown, never who voted for what.',
    hideResults: 'Results after voting', hideResultsHint: 'Totals stay hidden from people who have not voted yet, until the poll closes.',
    closesLabel: 'Closes', locked: 'Votes were received: only the question and the closing date can still change.',
    singleHint: 'One choice', multipleHint: 'Several choices allowed', anonymous: 'Anonymous', closed: 'Closed', closesAt: 'Closes {date}',
    voters: { zero: 'No votes', one: '1 voter', other: '{count} voters' }, resultsAfterVote: 'results shown once you vote',
    vote: 'Vote', change: 'Change my vote', retract: 'Withdraw my vote', close: 'Close the poll', voted: 'Voted', toVote: 'Poll',
  },
  presence: { screens: { announcements: 'Announcements' } },
}
