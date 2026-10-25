export type Role = 'owner' | 'admin' | 'member' | 'viewer' | 'guest'

export interface User {
  id: string
  email: string
  name: string
  avatar_url?: string | null
  theme?: 'system' | 'light' | 'dark' | null
  locale?: 'fr' | 'en' | null
  is_super_admin?: boolean
  prefs?: Record<string, unknown>
}

export interface Me {
  authenticated: boolean
  user?: User
}

export interface Workspace {
  id: string
  name: string
  role: Role
  created_at?: string
  updated_at?: string
  calendar_color?: string | null
}

export interface WorkspaceMember {
  user_id: string
  name: string
  email: string
  avatar_url?: string | null
  role: Role
  joined_at?: string
  role_id?: string | null
  role_name?: string | null
  role_color?: string | null
  group_tasks?: boolean | null
}

export interface Label {
  id: string
  board_id: string
  name: string
  color: string
}

export interface BoardAutomation {
  on_complete_move_to?: string | null
  apply_to_subtasks?: boolean
  auto_validate_on_move?: boolean
}

export interface BoardSummary {
  id: string
  workspace_id: string
  name: string
  automations?: BoardAutomation
  created_at?: string
  updated_at?: string
}

export interface Board extends BoardSummary {
  columns: Column[]
  labels: Label[]
}

export interface Column {
  id: string
  board_id: string
  name: string
  position: number
  tasks: Task[]
}

export type Priority = 0 | 1 | 2 | 3 | 4

export interface Task {
  id: string
  column_id: string
  parent_id?: string | null
  title: string
  description?: string | null
  position: number
  priority: Priority
  start_date?: string | null
  due_date?: string | null
  /** Événement lié (créneau) et occurrence. */
  event_id?: string | null
  event_start?: string | null
  is_per_person: boolean
  require_all_validations: boolean
  is_completed: boolean
  labels: Label[]
  assignees: string[]
  validations: string[]
  previous_column_id?: string | null
  created_at?: string
  updated_at?: string
}

export interface Page {
  id: string
  workspace_id: string
  parent_id?: string | null
  icon?: string | null
  title: string
  content?: string | null
  created_by?: string
  updated_by?: string | null
  position: number
  created_at?: string
  updated_at?: string
  share_token?: string | null
  is_public?: boolean | null
  /** Projet porté par la page (LaTeX, site, notebook). */
  project_id?: string | null
  project_kind?: 'site' | 'latex' | 'notebook' | null
  /** Permissions particulières (membres ou invités). */
  has_permissions?: boolean
  is_favorite?: boolean
  view_count?: number | null
  last_viewed_at?: string | null
  total_view_count?: number | null
  can_edit?: boolean
  /** Sous-pages permises par un partage (invités, rôles sans création de documents). */
  can_add_child?: boolean
}

export interface TaskActivity {
  id: string
  task_id: string
  user_id: string
  activity_type: string
  old_value?: any
  new_value?: any
  created_at: string
}

export interface TaskAttachment {
  id: string
  task_id: string
  uploaded_by: string
  filename: string
  file_path: string
  mime_type?: string | null
  size_bytes?: number | null
  created_at: string
}

export interface TaskComment {
  id: string
  task_id: string
  user_id: string
  content: string
  created_at: string
  updated_at: string
}

export interface Notification {
  id: string
  workspace_id: string
  actor_id?: string | null
  actor_name?: string | null
  kind: 'mention' | 'assigned' | 'comment' | string
  target: string
  data: Record<string, any>
  read_at?: string | null
  created_at: string
}

export interface MyTask {
  id: string
  title: string
  due_date?: string | null
  priority: Priority
  is_completed: boolean
  board_id: string
  board_name: string
  column_name: string
  updated_at: string
  is_per_person: boolean
  assigned_to_me: boolean
  /** Tâche individuelle : ma part est faite, d'autres n'ont pas fini. */
  done_by_me: boolean
  assignees: string[]
  labels: { id: string; name: string; color: string }[]
}

export interface SearchResult {
  id: string
  title: string
  type: 'page' | 'task'
  icon?: string | null
  snippet?: string | null
  parent_id?: string | null
}

export interface Presence {
  user_id: string
  /** `page:<id>`, `board:<id>`, `task:<id>`, un écran (`home`, `calendar`…) ou `hidden`. */
  location: string
  updated_at: number
  status?: 'active' | 'idle'
  /** Depuis quand (ms) la personne est à cet endroit. */
  since?: number
  /** Membre que la personne suit (mode suivi). */
  following?: string | null
}
