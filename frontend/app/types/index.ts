export interface User {
  id: string
  email: string
  name: string
  is_super_admin?: boolean
}

export interface WorkspaceMember {
  user_id: string
  name: string
  email: string
  role: 'owner' | 'admin' | 'member'
  joined_at?: string
}

export interface Workspace {
  id: string
  name: string
  role: 'owner' | 'admin' | 'member'
  created_at?: string
  updated_at?: string
}

export interface Label {
  id: string
  board_id: string
  name: string
  color: string
}

export interface BoardAutomation {
  on_complete_move_to?: string
  apply_to_subtasks?: boolean
  auto_validate_on_move?: boolean
}

export interface Board {
  id: string
  workspace_id: string
  name: string
  columns: Column[]
  labels?: Label[]
  automations?: BoardAutomation
  created_at?: string
  updated_at?: string
}

export interface Column {
  id: string
  board_id: string
  name: string
  position: number
  tasks: Task[]
  created_at?: string
  updated_at?: string
}

export interface Task {
  id: string
  column_id: string
  parent_id?: string | null
  title: string
  description?: string
  position: number
  priority: number
  start_date?: string | null
  due_date?: string | null
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
  content?: string
  created_by?: string
  created_at?: string
  updated_at?: string
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
  mime_type?: string
  size_bytes?: number
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
