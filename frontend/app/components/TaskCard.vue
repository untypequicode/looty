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

const priorityIcons: Record<number, {label: string, color: string, filled: boolean}> = {
  0: { label: 'Sans priorité', color: '#6b7280', filled: false },
  1: { label: 'Urgente', color: '#ef4444', filled: true },
  2: { label: 'Haute', color: '#f97316', filled: true },
  3: { label: 'Moyenne', color: '#eab308', filled: true },
  4: { label: 'Basse', color: '#3b82f6', filled: true },
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
      <span class="priority-dot-icon" :style="{ borderColor: getPriority(task.priority).color, backgroundColor: getPriority(task.priority).filled ? getPriority(task.priority).color : 'transparent' }" :title="getPriority(task.priority).label">
      </span>
      <div class="task-labels" v-if="task.labels?.length > 0">
        <span v-for="lbl in task.labels" :key="lbl.id" class="task-label-dot" :style="{ backgroundColor: lbl.color + '40', color: lbl.color }" :title="lbl.name">
          {{ lbl.name }}
        </span>
      </div>
    </div>
    
    <h4>
      <svg v-if="task.is_completed" class="status-icon completed" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" /></svg>
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
.task-card { background: var(--bg-color-2); border: 1px solid var(--border-color); border-radius: var(--radius-md); padding: 10px 12px; cursor: pointer; transition: all var(--transition-fast); box-shadow: var(--shadow-sm); }
.task-card:hover { background: var(--bg-color-hover); border-color: var(--border-color-hover); transform: translateY(-1px); box-shadow: var(--shadow-md); }
.task-card:active { cursor: grabbing; transform: translateY(0); box-shadow: var(--shadow-sm); }
.task-card-top { display: flex; align-items: center; gap: 8px; margin-bottom: 6px; }
.priority-dot-icon { width: 10px; height: 10px; border-radius: 50%; border: 2px solid; flex-shrink: 0; display: inline-block; }
.task-label-dot { padding: 2px 6px; border-radius: 12px; font-size: 0.65rem; font-weight: 500; display: inline-flex; align-items: center; }
.task-card h4 { margin: 0; font-size: 0.875rem; font-weight: 500; color: var(--text-color); line-height: 1.4; display: flex; align-items: flex-start; gap: 8px; }
.status-icon { font-size: 0.8rem; flex-shrink: 0; margin-top: 2px; }
.status-icon.completed { color: var(--success-color); }
.task-title-done { text-decoration: line-through; color: var(--text-color-3); font-weight: 400; }
.task-card-footer { display: flex; justify-content: space-between; align-items: center; margin-top: 10px; }
.task-due-date { font-size: 0.75rem; color: var(--text-color-3); display: flex; align-items: center; gap: 4px; }
.task-due-date::before { content: ''; display: inline-block; width: 12px; height: 12px; background-image: url('data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="%2371717a" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>'); background-size: contain; background-repeat: no-repeat; }
.task-due-date.overdue::before { background-image: url('data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="%23ef4444" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>'); }
.task-due-date.overdue { color: var(--danger-color); }
.task-labels { display: flex; gap: 4px; flex-wrap: wrap; }
.task-assignees { display: flex; margin-left: 6px; }
.assignee-avatar { width: 20px; height: 20px; border-radius: 50%; background: var(--primary-color-glass); color: var(--primary-color); font-size: 0.65rem; display: flex; align-items: center; justify-content: center; font-weight: 600; margin-left: -6px; border: 2px solid var(--bg-color-2); transition: transform var(--transition-fast); }
.assignee-avatar:hover { transform: translateY(-2px) scale(1.1); z-index: 2; border-color: var(--border-color); }
.assignee-avatar.more { background: var(--bg-color-modal); color: var(--text-color-3); font-size: 0.6rem; }
.validated-avatar { box-shadow: 0 0 0 2px var(--success-color); border-color: var(--success-color); }
</style>
