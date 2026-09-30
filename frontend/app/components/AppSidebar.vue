<script setup lang="ts">
import { computed } from 'vue'
import { useColorMode } from '@vueuse/core'
import type { User, Workspace, Board, Page } from '../types'
import PageTree from './PageTree.vue'

const props = defineProps<{
  user: { authenticated: boolean, user?: User }
  workspacesData?: { workspaces: Workspace[] }
  isLoadingWorkspaces: boolean
  activeWorkspaceId: string | null
  activeWorkspaceRole: string | null
  boards: Board[]
  pages: Page[]
  isLoadingPages: boolean
  activeBoard: Board | null
  activePage: Page | null
  currentModule: 'kanban' | 'wiki'
}>()

const emit = defineEmits<{
  (e: 'select-workspace', id: string): void
  (e: 'select-board', board: Board): void
  (e: 'select-page', page: Page): void
  (e: 'open-settings'): void
  (e: 'open-rename', type: 'workspace' | 'board' | 'column', id: string, name: string): void
  (e: 'delete-workspace', id: string): void
  (e: 'delete-board', id: string): void
  (e: 'delete-page', id: string): void
  (e: 'create-page'): void
  (e: 'create-subpage', id: string): void
  (e: 'show-create-modal'): void
  (e: 'show-board-modal'): void
  (e: 'copy-invite-link'): void
  (e: 'move-page', payload: any): void
}>()

const getInitials = (name: string) => name ? name.charAt(0).toUpperCase() : '?'
const colorMode = ref('auto')

import { onMounted } from 'vue'

const applyTheme = () => {
  if (typeof document === 'undefined') return
  if (colorMode.value === 'dark' || (colorMode.value === 'auto' && window.matchMedia('(prefers-color-scheme: dark)').matches)) {
    document.documentElement.classList.add('dark')
    document.documentElement.classList.remove('light')
  } else {
    document.documentElement.classList.remove('dark')
    document.documentElement.classList.add('light')
  }
}

onMounted(() => {
  const saved = localStorage.getItem('theme-mode')
  if (saved) colorMode.value = saved
  applyTheme()

  if (typeof window !== 'undefined') {
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
      if (colorMode.value === 'auto') applyTheme()
    })
  }
})

const toggleTheme = () => {
  if (colorMode.value === 'auto') colorMode.value = 'light'
  else if (colorMode.value === 'light') colorMode.value = 'dark'
  else colorMode.value = 'auto'
  
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem('theme-mode', colorMode.value)
  }
  applyTheme()
}

const showSearch = ref(false)
const searchQuery = ref('')
const searchResults = ref([])
const isSearching = ref(false)

const handleSearch = async () => {
  if (!searchQuery.value || searchQuery.value.trim().length < 2) {
    searchResults.value = []
    return
  }
  isSearching.value = true
  try {
    const res = await $fetch(`/api/workspaces/${props.activeWorkspaceId}/search?q=${encodeURIComponent(searchQuery.value)}`)
    searchResults.value = res.results || []
  } catch (e) {
    console.error("Search error", e)
  }
  isSearching.value = false
}

const selectSearchResult = (result) => {
  if (result.type === 'page') {
    const p = props.pages.find(page => page.id === result.id)
    if (p) {
      emit('select-page', p)
    }
  }
  showSearch.value = false
  searchQuery.value = ''
  searchResults.value = []
}

let searchTimeout = null
const onSearchInput = () => {
  clearTimeout(searchTimeout)
  searchTimeout = setTimeout(handleSearch, 300)
}

const favoritePages = computed(() => {
  return (props.pages || []).filter(p => p.is_favorite)
})
</script>

<template>
  <aside class="sidebar">
    <div class="sidebar-header"><div class="logo">Looty</div></div>
    
    <div class="sidebar-content">
      <div v-if="activeWorkspaceId" class="sidebar-search" style="padding: 1rem 0 0 0;">
        <button @click="showSearch = true" class="search-btn" style="width: 100%; display: flex; align-items: center; gap: 8px; background: var(--bg-color-3); border: 1px solid var(--border-color); padding: 8px 12px; border-radius: var(--radius-md); color: var(--text-color-3); cursor: pointer; transition: all 0.2s;">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" /></svg>
          <span>Rechercher...</span>
        </button>
      </div>

      <!-- Search Modal -->
      <ClientOnly>
        <Teleport to="body">
          <div v-if="showSearch" class="modal-overlay spotlight-overlay" @click.self="showSearch = false">
            <div class="spotlight-modal">
              <div class="spotlight-input-wrapper">
                <svg class="spotlight-icon" xmlns="http://www.w3.org/2000/svg" width="20" height="20" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" /></svg>
                <input 
                  v-model="searchQuery" 
                  @input="onSearchInput"
                  type="text" 
                  placeholder="Rechercher des documents, pages..." 
                  class="spotlight-input" 
                  autofocus
                />
                <div class="spotlight-shortcut">ESC</div>
              </div>
              <div class="spotlight-results" v-if="searchQuery.length > 0">
                <div v-if="isSearching" class="spotlight-state">
                  Recherche...
                </div>
                <div v-else-if="searchResults.length === 0 && searchQuery.length >= 2" class="spotlight-state">
                  Aucun résultat pour "<span style="color: var(--text-color);">{{ searchQuery }}</span>"
                </div>
                <div v-else-if="searchQuery.length < 2" class="spotlight-state">
                  Tapez au moins 2 lettres...
                </div>
                <div v-else class="spotlight-list">
                  <div 
                    v-for="res in searchResults" 
                    :key="res.id"
                    @click="selectSearchResult(res)"
                    class="spotlight-item"
                  >
                    <div class="spotlight-item-icon">
                      <svg v-if="res.type === 'page'" xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z" /></svg>
                      <svg v-else xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" /></svg>
                    </div>
                    <div class="spotlight-item-content">
                      <div class="spotlight-item-title">{{ res.title }}</div>
                      <div class="spotlight-item-snippet" v-if="res.snippet">{{ res.snippet.replace(/<[^>]*>?/gm, '') }}...</div>
                    </div>
                    <div class="spotlight-item-arrow">
                      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" /></svg>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </Teleport>
      </ClientOnly>

      <div class="section-title">
        <span>Espaces de Travail</span>
        <div style="display: flex; gap: 0.5rem;">
          <button v-if="activeWorkspaceId" @click="emit('copy-invite-link')" class="icon-btn" title="Inviter des membres">🔗</button>
          <button @click="emit('show-create-modal')" class="icon-btn" title="Créer un espace">+</button>
        </div>
      </div>
      
      <ul class="workspace-list">
        <li v-if="isLoadingWorkspaces" class="loading-item">Chargement...</li>
        <li v-else-if="!workspacesData?.workspaces?.length" class="empty-item">Aucun espace</li>
        <li v-for="ws in workspacesData?.workspaces" :key="ws.id"
          :class="['workspace-item', { active: activeWorkspaceId === ws.id }]" 
          @click="emit('select-workspace', ws.id)">
          <div style="display: flex; align-items: center; gap: 10px; flex: 1">
            <div class="ws-avatar">{{ getInitials(ws.name) }}</div>
            <span class="ws-name">{{ ws.name }}</span>
          </div>
          <div class="item-actions" v-if="activeWorkspaceId === ws.id">
            <button @click.stop="emit('open-settings')" title="Paramètres (Membres)">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"/><path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"/></svg>
            </button>
            <button v-if="ws.role === 'owner' || ws.role === 'admin'" @click.stop="emit('open-rename', 'workspace', ws.id, ws.name)" title="Renommer">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z"/></svg>
            </button>
            <button v-if="ws.role === 'owner' || ws.role === 'admin'" @click.stop="emit('delete-workspace', ws.id)" title="Supprimer">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/></svg>
            </button>
          </div>
        </li>
      </ul>

      <div v-if="activeWorkspaceId" class="section-title" style="margin-top:2rem;">
        <span>Favoris</span>
      </div>
      <div v-if="activeWorkspaceId && favoritePages.length === 0" class="empty-item">Aucun favori</div>
      <PageTree
        v-if="activeWorkspaceId && favoritePages.length > 0"
        :pages="pages"
        :root-ids="favoritePages.map(p => p.id)"
        :level="0"
        :active-page="activePage"
        :current-module="currentModule"
        @select-page="emit('select-page', $event)"
        @delete-page="emit('delete-page', $event)"
        @create-subpage="emit('create-subpage', $event)"
        @move-page="emit('move-page', $event)"
      />

      <div v-if="activeWorkspaceId" class="section-title" style="margin-top:2rem;">
        <span>Tableaux</span>
        <button v-if="activeWorkspaceRole !== 'viewer'" @click="emit('show-board-modal')" class="icon-btn" title="Créer un tableau">+</button>
      </div>
      
      <ul v-if="activeWorkspaceId" class="workspace-list">
        <li v-if="boards.length === 0" class="empty-item">Aucun tableau</li>
        <li v-for="b in boards" :key="b.id"
          :class="['workspace-item', { active: activeBoard?.id === b.id && currentModule === 'kanban' }]" 
          @click="emit('select-board', b)">
          <span class="ws-name" style="flex: 1; display: flex; align-items: center; gap: 6px;">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" class="item-icon"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"/><path d="M9 3v18M15 3v18M3 9h18M3 15h18"/></svg>
            {{ b.name }}
          </span>
          <div class="item-actions" v-if="activeBoard?.id === b.id && activeWorkspaceRole !== 'viewer'">
            <button @click.stop="emit('open-rename', 'board', b.id, b.name)" title="Renommer">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z"/></svg>
            </button>
            <button @click.stop="emit('delete-board', b.id)" title="Supprimer">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/></svg>
            </button>
          </div>
        </li>
      </ul>

      <div v-if="activeWorkspaceId" class="section-title" style="margin-top:2rem;">
        <span>Wiki (Pages)</span>
        <button @click="emit('create-page')" class="icon-btn" title="Créer une page">+</button>
      </div>
      
      <div v-if="activeWorkspaceId && !isLoadingPages && pages.length === 0" class="empty-item">Aucune page</div>
      <div v-else-if="isLoadingPages" class="loading-item">Chargement...</div>
      <PageTree
        v-if="activeWorkspaceId && pages.length > 0"
        :pages="pages"
        :parent-id="null"
        :level="0"
        :active-page="activePage"
        :current-module="currentModule"
        @select-page="emit('select-page', $event)"
        @delete-page="emit('delete-page', $event)"
        @create-subpage="emit('create-subpage', $event)"
        @move-page="emit('move-page', $event)"
      />
    </div>
    
    <div class="sidebar-footer">
      <div class="user-profile" v-if="user?.user">
        <div class="user-avatar">{{ getInitials(user.user.name) }}</div>
        <div class="user-info">
          <p class="user-name">{{ user.user.name }}</p>
          <p class="user-email">{{ user.user.email }}</p>
        </div>
      </div>
      <div style="display: flex; gap: 8px; align-items: center; justify-content: space-between;">
        <button class="theme-toggle-btn" @click="toggleTheme" :title="'Thème actuel: ' + colorMode">
          <svg v-if="colorMode === 'light'" xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" /></svg>
          <svg v-else-if="colorMode === 'dark'" xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" /></svg>
          <svg v-else xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" /></svg>
        </button>
        <div style="display: flex; gap: 8px;">
          <NuxtLink v-if="user?.user?.is_super_admin" to="/admin" class="logout-btn" style="background: var(--primary-color); color: white; text-decoration: none;">Admin</NuxtLink>
          <a href="/api/auth/logout" class="logout-btn">Déconnexion</a>
        </div>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  width: 240px;
  background-color: var(--bg-color-2);
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--border-color);
  z-index: 10;
  transition: width var(--transition-smooth);
}
.sidebar-header {
  padding: 16px 16px 8px;
}
.logo {
  font-size: 0.8125rem;
  font-weight: 600;
  color: var(--text-color);
  letter-spacing: -0.02em;
  display: flex;
  align-items: center;
  gap: 6px;
}
.logo::before {
  content: '✦';
  color: var(--primary-color);
  font-size: 0.875rem;
}
.sidebar-content {
  flex: 1;
  padding: 0 8px;
  overflow-y: auto;
}
.section-title {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 0.6875rem;
  color: var(--text-color-3);
  margin-bottom: 2px;
  margin-top: 20px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  padding: 0 8px;
}
.icon-btn {
  background: none;
  border: none;
  color: var(--text-color-3);
  cursor: pointer;
  padding: 2px;
  border-radius: var(--radius-xs);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.8125rem;
  transition: all var(--transition-fast);
  line-height: 1;
}
.icon-btn:hover {
  background: var(--bg-color-hover);
  color: var(--text-color);
}
.workspace-list {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
}
.loading-item, .empty-item {
  color: var(--text-color-3);
  font-size: 0.75rem;
  padding: 6px 8px;
}
.workspace-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 8px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  color: var(--text-color-2);
  font-size: 0.8125rem;
  font-weight: 400;
  transition: all var(--transition-fast);
  position: relative;
}
.workspace-item:hover {
  background: var(--bg-color-hover);
  color: var(--text-color);
}
.workspace-item.active {
  background: var(--primary-color-subtle);
  color: var(--primary-color);
  font-weight: 500;
}
.ws-avatar {
  width: 18px;
  height: 18px;
  border-radius: var(--radius-xs);
  background: var(--border-color);
  color: var(--text-color-2);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.625rem;
  font-weight: 600;
  flex-shrink: 0;
}
.workspace-item.active .ws-avatar {
  background: var(--primary-color);
  color: #fff;
}
.ws-name {
  font-size: 0.8125rem;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.sidebar-footer {
  padding: 12px;
  border-top: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.user-profile {
  display: flex;
  align-items: center;
  gap: 8px;
}
.user-avatar {
  width: 24px;
  height: 24px;
  border-radius: var(--radius-full);
  background: var(--primary-color-glass);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.6875rem;
  color: var(--primary-color);
  font-weight: 600;
  flex-shrink: 0;
}
.user-info p { margin: 0; }
.user-name { font-weight: 500; font-size: 0.8125rem; color: var(--text-color); }
.user-email { font-size: 0.6875rem; color: var(--text-color-3); }
.logout-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  text-decoration: none;
  color: var(--text-color-3);
  font-size: 0.75rem;
  transition: color var(--transition-fast);
  padding: 4px 8px;
  border-radius: var(--radius-sm);
}
.logout-btn:hover {
  color: var(--danger-color);
  background: var(--danger-glass);
}
.theme-toggle-btn {
  background: none;
  border: none;
  font-size: 1rem;
  cursor: pointer;
  padding: 4px;
  border-radius: var(--radius-sm);
  transition: background var(--transition-fast);
  color: var(--text-color-3);
  display: flex;
  align-items: center;
  justify-content: center;
}
.theme-toggle-btn:hover {
  background: var(--bg-color-hover);
  color: var(--text-color-2);
}
.item-actions {
  display: none;
  gap: 2px;
  position: absolute;
  right: 4px;
  background: var(--bg-color-2);
  padding: 2px;
  border-radius: var(--radius-xs);
  border: 1px solid var(--border-color);
}
.workspace-item:hover .item-actions {
  display: flex;
}
.item-actions button {
  background: none;
  border: none;
  color: var(--text-color-3);
  cursor: pointer;
  padding: 3px;
  border-radius: var(--radius-xs);
  font-size: 0.75rem;
  transition: all var(--transition-fast);
  display: flex;
  align-items: center;
  justify-content: center;
}
.item-actions button:hover {
  background: var(--bg-color-hover);
  color: var(--text-color);
}

/* Spotlight Search */
.spotlight-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
  z-index: 1000;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding-top: 18vh;
}

.spotlight-modal {
  width: 100%;
  max-width: 560px;
  background: var(--bg-color-modal);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-float);
  border: 1px solid var(--border-color);
  overflow: hidden;
  display: flex;
  flex-direction: column;
  animation: spotlightIn 0.15s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes spotlightIn {
  from { opacity: 0; transform: translateY(-8px) scale(0.98); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

.spotlight-input-wrapper {
  display: flex;
  align-items: center;
  padding: 0 16px;
  border-bottom: 1px solid var(--border-color);
}

.spotlight-icon {
  color: var(--text-color-3);
  margin-right: 10px;
  flex-shrink: 0;
}

.spotlight-input {
  flex: 1;
  background: transparent;
  border: none;
  padding: 14px 0;
  font-size: 1rem;
  color: var(--text-color);
  outline: none;
  font-family: inherit;
}
.spotlight-input::placeholder {
  color: var(--text-color-3);
  font-weight: 400;
}

.spotlight-shortcut {
  font-size: 0.6875rem;
  color: var(--text-color-3);
  background: var(--bg-color-3);
  padding: 2px 6px;
  border-radius: var(--radius-xs);
  font-weight: 500;
  border: 1px solid var(--border-color);
  font-family: var(--font-mono);
}

.spotlight-results {
  max-height: 320px;
  overflow-y: auto;
  padding: 8px;
}

.spotlight-state {
  padding: 24px 16px;
  text-align: center;
  color: var(--text-color-3);
  font-size: 0.875rem;
}

.spotlight-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.spotlight-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 10px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background var(--transition-fast);
}

.spotlight-item:hover {
  background: var(--bg-color-hover);
}

.spotlight-item-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: var(--radius-sm);
  background: var(--bg-color-3);
  color: var(--text-color-2);
  flex-shrink: 0;
}
.spotlight-item:hover .spotlight-item-icon {
  background: var(--primary-color-glass);
  color: var(--primary-color);
}

.spotlight-item-content {
  flex: 1;
  overflow: hidden;
  min-width: 0;
}

.spotlight-item-title {
  font-weight: 500;
  font-size: 0.875rem;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.spotlight-item-snippet {
  font-size: 0.75rem;
  color: var(--text-color-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.spotlight-item-arrow {
  color: var(--text-color-3);
  opacity: 0;
  transform: translateX(-4px);
  transition: all var(--transition-fast);
  flex-shrink: 0;
}
.spotlight-item:hover .spotlight-item-arrow {
  opacity: 1;
  transform: translateX(0);
  color: var(--text-color-2);
}
</style>

