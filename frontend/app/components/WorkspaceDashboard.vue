<template>
  <div class="workspace-dashboard">
    <div class="dashboard-header">
      <h2>Accueil</h2>
      
      <div class="tabs-container">
        <button 
          v-for="tab in tabs" 
          :key="tab.id" 
          class="tab-btn" 
          :class="{ active: currentTab === tab.id }"
          @click="currentTab = tab.id"
        >
          {{ tab.label }}
        </button>
      </div>

      <div class="filter-row" v-if="currentTab !== 'created'">
        <select v-model="scopeFilter" class="modern-select scope-select">
          <option value="me">Pour moi</option>
          <option value="all">Tout le monde</option>
        </select>
      </div>
    </div>

    <div class="page-list">
      <div v-for="page in sortedPages" :key="page.id" class="page-item" @click="emit('open-page', page)">
        <div class="page-icon">{{ page.icon || '📄' }}</div>
        <div class="page-details">
          <div class="page-title">
            {{ page.title || 'Sans titre' }}
            <span v-if="page.is_favorite" style="color: #f59e0b; margin-left: 4px;">★</span>
          </div>
          <div class="page-meta">
            {{ getPageMetaText(page) }}
          </div>
        </div>
      </div>
      <div v-if="sortedPages.length === 0" class="empty-state">
        Aucune page trouvée pour ces critères.
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'

const props = defineProps<{
  pages: any[],
  boards: any[],
  myTasks: any[],
  userId: string | null
}>()

const emit = defineEmits(['open-task', 'open-page'])

const currentTab = ref('recent')
const scopeFilter = ref('me')

const tabs = [
  { id: 'recent', label: 'Vu récemment' },
  { id: 'popular', label: 'Populaire' },
  { id: 'updated', label: 'Récemment mis à jour' },
  { id: 'created', label: 'Créé par moi' },
]

const formatRelativeDate = (ds: string) => {
  if (!ds) return ''
  return new Date(ds).toLocaleDateString() // Simplifié, idéalement date-fns formatDistanceToNow
}

const getPageMetaText = (page: any) => {
  let meta = ""
  if (page.updated_at) {
    meta += `Mis à jour le ${formatRelativeDate(page.updated_at)}`
  }
  
  if (currentTab.value === 'recent' && page.last_viewed_at) {
    meta += ` • Vu le ${formatRelativeDate(page.last_viewed_at)}`
  } else if (currentTab.value === 'popular') {
    let views = scopeFilter.value === 'me' ? (page.view_count || 0) : (page.total_view_count || 0)
    meta += ` • ${views} vue(s)`
  }
  
  return meta
}

const sortedPages = computed(() => {
  let p = [...props.pages]
  
  if (currentTab.value === 'recent') {
    // Si 'me', on trie par last_viewed_at. Si 'all', on n'a pas last_viewed_at global donc on trie par updated_at
    if (scopeFilter.value === 'me') {
      p = p.filter(x => x.last_viewed_at)
      p.sort((a, b) => new Date(b.last_viewed_at).getTime() - new Date(a.last_viewed_at).getTime())
    } else {
      p.sort((a, b) => new Date(b.updated_at || 0).getTime() - new Date(a.updated_at || 0).getTime())
    }
  } else if (currentTab.value === 'popular') {
    p.sort((a, b) => {
      let va = scopeFilter.value === 'me' ? (a.view_count || 0) : (a.total_view_count || 0)
      let vb = scopeFilter.value === 'me' ? (b.view_count || 0) : (b.total_view_count || 0)
      return vb - va
    })
  } else if (currentTab.value === 'updated') {
    if (scopeFilter.value === 'me') {
      p = p.filter(x => x.updated_by === props.userId)
    }
    p.sort((a, b) => new Date(b.updated_at || 0).getTime() - new Date(a.updated_at || 0).getTime())
  } else if (currentTab.value === 'created') {
    p = p.filter(x => x.created_by === props.userId)
    p.sort((a, b) => new Date(b.created_at || 0).getTime() - new Date(a.created_at || 0).getTime())
  }
  
  return p
})
</script>

<style scoped>
.workspace-dashboard {
  padding: 40px;
  max-width: 900px;
  margin: 0 auto;
}

@media (max-width: 768px) {
  .workspace-dashboard {
    padding: 16px;
  }
}

.dashboard-header h2 {
  font-size: 1.5rem;
  font-weight: 600;
  margin-bottom: 20px;
  color: var(--text-color);
  letter-spacing: -0.02em;
}

.tabs-container {
  display: flex;
  gap: 0;
  border-bottom: 1px solid var(--border-color);
  margin-bottom: 20px;
  overflow-x: auto;
  -webkit-overflow-scrolling: touch;
  scrollbar-width: none;
}
.tabs-container::-webkit-scrollbar {
  display: none;
}

.tab-btn {
  background: transparent;
  border: none;
  padding: 8px 12px;
  font-size: 0.8125rem;
  font-weight: 500;
  color: var(--text-color-3);
  cursor: pointer;
  position: relative;
  white-space: nowrap;
  transition: color var(--transition-fast);
}

.tab-btn:hover {
  color: var(--text-color-2);
}

.tab-btn.active {
  color: var(--text-color);
}

.tab-btn.active::after {
  content: '';
  position: absolute;
  bottom: -1px;
  left: 12px;
  right: 12px;
  height: 2px;
  background: var(--primary-color);
  border-radius: 1px;
}

.filter-row {
  margin-bottom: 16px;
}

.scope-select {
  padding: 5px 10px;
  font-size: 0.8125rem;
  width: auto;
  border-radius: var(--radius-sm);
}

.page-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.page-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  background: transparent;
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: background var(--transition-fast);
}

.page-item:hover {
  background: var(--bg-color-hover);
}

.page-icon {
  font-size: 1.125rem;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  background: var(--bg-color-3);
  border-radius: var(--radius-sm);
  flex-shrink: 0;
}

.page-details {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.page-title {
  font-weight: 500;
  font-size: 0.875rem;
  color: var(--text-color);
  display: flex;
  align-items: center;
}

.page-meta {
  font-size: 0.75rem;
  color: var(--text-color-3);
}

.empty-state {
  text-align: center;
  padding: 40px;
  color: var(--text-color-3);
  font-size: 0.8125rem;
  border-radius: var(--radius-md);
  border: 1px dashed var(--border-color);
  margin-top: 16px;
}
</style>
.workspace-dashboard {
