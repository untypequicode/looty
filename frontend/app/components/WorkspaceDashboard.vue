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
  padding: 48px;
  max-width: 1000px;
  margin: 0 auto;
}

.dashboard-header h2 {
  font-size: 2.5rem;
  font-weight: 700;
  margin-bottom: 24px;
  color: var(--text-color);
}

.tabs-container {
  display: flex;
  gap: 24px;
  border-bottom: 1px solid var(--border-color);
  margin-bottom: 24px;
}

.tab-btn {
  background: transparent;
  border: none;
  padding: 12px 0;
  font-size: 1rem;
  font-weight: 500;
  color: var(--text-color-3);
  cursor: pointer;
  position: relative;
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
  left: 0;
  right: 0;
  height: 2px;
  background: var(--primary-color);
}

.filter-row {
  margin-bottom: 24px;
}

.scope-select {
  padding: 6px 12px;
  font-size: 0.9rem;
  width: auto;
  border-radius: 4px;
}

.page-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.page-item {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 16px;
  background: transparent;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.2s;
}

.page-item:hover {
  background: var(--bg-color-2);
}

.page-icon {
  font-size: 1.5rem;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  background: var(--bg-color-3);
  border-radius: 4px;
}

.page-details {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.page-title {
  font-weight: 500;
  font-size: 1.1rem;
  color: var(--text-color);
  display: flex;
  align-items: center;
}

.page-meta {
  font-size: 0.85rem;
  color: var(--text-color-3);
}

.empty-state {
  text-align: center;
  padding: 48px;
  color: var(--text-color-3);
  background: var(--bg-color-2);
  border-radius: 8px;
  border: 1px dashed var(--border-color);
  margin-top: 24px;
}
</style>
