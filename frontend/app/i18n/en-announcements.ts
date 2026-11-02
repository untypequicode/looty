export default {
  errors: { announcement_too_long: 'This announcement is too long.' },
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
}
