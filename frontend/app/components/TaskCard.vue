<script setup lang="ts">
import type { Task, WorkspaceMember } from '../types'
import SubtaskTree from './SubtaskTree.vue'

const props = defineProps<{
  task: Task
  columnTasks: Task[]
  workspaceMembers: WorkspaceMember[]
}>()

const emit = defineEmits<{
  (e: 'open-task', task: Task): void
  (e: 'toggle-validation', task: Task): void
}>()

const priorityIcons: Record<number, {label: string, color: string, icon: string}> = {
  0: { label: 'Sans priorité', color: '#6b7280', icon: '○' },
  1: { label: 'Urgente', color: '#ef4444', icon: '🔴' },
  2: { label: 'Haute', color: '#f97316', icon: '🟠' },
  3: { label: 'Moyenne', color: '#eab308', icon: '🟡' },
  4: { label: 'Basse', color: '#6b7280', icon: '🔵' },
}

const getPriority = (p: number) => priorityIcons[p] || priorityIcons[0]

const getMemberInitials = (userId: string) => {
  const member = props.workspaceMembers.find(m => m.user_id === userId)
  return member ? member.name.charAt(0).toUpperCase() : '?'
}

const getMemberName = (userId: string) => {
  const member = props.workspaceMembers.find(m => m.user_id === userId)
  return member ? member.name : 'Inconnu'
}
</script>

<template>
  <div class="task-card" @click="emit('open-task', task)">
    <div class="task-card-top">
      <span class="priority-dot" :style="{ color: getPriority(task.priority).color }" :title="getPriority(task.priority).label">
        {{ getPriority(task.priority).icon }}
      </span>
      <div class="task-labels" v-if="task.labels?.length > 0">
        <span v-for="lbl in task.labels" :key="lbl.id" class="task-label-dot" :style="{ backgroundColor: lbl.color }" :title="lbl.name"></span>
      </div>
    </div>
    
    <h4>
      <span v-if="task.is_completed" class="status-icon completed">✓</span>
      <span :class="{ 'task-title-done': task.is_completed }">{{ task.title }}</span>
    </h4>
    
    <div class="task-card-footer">
      <div v-if="task.due_date" class="task-due-date" :class="{ 'overdue': new Date(task.due_date) < new Date() }">
        {{ new Date(task.due_date).toLocaleDateString('fr-FR', { day: 'numeric', month: 'short' }) }}
      </div>
      <div v-else></div>
      
      <div class="task-assignees" v-if="task.assignees?.length > 0">
        <span v-for="userId in task.assignees.slice(0, 3)" :key="userId" 
              class="assignee-avatar" 
              :class="{'validated-avatar': task.is_per_person && task.validations?.includes(userId)}" 
              :title="getMemberName(userId)">
          {{ getMemberInitials(userId) }}
        </span>
        <span v-if="task.assignees.length > 3" class="assignee-avatar more">
          +{{ task.assignees.length - 3 }}
        </span>
      </div>
    </div>
    
    <SubtaskTree 
      v-if="columnTasks.some(t => t.parent_id === task.id)" 
      :parentTask="task" 
      :tasks="columnTasks" 
      :openTaskDetail="(t) => emit('open-task', t)" 
      :toggleTaskValidation="(t) => emit('toggle-validation', t)" 
    />
  </div>
</template>

<style scoped>
.task-card { background: transparent; border: none; border-radius: 6px; padding: 6px 8px; cursor: pointer; transition: background 0.1s; }
.task-card:hover { background: var(--bg-color-hover); }
.task-card:active { cursor: grabbing; }
.task-card-top { display: flex; align-items: center; gap: 6px; margin-bottom: 2px; }
.priority-dot { font-size: 0.65rem; line-height: 1; }
.task-label-dot { width: 8px; height: 8px; border-radius: 50%; display: inline-block; }
.task-card h4 { margin: 0; font-size: 0.85rem; font-weight: 400; color: #e4e4e7; line-height: 1.4; display: flex; align-items: center; gap: 6px; }
.status-icon { font-size: 0.75rem; flex-shrink: 0; }
.status-icon.completed { color: #10b981; }
.task-title-done { text-decoration: line-through; color: #6b7280; }
.task-card-footer { display: flex; justify-content: space-between; align-items: center; margin-top: 4px; }
.task-due-date { font-size: 0.7rem; color: var(--text-color-3); }
.task-due-date.overdue { color: #f87171; }
.task-labels { display: flex; gap: 3px; }
.task-assignees { display: flex; margin-left: 4px; }
.assignee-avatar { width: 18px; height: 18px; border-radius: 50%; background: linear-gradient(135deg, #5e6ad2, #7c3aed); color: #fff; font-size: 0.6rem; display: flex; align-items: center; justify-content: center; font-weight: 600; margin-left: -4px; border: 1.5px solid var(--bg-color-main); }
.assignee-avatar.more { background: var(--bg-color-modal); color: var(--text-color-3); font-size: 0.55rem; }
.validated-avatar { box-shadow: 0 0 0 2px #10b981; opacity: 1; }
</style>
