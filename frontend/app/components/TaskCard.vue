<script setup lang="ts">
import type { Task, WorkspaceMember } from '../types'
import SubtaskTree from './SubtaskTree.vue'
import { useAuth } from '../composables/useAuth'

const { user } = useAuth()

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
      <div @click.stop="emit('toggle-validation', task)" style="cursor: pointer; display: flex; align-items: center; justify-content: center; width: 16px; height: 16px; margin-top: 2px;" title="Valider la tâche">
        <svg v-if="task.is_completed" class="status-icon completed" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" /></svg>
        <svg v-else-if="task.is_per_person && task.validations?.includes(user?.id)" class="status-icon partially-completed" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 20 20" fill="currentColor" style="color: var(--primary-color);"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" /></svg>
        <div v-else style="width: 12px; height: 12px; border: 1.5px solid var(--text-color-3); border-radius: 3px; display: inline-block;" :style="task.is_per_person ? 'border-style: dashed; border-color: var(--primary-color);' : ''"></div>
      </div>
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
.task-card { background: var(--bg-color-2); border: 1px solid var(--border-color); border-radius: var(--radius-md); padding: 10px 12px; cursor: pointer; transition: all var(--transition-fast); }
.task-card:hover { background: var(--bg-color-hover); border-color: var(--border-color-hover); }
.task-card:active { cursor: grabbing; }
.task-card-top { display: flex; align-items: center; gap: 6px; margin-bottom: 4px; }
.priority-dot-icon { width: 8px; height: 8px; border-radius: 50%; border: 1.5px solid; flex-shrink: 0; display: inline-block; }
.task-label-dot { padding: 1px 6px; border-radius: var(--radius-full); font-size: 0.625rem; font-weight: 500; display: inline-flex; align-items: center; }
.task-card h4 { margin: 0; font-size: 0.8125rem; font-weight: 500; color: var(--text-color); line-height: 1.4; display: flex; align-items: flex-start; gap: 6px; }
.status-icon { font-size: 0.75rem; flex-shrink: 0; margin-top: 2px; }
.status-icon.completed { color: var(--success-color); }
.task-title-done { text-decoration: line-through; color: var(--text-color-3); font-weight: 400; }
.task-card-footer { display: flex; justify-content: space-between; align-items: center; margin-top: 8px; }
.task-due-date { font-size: 0.6875rem; color: var(--text-color-3); display: flex; align-items: center; gap: 4px; }
.task-due-date::before { content: ''; display: inline-block; width: 12px; height: 12px; background-image: url('data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="%239b9b9b" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>'); background-size: contain; background-repeat: no-repeat; }
.task-due-date.overdue::before { background-image: url('data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="%23d9534f" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>'); }
.task-due-date.overdue { color: var(--danger-color); }
.task-labels { display: flex; gap: 4px; flex-wrap: wrap; }
.task-assignees { display: flex; margin-left: 4px; }
.assignee-avatar { width: 18px; height: 18px; border-radius: 50%; background: var(--primary-color-glass); color: var(--primary-color); font-size: 0.5625rem; display: flex; align-items: center; justify-content: center; font-weight: 600; margin-left: -4px; border: 2px solid var(--bg-color-2); transition: transform var(--transition-fast); }
.assignee-avatar:hover { transform: scale(1.15); z-index: 2; }
.assignee-avatar.more { background: var(--bg-color-3); color: var(--text-color-3); font-size: 0.5rem; }
.validated-avatar { box-shadow: 0 0 0 1.5px var(--success-color); border-color: var(--success-color); }
</style>
.task-card { background: var(--bg-color-2); border: 1px solid var(--border-color); border-radius: var(--radius-md); padding: 10px 12px; cursor: pointer; transition: all var(--transition-fast); box-shadow: var(--shadow-sm); }
