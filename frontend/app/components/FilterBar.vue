<script setup lang="ts">
import type { WorkspaceMember, Label } from '../types'

const props = defineProps<{
  searchQuery: string
  filterAssignees: string[]
  filterLabels: string[]
  hideCompleted: boolean
  workspaceMembers: WorkspaceMember[]
  boardLabels: Label[]
}>()

const emit = defineEmits<{
  (e: 'update:searchQuery', value: string): void
  (e: 'update:hideCompleted', value: boolean): void
  (e: 'toggle-assignee', id: string): void
  (e: 'toggle-label', id: string): void
}>()

const getInitials = (name: string) => name ? name.charAt(0).toUpperCase() : '?'
</script>

<template>
  <div class="filter-bar">
    <input 
      :value="searchQuery" 
      @input="emit('update:searchQuery', ($event.target as HTMLInputElement).value)"
      type="text" 
      class="modern-input search-input" 
      placeholder="Rechercher une tâche..."
    >
    
    <div v-if="workspaceMembers?.length > 0" class="filter-section">
      <span class="filter-label">Membres :</span>
      <div class="filter-chips">
        <span v-for="m in workspaceMembers" :key="m.user_id"
              class="assignee-avatar small clickable-chip"
              :class="{ 'validated-avatar': filterAssignees.includes(m.user_id) }"
              :style="{ opacity: filterAssignees.includes(m.user_id) ? 1 : 0.5 }"
              @click="emit('toggle-assignee', m.user_id)"
              :title="m.name">
          {{ getInitials(m.name) }}
        </span>
      </div>
    </div>

    <div v-if="boardLabels?.length > 0" class="filter-section">
      <span class="filter-label">Labels :</span>
      <div class="filter-chips">
        <span v-for="l in boardLabels" :key="l.id"
              class="task-label-chip clickable-chip"
              :style="{ 
                backgroundColor: filterLabels.includes(l.id) ? l.color : '#2f3136', 
                border: filterLabels.includes(l.id) ? 'none' : '1px solid #4a4d55', 
                color: filterLabels.includes(l.id) ? '#fff' : '#9ca3af' 
              }"
              @click="emit('toggle-label', l.id)">
          {{ l.name }}
        </span>
      </div>
    </div>

    <label class="hide-completed-toggle">
      <input 
        type="checkbox" 
        :checked="hideCompleted"
        @change="emit('update:hideCompleted', ($event.target as HTMLInputElement).checked)"
      >
      Masquer terminées
    </label>
  </div>
</template>

<style scoped>
.filter-bar { display: flex; gap: 16px; padding: 12px 24px; background: var(--bg-color); border-bottom: 1px solid var(--border-color); align-items: center; flex-wrap: wrap; }
.search-input { max-width: 250px; margin: 0; padding: 6px 12px; font-size: 0.85rem; }
.filter-section { display: flex; gap: 8px; align-items: center; }
.filter-label { font-size: 0.8rem; color: var(--text-color-3); }
.filter-chips { display: flex; gap: 4px; flex-wrap: wrap; }
.hide-completed-toggle { display: flex; align-items: center; font-size: 0.85rem; color: var(--text-color-2); cursor: pointer; margin-left: auto; gap: 6px; }
.task-label-chip { padding: 2px 6px; border-radius: 12px; font-size: 0.7rem; color: #fff; font-weight: 500; text-shadow: 0 1px 2px rgba(0,0,0,0.5); }
.clickable-chip { cursor: pointer; transition: all 0.12s; user-select: none; }
.clickable-chip:hover { filter: brightness(1.15); transform: scale(1.03); }
.clickable-chip:active { transform: scale(0.97); }
.validated-avatar { box-shadow: 0 0 0 2px #10b981; opacity: 1; }
.assignee-avatar { width: 18px; height: 18px; border-radius: 50%; background: linear-gradient(135deg, #5e6ad2, #7c3aed); color: #fff; font-size: 0.6rem; display: flex; align-items: center; justify-content: center; font-weight: 600; margin-left: -4px; border: 1.5px solid var(--bg-color-main); }
.assignee-avatar.small { width: 22px; height: 22px; font-size: 0.7rem; margin: 0; }
</style>
