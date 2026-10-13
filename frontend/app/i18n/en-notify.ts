export default {
  errors: {
    smtp_not_configured: 'Email sending is not configured', smtp_incomplete: 'Server and sender address are required',
    smtp_invalid_from: 'Invalid sender address', smtp_invalid_recipient: 'Invalid recipient address',
    smtp_send_failed: 'The SMTP server refused the email', smtp_tls: 'TLS error with the SMTP server', smtp_message: 'Could not build the email',
    push_invalid_endpoint: 'This browser’s push service is not allowed', push_invalid_keys: 'Invalid push subscription', push_too_many: 'Too many devices (20 max)',
    invalid_event: 'Unknown notification type', invalid_value: 'Invalid value',
  },
  inbox: { kinds: { due_soon: 'Due soon: {title}', completed: '{name} completed {title}' } },
  settings: {
    nav: { notifications: 'Notifications', email: 'Email' },
    notify: {
      channels: 'Channels', channelsHint: 'Notifications always appear in your inbox. Choose where else you want to receive them.',
      emailLabel: 'Email notifications', emailHint: 'Sent to {email}', emailUnavailable: 'Email sending isn’t configured on this instance yet.',
      pushLabel: 'Push notifications', pushHint: 'On your computer or phone, even when Looty is closed.',
      pushIos: 'On iPhone and iPad, notifications need the app on your home screen: in Safari tap Share → “Add to Home Screen”, then open Looty from there and come back here (iOS 16.4 or later).', pushUnsupported: 'Not available in this browser (a secure https connection is required).', pushDenied: 'Blocked in this browser’s site settings.',
      pushOnHere: 'On for this device', enableHere: 'Enable on this device', pushEnabled: 'Push notifications enabled on this device',
      enableDesktop: 'Notify while Looty is open', desktopOn: 'While Looty is open',
      events: 'What you receive', eventsHint: 'Pick a channel for each kind of event.', email: 'Email', push: 'Push',
      ev: {
        task_assigned: 'Assigned to a task', task_mention: 'Mentioned in a task', doc_mention: 'Mentioned in a document',
        task_comment: 'Comments on my tasks', task_due: 'Due date reminders', task_completed: 'My tasks completed by someone else',
      },
      evHint: {
        task_assigned: 'Someone assigns you to a task.', task_mention: 'Someone @mentions you in a task comment.', doc_mention: 'Someone @mentions you in a wiki page.',
        task_comment: 'New comments on tasks assigned to you.', task_due: 'Tasks assigned to you due in the next 24 hours.', task_completed: 'Someone completes a task assigned to you.',
      },
      noDeviceNote: 'Push is only sent to devices where you enabled it.',
      emailDelayNote: 'Emails are sent a couple of minutes later, and skipped if you have already read the notification in Looty.',
      devices: 'Devices', devicesHint: 'Browsers that receive your push notifications.', noDevices: 'No device yet.',
      thisDevice: 'This device', test: 'Send a test', testSent: 'Test sent to {n} device(s)', testNone: 'No device could be reached',
    },
    mail: {
      title: 'Email (SMTP)', description: 'Server used to send notifications by email. Users choose what they receive in their settings.',
      test: 'Send a test email', testSent: 'Test email sent to {email}',
      envActive: 'Email settings currently come from the server environment (SMTP_* variables). Enable this form to override them.',
      enabled: 'Send emails', enabledHint: 'Use the settings below.', host: 'SMTP server', security: 'Security',
      securityHint: 'STARTTLS on port 587, TLS on port 465.', none: 'None', port: 'Port', username: 'Username', password: 'Password',
      passwordKept: 'Leave empty to keep the current password.', from: 'Sender address', fromHint: 'Must be allowed by your SMTP server.',
      fromName: 'Sender name', delay: 'Email delay (minutes)', delayHint: 'Lets people read the notification in the app first; 0 sends right away.',
    },
  },
}
