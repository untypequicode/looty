export default {
  errors: {
    invalid_provider: 'Unknown provider', api_key_required: 'An API key is required for this provider',
    base_url_required: 'The server address is required', invalid_base_url: 'Invalid server address (https:// and a public address, unless an admin allows private networks with AI_ALLOW_PRIVATE)',
    too_many_connections: 'Limit of 20 connections reached', no_connection: 'Choose an AI connection first',
    api_key_unreadable: 'The stored key is unreadable: enter it again', invalid_mode: 'Invalid write mode',
    study_grant_target: 'Choose where the assistant may create study sets, or which set it may edit', invalid_expiry: 'Invalid duration', invalid_message: 'Invalid message', invalid_context: 'Invalid context',
    too_many_tool_rounds: 'The assistant looped on its tools; try again',
  },
  ai: {
    title: 'AI assistant',
    settings: {
      title: 'AI connections',
      description: 'Connect your own AI accounts. Keys are stored encrypted on this server and are only used for your conversations.',
      add: 'Add a connection', edit: 'Edit connection', none: 'No connection yet.',
      privacy: 'The assistant can only read what you can access and never writes without your explicit permission, page by page.',
      provider: 'Provider', label: 'Name', labelPlaceholder: 'e.g. Work account', model: 'Model', baseUrl: 'Server address', optional: 'optional',
      apiKey: 'API key', keepKey: 'Leave empty to keep the current key',
      deleteTitle: 'Delete “{name}”?', deleteMessage: 'Conversations using it keep their history but need another connection.',
    },
    chat: {
      new: 'New conversation', untitled: 'Untitled', connection: 'Connection', context: 'Context', history: 'History',
      ctx: { page: 'This page', project: 'This project', workspace: 'This workspace', none: 'Global' },
      noConnection: 'Add an AI connection in the settings to start.', setUp: 'Open settings',
      readOnlyHint: 'Read-only: the assistant can read the pages you can access. To let it write, choose a page below.',
      placeholder: 'Ask something… (Enter to send)', send: 'Send', stop: 'Stop',
      writeTarget: 'Write target', readOnly: 'read-only', workspace: 'Workspace', page: 'Page', modeLabel: 'Mode', expiry: 'Duration', allow: 'Allow',
      studyGrants: 'Study sets', studyCreate: 'Create study sets in…', studyUpdate: 'Edit a study set', personal: 'Personal space', studySet: 'Study set', studyLink: 'Open the study set', studyGrantCreate: 'create sets in {where}', studyGrantUpdate: 'edit {title}',
      mode: { suggest: 'Suggest', append: 'Append', replace: 'Replace' },
      modeHint: {
        suggest: 'Suggest: the assistant proposes a change and you accept or reject it.',
        append: 'Append: the assistant may add content at the end of the page. A previous version is saved.',
        replace: 'Replace: the assistant may rewrite the whole page. A previous version is saved.',
      },
      suggestion: 'Proposed change', accept: 'Accept', reject: 'Reject', applied: 'Change applied (previous version saved in history)',
    },
  },
  nav: { aiAssistant: 'AI assistant' },
}
