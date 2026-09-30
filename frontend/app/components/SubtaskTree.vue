<template>
  <div class="nested-subtasks" style="margin-top: 8px; border-top: 1px solid var(--border-color); padding-top: 8px; margin-left: 8px; border-left: 1px solid var(--border-color); padding-left: 8px;">
    <div v-for="subtask in getSubtasks(parentTask)" :key="subtask.id">
      <div class="subtask-row" @click.stop="openTaskDetail(subtask)" style="display: flex; align-items: center; gap: 8px; font-size: 0.85rem; padding: 4px 0; color: var(--text-color-3); cursor: pointer;">
        <div class="subtask-checkbox" style="display:flex; align-items:center;" @click.stop="toggleTaskValidation(subtask)">
           <svg v-if="subtask.is_completed" class="status-icon completed" style="color:#10b981;" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" /></svg>
           <svg v-else-if="subtask.is_per_person && subtask.validations?.includes(currentUser?.id)" class="status-icon partially-completed" style="color:var(--primary-color);" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" /></svg>
           <div v-else style="width: 12px; height: 12px; border: 1.5px solid var(--text-color-3); border-radius: 3px;" :style="subtask.is_per_person ? 'border-style: dashed; border-color: var(--primary-color);' : ''"></div>
        </div>
        <span :style="{ textDecoration: subtask.is_completed ? 'line-through' : 'none' }">{{ subtask.title }}</span>
      </div>
      <!-- Recursion -->
      <SubtaskTree v-if="hasSubtasks(subtask)" :parentTask="subtask" :tasks="tasks" :openTaskDetail="openTaskDetail" :toggleTaskValidation="toggleTaskValidation" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { useAuth } from '../composables/useAuth'

const { user: currentUser } = useAuth()
const props = defineProps<{
  parentTask: any,
  tasks: any[],
  openTaskDetail: (task: any) => void,
  toggleTaskValidation: (task: any) => void
}>()

function getSubtasks(parent: any) {
  return props.tasks.filter((t: any) => t.parent_id === parent.id)
}

function hasSubtasks(parent: any) {
  return props.tasks.some((t: any) => t.parent_id === parent.id)
}
</script>
