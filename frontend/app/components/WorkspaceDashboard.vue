<template>
  <div class="workspace-dashboard">
    <div class="dashboard-header">
      <h2>Récapitulatif de l'espace de travail</h2>
      <p class="subtitle">Vos tâches et pages récentes</p>
    </div>

    <div class="dashboard-grid">
      <div class="dashboard-card tasks-card">
        <h3>Vos tâches ({{ sortedTasks.length }})</h3>
        <div class="task-list">
          <div v-for="task in sortedTasks" :key="task.id" class="task-item" @click="emit('open-task', task)">
            <div class="task-icon">
              <svg v-if="task.is_completed" style="color: var(--success-color);" xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" /></svg>
              <div v-else style="width: 16px; height: 16px; border: 2px solid var(--text-color-3); border-radius: 4px;"></div>
            </div>
            <div class="task-info">
              <span class="task-title" :class="{ 'is-completed': task.is_completed }">{{ task.title }}</span>
              <span class="task-date" v-if="task.due_date">{{ formatDate(task.due_date) }}</span>
            </div>
          </div>
          <div v-if="sortedTasks.length === 0" class="empty-state">
            Aucune tâche ne vous est assignée.
          </div>
        </div>
      </div>

      <div class="dashboard-card pages-card">
        <div class="pages-header">
          <h3>Pages du Wiki</h3>
          <select v-model="pageSort" class="modern-input sort-select">
            <option value="recent">Récentes</option>
            <option value="alpha">A-Z</option>
          </select>
        </div>
        <div class="page-list">
          <div v-for="page in sortedPages" :key="page.id" class="page-item" @click="emit('open-page', page)">
            <span class="page-icon">{{ page.icon || '📄' }}</span>
            <span class="page-title">{{ page.title }}</span>
          </div>
          <div v-if="sortedPages.length === 0" class="empty-state">
            Aucune page dans ce wiki.
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'

const props = defineProps<{
  pages: any[],
  boards: any[],
  myTasks: any[],
  userId: string | null
}>()

const emit = defineEmits(['open-task', 'open-page'])

const pageSort = ref('recent')

const sortedTasks = computed(() => {
  return [...props.myTasks].sort((a, b) => {
    if (a.due_date && b.due_date) {
      return new Date(a.due_date).getTime() - new Date(b.due_date).getTime()
    }
    if (a.due_date) return -1
    if (b.due_date) return 1
    return 0
  })
})

const sortedPages = computed(() => {
  const p = [...props.pages]
  if (pageSort.value === 'alpha') {
    return p.sort((a, b) => (a.title || '').localeCompare(b.title || ''))
  }
  // Default to recent (using updated_at if available, otherwise reverse order)
  return p.sort((a, b) => {
    const da = a.updated_at ? new Date(a.updated_at).getTime() : 0
    const db = b.updated_at ? new Date(b.updated_at).getTime() : 0
    return db - da
  })
})

function formatDate(ds: string) {
  if (!ds) return ''
  const d = new Date(ds)
  return d.toLocaleDateString()
}
</script>

<style scoped>
.workspace-dashboard {
  padding: 32px;
  max-width: 1200px;
  margin: 0 auto;
}

.dashboard-header {
  margin-bottom: 32px;
}
.dashboard-header h2 {
  font-size: 2rem;
  font-weight: 700;
  margin-bottom: 8px;
  background: var(--gradient-primary);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}
.subtitle {
  color: var(--text-color-2);
  font-size: 1.1rem;
}

.dashboard-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 24px;
}

@media (max-width: 768px) {
  .dashboard-grid {
    grid-template-columns: 1fr;
  }
}

.dashboard-card {
  background: var(--bg-color-2);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: 24px;
  display: flex;
  flex-direction: column;
}

.dashboard-card h3 {
  font-size: 1.25rem;
  font-weight: 600;
  margin-bottom: 16px;
  color: var(--text-color);
}

.pages-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}
.pages-header h3 {
  margin-bottom: 0;
}
.sort-select {
  padding: 6px 12px;
  font-size: 0.9rem;
  width: auto;
}

.task-list, .page-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.task-item, .page-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  background: var(--bg-color);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: all 0.2s ease;
}

.task-item:hover, .page-item:hover {
  border-color: var(--primary-color);
  transform: translateY(-2px);
  box-shadow: var(--shadow-sm);
}

.task-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.task-title {
  font-weight: 500;
  color: var(--text-color);
}
.task-title.is-completed {
  text-decoration: line-through;
  opacity: 0.6;
}
.task-date {
  font-size: 0.8rem;
  color: var(--warning-color);
}

.page-title {
  font-weight: 500;
  color: var(--text-color);
}
.page-icon {
  font-size: 1.2rem;
}

.empty-state {
  text-align: center;
  padding: 32px;
  color: var(--text-color-3);
  background: var(--bg-color);
  border-radius: var(--radius-md);
  border: 1px dashed var(--border-color);
}
</style>
