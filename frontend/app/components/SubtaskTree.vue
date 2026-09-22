<template>
  <div class="nested-subtasks" style="margin-top: 8px; border-top: 1px solid #3f3f46; padding-top: 8px; margin-left: 8px; border-left: 1px solid #3f3f46; padding-left: 8px;">
    <div v-for="subtask in getSubtasks(parentTask)" :key="subtask.id">
      <div class="subtask-row" @click.stop="openTaskDetail(subtask)" style="display: flex; align-items: center; gap: 8px; font-size: 0.85rem; padding: 4px 0; color: #a1a1aa; cursor: pointer;">
        <div class="subtask-checkbox" :style="{ color: subtask.is_completed ? '#10b981' : '#52525b' }" @click.stop="toggleTaskValidation(subtask)">
           {{ subtask.is_completed ? '☑' : '☐' }}
        </div>
        <span :style="{ textDecoration: subtask.is_completed ? 'line-through' : 'none' }">{{ subtask.title }}</span>
      </div>
      <!-- Recursion -->
      <SubtaskTree v-if="hasSubtasks(subtask)" :parentTask="subtask" :tasks="tasks" :openTaskDetail="openTaskDetail" :toggleTaskValidation="toggleTaskValidation" />
    </div>
  </div>
</template>

<script setup lang="ts">
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
