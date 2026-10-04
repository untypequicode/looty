export default {
  attachments: { title: 'Attachments', drop: 'Drop files here or click to upload', deleteTitle: 'Delete this attachment?' },
  board: {
    addColumn: 'Add column', addTask: 'Add task', columnName: 'Column name', display: 'Display',
    automations: 'Automations', settings: 'Board settings', doneColumn: '“Done” column',
    doneColumnHint: 'Completed tasks are moved there automatically, and go back to their column when reopened.',
    noDoneColumn: 'None', autoValidate: 'Complete on move', autoValidateHint: 'Dropping a task in the “Done” column marks it as completed.',
    applySubtasks: 'Apply to sub-tasks', applySubtasksHint: 'Completed sub-tasks also follow the move rule.',
    markAsDone: '“Done” column', newLabel: 'New label…', noLabels: 'No labels on this board',
    emptyColumn: 'No tasks', quickAddHint: 'to create · Esc to close',
    notFound: 'Board not found', notFoundHint: 'It may have been deleted, or you don’t have access.',
    views: { board: 'Board', list: 'List', timeline: 'Timeline' },
  },
  comments: {
    activity: 'Activity', showHistory: 'History', placeholder: 'Leave a comment… (@ to mention)', send: 'Comment',
    edited: 'edited', deleteTitle: 'Delete this comment?',
  },
  filters: {
    clear: 'Clear all', searchTasks: 'Search tasks…', showCompleted: 'Show completed tasks', completed: 'Completed tasks', doneShow: 'Show', doneHide: 'Hide', doneMine: 'Hide mine',
    showSubtasks: 'Show sub-tasks as cards', showing: '{shown} of {total}', unassigned: 'Unassigned',
    due: { overdue: 'Overdue', today: 'Today', week: 'This week', none: 'No due date' },
  },
  history: {
    created: 'created the task', moved: 'moved from {from} to {to}', completed: 'completed the task', reopened: 'reopened the task',
    assigned: 'assigned {name}', unassigned: 'unassigned {name}', labelAdded: 'added label {name}', labelRemoved: 'removed label {name}',
    attached: 'attached {name}', renamed: 'renamed to “{title}”', description: 'updated the description',
    priority: 'set priority to {p}', due: 'set the due date to {date}', dueRemoved: 'removed the due date', updated: 'updated the task',
    restore: 'Restore this version', restoreTitle: 'Restore this version?', restoreMessage: 'The current value will be replaced (the change stays in the history).',
  },
  task: {
    addSubtask: 'Add sub-task', subtasks: 'Sub-tasks', subtaskPlaceholder: 'Sub-task title… (Enter)',
    assignees: 'Assignees', copyId: 'Copy ID', createdAgo: 'Created {time}', updatedAgo: 'Updated {time}', noDate: 'No date',
    pending: 'Pending', validated: 'Validated', perPerson: 'Per-person validation', perPersonHint: 'Each assignee validates on their own.',
    priority: 'Priority', reopen: 'Reopen', requireAll: 'Everyone must validate', startDate: 'Start', status: 'Status', unassigned: 'Unassigned',
  },
  timeline: {
    days: 'Days', weeks: 'Weeks', hint: 'Drag a bar to move it, its edges to resize.',
    unscheduled: 'Unscheduled', clickToSchedule: 'Click to schedule on this date',
  },
}
