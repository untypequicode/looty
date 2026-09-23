<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import type { Task, WorkspaceMember, Label } from '../types'
import TaskComments from './TaskComments.vue'
import TaskAttachments from './TaskAttachments.vue'
import TaskActivities from './TaskActivities.vue'
import SimpleEditor from './SimpleEditor.vue'

const props = defineProps<{
  editingTask: Task | null
  workspaceMembers: WorkspaceMember[]
  filteredLabels: Label[]
  filteredAssignees: WorkspaceMember[]
  targetColumnId: string | null
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'save', taskData: any): void
  (e: 'delete'): void
  (e: 'create-label', name: string, color: string, id?: string): void
  (e: 'delete-label', id: string): void
  (e: 'toggle-validation', task: Task): void
  (e: 'create-subtask', parentId: string, title: string): void
  (e: 'open-task', task: Task): void
  (e: 'toggle-task-label', task: Task | null, label: Label): void
  (e: 'toggle-task-assignee', task: Task | null, userId: string): void
}>()

const editTaskTitle = ref('')
const editTaskDesc = ref('')
const editTaskStartDate = ref('')
const editTaskDueDate = ref('')
const editTaskPriority = ref(0)
const editTaskIsPerPerson = ref(false)

const labelSearch = ref('')
const assigneeSearch = ref('')
const showCreateLabel = ref(false)
const newLabelName = ref('')
const newLabelColor = ref('#5e6ad2')
const editingLabelId = ref<string | null>(null)
const newSubtaskTitle = ref('')

const createTaskLabels = ref<Label[]>([])
const createTaskAssignees = ref<string[]>([])

const isLabelSelected = (labelId: string) => {
  if (props.editingTask) {
    return props.editingTask.labels?.some(l => l.id === labelId)
  }
  return createTaskLabels.value.some(l => l.id === labelId)
}

const isAssigneeSelected = (userId: string) => {
  if (props.editingTask) {
    return props.editingTask.assignees?.includes(userId)
  }
  return createTaskAssignees.value.includes(userId)
}

const toggleLabel = (lbl: Label) => {
  emit('toggle-task-label', props.editingTask, lbl)
  if (!props.editingTask) {
    if (isLabelSelected(lbl.id)) {
      createTaskLabels.value = createTaskLabels.value.filter(l => l.id !== lbl.id)
    } else {
      createTaskLabels.value.push(lbl)
    }
  }
}

const toggleAssignee = (userId: string) => {
  emit('toggle-task-assignee', props.editingTask, userId)
  if (!props.editingTask) {
    if (isAssigneeSelected(userId)) {
      createTaskAssignees.value = createTaskAssignees.value.filter(u => u !== userId)
    } else {
      createTaskAssignees.value.push(userId)
    }
  }
}

const startEditLabel = (lbl: Label) => {
  editingLabelId.value = lbl.id
  newLabelName.value = lbl.name
  newLabelColor.value = lbl.color
  showCreateLabel.value = true
}

const toggleCreateLabel = () => {
  if (showCreateLabel.value && !editingLabelId.value) {
    showCreateLabel.value = false
  } else {
    editingLabelId.value = null
    newLabelName.value = ''
    newLabelColor.value = '#5e6ad2'
    showCreateLabel.value = true
  }
}

const submitTask = () => {
  emit('save', {
    title: editTaskTitle.value,
    description: editTaskDesc.value,
    priority: editTaskPriority.value,
    start_date: editTaskStartDate.value ? new Date(editTaskStartDate.value).toISOString() : null,
    due_date: editTaskDueDate.value ? new Date(editTaskDueDate.value).toISOString() : null,
    is_per_person: editTaskIsPerPerson.value,
    labels: props.editingTask ? undefined : createTaskLabels.value.map(l => l.id),
    assignees: props.editingTask ? undefined : createTaskAssignees.value
  })
}

const saveTaskSettings = () => {
  if (!props.editingTask) return
  emit('save', {
    title: props.editingTask.title,
    description: props.editingTask.description,
    is_per_person: editTaskIsPerPerson.value,
    start_date: props.editingTask.start_date,
    due_date: props.editingTask.due_date
  })
}

const getMemberName = (userId: string) => {
  const m = props.workspaceMembers.find(x => x.user_id === userId)
  return m ? m.name : 'Inconnu'
}

const getMemberInitials = (userId: string) => getMemberName(userId).charAt(0).toUpperCase()

const getSubtasks = (parentTask: Task) => {
  // We need to pass columnTasks or pass subtasks as prop
  // In the original it iterated through activeBoard.columns
  // We should pass subtasks as a prop computed in index.vue for purity
  return [] // Placeholder for now, we will pass them as props
}

// Watchers to sync editingTask with local refs
watch(() => props.editingTask, (task) => {
  if (task) {
    editTaskTitle.value = task.title
    editTaskDesc.value = task.description || ''
    editTaskStartDate.value = task.start_date ? task.start_date.split('T')[0] : ''
    editTaskDueDate.value = task.due_date ? task.due_date.split('T')[0] : ''
    editTaskPriority.value = task.priority || 0
    editTaskIsPerPerson.value = task.is_per_person || false
  } else {
    editTaskTitle.value = ''
    editTaskDesc.value = ''
    editTaskStartDate.value = ''
    editTaskDueDate.value = ''
    editTaskPriority.value = 0
    editTaskIsPerPerson.value = false
    createTaskLabels.value = []
    createTaskAssignees.value = []
  }
}, { immediate: true })

</script>

<template>
  <div class="slideover-overlay" @click.self="emit('close')">
    <div class="slideover-panel">
      <form @submit.prevent="submitTask" style="display: flex; flex-direction: column; height: 100%;">
        <div class="slideover-header">
          <input v-focus v-model="editTaskTitle" type="text" placeholder="Titre de la tâche" class="modern-input title-input" required>
          <button type="button" class="close-btn" @click="emit('close')">✕</button>
        </div>
        
        <div class="slideover-body">
          <div class="slideover-main">
            <!-- Meta horizontale -->
            <div class="slideover-meta-horizontal">
              <div class="meta-field">
                <span class="meta-label">Priorité</span>
                <select v-model="editTaskPriority" class="modern-select meta-select">
                  <option :value="0">Sans priorité</option>
                  <option :value="1">🔴 Urgente</option>
                  <option :value="2">🟠 Haute</option>
                  <option :value="3">🟡 Moyenne</option>
                  <option :value="4">🔵 Basse</option>
                </select>
              </div>
              <div class="meta-field">
                <span class="meta-label">Début</span>
                <input v-model="editTaskStartDate" type="date" class="modern-input meta-input">
              </div>
              <div class="meta-field">
                <span class="meta-label">Échéance</span>
                <input v-model="editTaskDueDate" type="date" class="modern-input meta-input">
              </div>
            </div>
            
            <div class="slideover-meta-horizontal" style="margin-top: 1rem;">
              <div class="meta-section" style="flex: 1; min-width: 200px;">
                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;">
                  <label class="section-label" style="margin: 0;">Labels</label>
                  <button type="button" @click="toggleCreateLabel" style="background:none; border:none; color:#5e6ad2; cursor:pointer; font-size: 0.75rem;">+ Créer</button>
                </div>
                
                <div v-if="showCreateLabel" style="margin-bottom: 8px; display: flex; flex-direction: column; gap: 8px; background: #222327; padding: 8px; border-radius: 4px;">
                  <input v-model="newLabelName" type="text" class="modern-input" placeholder="Nom du label" style="margin-bottom: 0; padding: 6px;">
                  <div style="display: flex; gap: 8px; align-items: center;">
                    <input v-model="newLabelColor" type="color" style="height: 28px; width: 40px; border:none; background:none; cursor:pointer;">
                    <button type="button" class="primary-btn small-btn" @click="emit('create-label', newLabelName, newLabelColor, editingLabelId || undefined)" :disabled="!newLabelName" style="flex: 1;">{{ editingLabelId ? 'Sauver' : 'Ajouter' }}</button>
                    <button type="button" class="secondary-btn small-btn" @click="showCreateLabel = false">✕</button>
                  </div>
                </div>

                <!-- Input search for labels would go here... -->
                <div style="display: flex; gap: 4px; flex-wrap: wrap;">
                  <span v-for="lbl in filteredLabels" :key="lbl.id" 
                    class="task-label-chip clickable-chip" 
                    :class="{ 'selected-chip': isLabelSelected(lbl.id) }"
                    :style="{ backgroundColor: isLabelSelected(lbl.id) ? lbl.color : '#2f3136', border: isLabelSelected(lbl.id) ? 'none' : '1px solid #4a4d55', color: isLabelSelected(lbl.id) ? '#fff' : '#9ca3af' }"
                    @click="toggleLabel(lbl)">
                    {{ lbl.name }}
                    <button type="button" @click.stop="startEditLabel(lbl)" title="Modifier ce label" style="background:none; border:none; color:inherit; cursor:pointer; margin-left:4px; font-size:10px; opacity:0.7">✏️</button>
                    <button type="button" @click.stop="emit('delete-label', lbl.id)" title="Supprimer définitivement" style="background:none; border:none; color:inherit; cursor:pointer; font-size:10px; opacity:0.7">🗑</button>
                  </span>
                </div>
              </div>

              <div class="meta-section" style="flex: 1; min-width: 200px;">
                <label class="section-label">Membres assignés</label>
                <!-- Input search for assignees would go here... -->
                <div style="display: flex; gap: 6px; flex-wrap: wrap;">
                  <span v-for="m in filteredAssignees" :key="m.user_id" 
                    class="assignee-chip clickable-chip"
                    :class="{ 'selected-chip': isAssigneeSelected(m.user_id) }"
                    :style="{ backgroundColor: isAssigneeSelected(m.user_id) ? '#5e6ad2' : '#2f3136', border: isAssigneeSelected(m.user_id) ? 'none' : '1px solid #4a4d55', color: isAssigneeSelected(m.user_id) ? '#fff' : '#9ca3af' }"
                    @click="toggleAssignee(m.user_id)">
                    {{ m.name }}
                  </span>
                </div>
              </div>
            </div>

            <label class="section-label">Description</label>
            <SimpleEditor v-model="editTaskDesc" />
            
            <template v-if="editingTask">
              <div style="margin-top: 2rem;">
                <TaskComments :task-id="editingTask.id" :workspace-members="workspaceMembers" />
              </div>

              <div style="margin-top: 2rem;">
                <TaskAttachments :task-id="editingTask.id" />
              </div>

              <div style="margin-top: 2rem;">
                <TaskActivities :task-id="editingTask.id" :workspace-members="workspaceMembers" />
              </div>
              
              <!-- Subtasks and validations section omitted for brevity, to be added later if needed -->
            </template>
          </div>
        </div>
        
        <div class="slideover-footer">
          <button type="button" v-if="editingTask" class="secondary-btn" style="color: #f87171" @click="emit('delete')">Supprimer la tâche</button>
          <div style="flex: 1"></div>
          <button type="button" class="secondary-btn" @click="emit('close')">Annuler</button>
          <button type="submit" class="primary-btn">{{ editingTask ? 'Enregistrer' : 'Créer' }}</button>
        </div>
      </form>
    </div>
  </div>
</template>

<style scoped>
/* styles adapted from index.vue */
.slideover-overlay { position: fixed; top: 0; left: 0; width: 100vw; height: 100vh; background: rgba(0, 0, 0, 0.4); z-index: 100; display: flex; justify-content: flex-end; backdrop-filter: blur(2px); }
.slideover-panel { width: 720px; max-width: 100%; height: 100vh; background: var(--bg-color-2); border-left: 1px solid var(--border-color); box-shadow: -8px 0 40px rgba(0,0,0,0.5); display: flex; flex-direction: column; }
.slideover-header { padding: 1.25rem 1.5rem; border-bottom: 1px solid var(--border-color); display: flex; justify-content: space-between; align-items: center; }
.title-input { font-size: 1.1rem; font-weight: 600; padding: 0.4rem; background: transparent; border: 1px solid transparent; width: 100%; margin-right: 1rem; color: #fff; border-radius: 4px; }
.title-input:focus { background: var(--bg-color-modal); border-color: var(--primary-color); }
.close-btn { background: none; border: none; color: var(--text-color-3); font-size: 1rem; cursor: pointer; padding: 0.4rem; border-radius: 4px; transition: 0.15s; }
.close-btn:hover { background: var(--border-color); color: #eee; }
.slideover-body { display: flex; flex: 1; flex-direction: column; overflow: hidden; }
.slideover-main { flex: 1; padding: 1.5rem; overflow-y: auto; }
.slideover-meta-horizontal { display: flex; gap: 1rem; flex-wrap: wrap; margin-bottom: 1.25rem; padding: 12px; background: var(--bg-color-modal); border-radius: 8px; border: 1px solid var(--border-color); }
.meta-field { display: flex; align-items: center; gap: 8px; }
.meta-label { font-size: 0.75rem; color: var(--text-color-3); font-weight: 500; white-space: nowrap; }
.modern-input { flex: 1; padding: 6px 10px; border-radius: 6px; background: #111; border: 1px solid var(--border-color); color: #eee; font-family: inherit; font-size: 0.85rem; }
.meta-input { width: 130px; margin: 0; padding: 4px 8px; font-size: 0.8rem; }
.modern-select { background: #111; color: #eee; border: 1px solid var(--border-color); padding: 4px 8px; border-radius: 6px; outline: none; cursor: pointer; font-family: inherit; font-size: 0.85rem; }
.meta-select { margin: 0; padding: 4px 8px; font-size: 0.8rem; min-width: 120px; }
.section-label { display: block; font-size: 0.7rem; font-weight: 500; color: var(--text-color-3); text-transform: uppercase; letter-spacing: 0.5px; margin-bottom: 0.5rem; }
.slideover-footer { padding: 1rem 1.5rem; border-top: 1px solid var(--border-color); display: flex; justify-content: space-between; align-items: center; gap: 1rem; background: var(--bg-color-2); }
.primary-btn { padding: 6px 14px; border-radius: 6px; border: none; cursor: pointer; background: var(--primary-color); color: #fff; font-weight: 500; font-size: 0.85rem; transition: all 0.12s; }
.primary-btn:hover { background: var(--primary-color-hover); }
.secondary-btn { padding: 6px 14px; border-radius: 6px; border: 1px solid var(--border-color); cursor: pointer; background: transparent; color: var(--text-color-3); font-weight: 500; font-size: 0.85rem; transition: all 0.12s; }
.secondary-btn:hover { background: var(--border-color); color: #eee; }
.task-label-chip { padding: 2px 6px; border-radius: 12px; font-size: 0.7rem; color: #fff; font-weight: 500; text-shadow: 0 1px 2px rgba(0,0,0,0.5); }
.clickable-chip { cursor: pointer; transition: all 0.12s; user-select: none; }
.selected-chip { box-shadow: 0 0 0 2px rgba(94, 106, 210, 0.4); }
.assignee-chip { display: inline-flex; align-items: center; background: var(--border-color); padding: 4px 8px; border-radius: 4px; font-size: 0.8rem; color: #eee; }
</style>
