<script setup lang="ts">
import { VueDraggableNext } from 'vue-draggable-next'
import type { Column, WorkspaceMember } from '../types'
import TaskCard from './TaskCard.vue'

const props = defineProps<{
  column: Column
  workspaceMembers: WorkspaceMember[]
  activeWorkspaceRole: string | null
}>()

const emit = defineEmits<{
  (e: 'task-move', event: any, toColumnId: string): void
  (e: 'open-task', task: any): void
  (e: 'toggle-validation', task: any): void
  (e: 'open-rename', id: string, name: string): void
  (e: 'delete-column', id: string): void
  (e: 'open-task-modal', columnId: string): void
}>()
</script>

<template>
  <div class="kanban-column">
    <div class="column-header">
      <h3 
        style="flex: 1; cursor: grab" 
        @dblclick="(activeWorkspaceRole !== 'viewer') ? emit('open-rename', column.id, column.name) : null" 
        :title="(activeWorkspaceRole !== 'viewer') ? 'Double-clic pour renommer' : ''"
      >
        {{ column.name }}
      </h3>
      <div style="display: flex; gap: 6px; align-items: center;">
        <span class="task-count">{{ column.tasks.length }}</span>
        <button 
          v-if="activeWorkspaceRole !== 'viewer'" 
          class="icon-btn" 
          style="font-size: 0.75rem" 
          @click="emit('delete-column', column.id)" 
          title="Supprimer la colonne"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/></svg>
        </button>
      </div>
    </div>
    
    <VueDraggableNext
      v-model="column.tasks"
      group="tasks"
      @change="emit('task-move', $event, column.id)"
      class="task-list"
      ghost-class="ghost-task"
      :disabled="activeWorkspaceRole === 'viewer'"
    >
      <TaskCard
        v-for="task in column.tasks" 
        :key="task.id"
        :task="task"
        :columnTasks="column.tasks"
        :workspaceMembers="workspaceMembers"
        @open-task="(t) => emit('open-task', t)"
        @toggle-validation="(t) => emit('toggle-validation', t)"
      />
    </VueDraggableNext>
    
    <button v-if="activeWorkspaceRole !== 'viewer'" class="add-task-btn" @click="emit('open-task-modal', column.id)">+ Nouvelle tâche</button>
  </div>
</template>

<style scoped>
.kanban-column { background: transparent; border: none; border-radius: 0; width: 300px; min-width: 300px; display: flex; flex-direction: column; max-height: 100%; }
.column-header { padding: 0 4px 10px; display: flex; justify-content: space-between; align-items: center; border-bottom: 2px solid transparent; margin-bottom: 8px; transition: border-color var(--transition-fast); }
.column-header h3 { margin: 0; font-size: 0.8125rem; font-weight: 500; color: var(--text-color); letter-spacing: 0px; }
.task-count { color: var(--text-color-3); font-size: 0.6875rem; padding: 2px 6px; background: var(--bg-color-2); border-radius: var(--radius-full); font-weight: 500; }
.icon-btn { background: none; border: none; color: var(--text-color-3); cursor: pointer; padding: 4px; border-radius: var(--radius-sm); display: flex; align-items: center; justify-content: center; font-size: 0.8125rem; transition: all var(--transition-fast); opacity: 0; }
.column-header:hover .icon-btn { opacity: 1; }
.icon-btn:hover { background: var(--bg-color-hover); color: var(--text-color); }
.task-list { padding: 2px; flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 8px; min-height: 50px; }
.ghost-task { opacity: 0.4; background: var(--bg-color-2); border-radius: var(--radius-md); border: 1px dashed var(--border-color); }
.add-task-btn { background: none; border: none; color: var(--text-color-3); cursor: pointer; padding: 6px 8px; text-align: left; font-size: 0.8125rem; border-radius: var(--radius-md); transition: all var(--transition-fast); margin-top: 4px; }
.add-task-btn:hover { color: var(--text-color); background: var(--bg-color-hover); }
</style>
