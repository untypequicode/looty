<script setup lang="ts">
import { VueDraggableNext } from 'vue-draggable-next'
import type { Board, WorkspaceMember } from '../types'
import KanbanColumn from './KanbanColumn.vue'

const props = defineProps<{
  board: Board | null
  workspaceMembers: WorkspaceMember[]
  activeWorkspaceRole: string | null
}>()

const emit = defineEmits<{
  (e: 'column-move', event: any): void
  (e: 'task-move', event: any, toColumnId: string): void
  (e: 'open-task', task: any): void
  (e: 'toggle-validation', task: any): void
  (e: 'open-rename', id: string, name: string): void
  (e: 'delete-column', id: string): void
  (e: 'open-task-modal', columnId: string): void
  (e: 'show-column-modal'): void
}>()
</script>

<template>
  <div class="kanban-board" v-if="board">
    <VueDraggableNext
      v-model="board.columns"
      group="columns"
      @change="emit('column-move', $event)"
      class="columns-container"
      ghost-class="ghost-column"
      handle=".column-header"
      :disabled="activeWorkspaceRole === 'viewer'"
    >
      <KanbanColumn
        v-for="col in board.columns" 
        :key="col.id" 
        :column="col"
        :workspaceMembers="workspaceMembers"
        :activeWorkspaceRole="activeWorkspaceRole"
        @task-move="(e, colId) => emit('task-move', e, colId)"
        @open-task="(t) => emit('open-task', t)"
        @toggle-validation="(t) => emit('toggle-validation', t)"
        @open-rename="(id, name) => emit('open-rename', id, name)"
        @delete-column="(id) => emit('delete-column', id)"
        @open-task-modal="(colId) => emit('open-task-modal', colId)"
      />
    </VueDraggableNext>
    
    <div 
      v-if="activeWorkspaceRole !== 'viewer'" 
      class="kanban-column add-column" 
      @click="emit('show-column-modal')"
    >
      + Ajouter une colonne
    </div>
  </div>
</template>

<style scoped>
.kanban-board { display: flex; gap: 1rem; align-items: flex-start; overflow-x: auto; flex: 1; padding: 1rem 2rem; }
.columns-container { display: flex; gap: 1rem; align-items: flex-start; height: 100%; min-height: 100px; }
.kanban-column { background: transparent; border: none; border-radius: 0; width: 280px; min-width: 280px; display: flex; flex-direction: column; max-height: 100%; }
.add-column { background: transparent; border: 1px dashed var(--border-color); color: var(--text-color-3); cursor: pointer; align-items: center; justify-content: center; padding: 1rem; font-size: 0.85rem; border-radius: 8px; }
.add-column:hover { border-color: var(--primary-color); color: var(--text-color); }
.ghost-column { opacity: 0.5; background: var(--border-color); border-radius: 6px; }
</style>
