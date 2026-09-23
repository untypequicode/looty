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
        @dblclick="activeWorkspaceRole !== 'member' ? emit('open-rename', column.id, column.name) : null" 
        :title="activeWorkspaceRole !== 'member' ? 'Double-clic pour renommer' : ''"
      >
        {{ column.name }}
      </h3>
      <div style="display: flex; gap: 6px; align-items: center;">
        <span class="task-count">{{ column.tasks.length }}</span>
        <button 
          v-if="activeWorkspaceRole === 'owner' || activeWorkspaceRole === 'admin'" 
          class="icon-btn" 
          style="font-size: 0.75rem" 
          @click="emit('delete-column', column.id)" 
          title="Supprimer la colonne"
        >
          🗑️
        </button>
      </div>
    </div>
    
    <VueDraggableNext
      v-model="column.tasks"
      group="tasks"
      @change="emit('task-move', $event, column.id)"
      class="task-list"
      ghost-class="ghost-task"
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
    
    <button class="add-task-btn" @click="emit('open-task-modal', column.id)">+ Nouvelle tâche</button>
  </div>
</template>

<style scoped>
.kanban-column { background: transparent; border: none; border-radius: 0; width: 280px; min-width: 280px; display: flex; flex-direction: column; max-height: 100%; }
.column-header { padding: 0 4px 10px; display: flex; justify-content: space-between; align-items: center; }
.column-header h3 { margin: 0; font-size: 0.8rem; font-weight: 500; color: var(--text-color-3); text-transform: uppercase; letter-spacing: 0.3px; }
.task-count { color: var(--text-color-3); font-size: 0.7rem; padding: 0; }
.icon-btn { background: none; border: none; color: var(--text-color-3); cursor: pointer; padding: 2px 4px; border-radius: 4px; display: flex; align-items: center; justify-content: center; font-size: 0.85rem; transition: all 0.15s; }
.icon-btn:hover { background: var(--border-color); color: var(--text-color); }
.task-list { padding: 0; flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 2px; min-height: 50px; }
.ghost-task { opacity: 0.4; background: var(--border-color); border-radius: 6px; }
.add-task-btn { background: none; border: none; color: var(--text-color-3); cursor: pointer; padding: 8px 4px; text-align: left; font-size: 0.8rem; border-radius: 4px; transition: all 0.12s; }
.add-task-btn:hover { color: #eee; background: var(--bg-color-hover); }
</style>
