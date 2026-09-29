<template>
  <div class="activities-section" style="margin-top: 2rem;">
    <h3 style="margin: 0 0 12px 0; font-size: 1rem; font-weight: 500;">Historique d'activité</h3>
    
    <div class="activities-list">
      <div v-for="activity in activities" :key="activity.id" class="activity-item">
        <div class="activity-header">
          <div class="activity-avatar">{{ getUserInitial(activity.user_id) }}</div>
          <div class="activity-info">
            <span class="activity-user">{{ getUserName(activity.user_id) }}</span>
            <span class="activity-text">{{ getActivityText(activity) }}</span>
          </div>
          <div class="activity-time">{{ formatDate(activity.created_at) }}</div>
        </div>
        
        <div v-if="activity.activity_type === 'update_description' || activity.activity_type === 'update_title'" class="activity-actions">
          <button class="secondary-btn small-btn" @click="restoreActivity(activity.id)">Restaurer cette version</button>
        </div>
      </div>
      <div v-if="!activities || activities.length === 0" class="empty-state">
        Aucune activité récente.
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, watch } from 'vue'

const props = defineProps({
  taskId: {
    type: String,
    required: true
  },
  workspaceMembers: {
    type: Array,
    default: () => []
  },
  task: {
    type: Object,
    default: () => ({})
  }
})

const activities = ref([])
const confirmAction = (msg) => window.confirm(msg)

async function loadActivities() {
  try {
    const res = await $fetch(`/api/boards/tasks/${props.taskId}/activities`)
    activities.value = Array.isArray(res) ? res : []
  } catch (e) {
    console.error(e)
    activities.value = []
  }
}

onMounted(() => {
  loadActivities()
})

watch(() => props.taskId, loadActivities)

let timeout = null
watch(() => props.task, () => {
  if (timeout) clearTimeout(timeout)
  timeout = setTimeout(loadActivities, 500)
}, { deep: true })

async function restoreActivity(activityId) {
  if (!confirmAction("Voulez-vous vraiment restaurer la tâche à cet état ?")) return
  try {
    await $fetch(`/api/boards/tasks/${props.taskId}/activities/${activityId}/restore`, { method: 'POST' })
    // Reload task data by emitting an event or just alert
    alert("Version restaurée ! Veuillez rafraîchir la tâche.")
  } catch (e) {
    console.error(e)
    alert("Erreur lors de la restauration")
  }
}

function getUserName(userId) {
  const member = props.workspaceMembers.find(m => m.user_id === userId)
  return member ? member.name : 'Utilisateur inconnu'
}

function getUserInitial(userId) {
  const name = getUserName(userId)
  return name ? name.charAt(0).toUpperCase() : '?'
}

function getActivityText(activity) {
  switch (activity.activity_type) {
    case 'update_title': return `a modifié le titre`
    case 'update_description': return `a modifié la description`
    case 'move_column': return `a déplacé la tâche`
    case 'toggle_validation': return `a modifié la validation`
    case 'add_assignee': return `a ajouté un assigné`
    case 'remove_assignee': return `a retiré un assigné`
    case 'add_label': return `a ajouté une étiquette`
    case 'remove_label': return `a retiré une étiquette`
    case 'upload_attachment': return `a ajouté une pièce jointe`
    default: return `a effectué une action`
  }
}

function formatDate(dateStr) {
  if (!dateStr) return ''
  const d = new Date(dateStr)
  return d.toLocaleDateString() + ' ' + d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}
</script>

<style scoped>
.activities-list {
  display: flex;
  flex-direction: column;
  gap: 16px;
  max-height: 250px;
  overflow-y: auto;
  padding-right: 8px;
}
.activities-list::-webkit-scrollbar {
  width: 6px;
}
.activities-list::-webkit-scrollbar-track {
  background: transparent;
}
.activities-list::-webkit-scrollbar-thumb {
  background-color: var(--border-color);
  border-radius: 10px;
}
.activity-item {
  padding: 12px;
  background: var(--bg-color-main);
  border: 1px solid var(--border-color);
  border-radius: 8px;
}
.activity-header {
  display: flex;
  align-items: center;
  gap: 12px;
}
.activity-avatar {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  background: var(--primary-color);
  color: white;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.8rem;
  font-weight: 600;
}
.activity-info {
  flex: 1;
  font-size: 0.9rem;
}
.activity-user {
  font-weight: 600;
  color: var(--text-color);
  margin-right: 4px;
}
.activity-text {
  color: var(--text-color-3);
}
.activity-time {
  font-size: 0.8rem;
  color: var(--text-color-3);
}
.activity-actions {
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid var(--border-color);
  display: flex;
  justify-content: flex-end;
}
.small-btn {
  padding: 4px 8px;
  font-size: 0.8rem;
}
.empty-state {
  color: var(--text-color-3);
  font-size: 0.9rem;
  font-style: italic;
  padding: 12px 0;
}
</style>
