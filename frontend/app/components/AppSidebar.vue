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
          <div v-if="showSearch" class="modal-overlay" @click.self="showSearch = false">
            <div class="modal-content search-modal" style="margin-top: 10vh; max-width: 600px; padding: 0;">
              <input 
                v-model="searchQuery" 
                @input="onSearchInput"
                type="text" 
                placeholder="Rechercher dans l'espace (pages, contenus)..." 
                class="modern-input search-modal-input" 
                style="border: none; border-bottom: 1px solid var(--border-color); border-radius: var(--radius-md) var(--radius-md) 0 0; margin-bottom: 0; padding: 16px; font-size: 1.1rem; box-shadow: none;"
                autofocus
              />
              <div class="search-results" style="max-height: 400px; overflow-y: auto; padding: 8px;">
                <div v-if="isSearching" style="padding: 16px; text-align: center; color: var(--text-color-3);">Recherche en cours...</div>
                <div v-else-if="searchResults.length === 0 && searchQuery.length >= 2" style="padding: 16px; text-align: center; color: var(--text-color-3);">Aucun résultat trouvé</div>
                <div v-else-if="searchQuery.length < 2" style="padding: 16px; text-align: center; color: var(--text-color-3);">Tapez au moins 2 caractères</div>
                <div 
                  v-else
                  v-for="res in searchResults" 
                  :key="res.id"
                  @click="selectSearchResult(res)"
                  class="search-result-item"
                  style="padding: 12px; cursor: pointer; border-radius: var(--radius-md); transition: background 0.1s;"
                >
                  <div style="font-weight: 500; color: var(--text-color); margin-bottom: 4px; display: flex; align-items: center; gap: 8px;">
                    <span v-if="res.type === 'page'">📄</span> {{ res.title }}
                  </div>
                  <div v-if="res.snippet" style="font-size: 0.85rem; color: var(--text-color-3); white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">
                    {{ res.snippet.replace(/<[^>]*>?/gm, '') }}...
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
  width: 250px; 
  background-color: var(--bg-color-2); 
  display: flex; 
  flex-direction: column; 
  border-right: 1px solid var(--border-color); 
  z-index: 10;
  transition: width var(--transition-smooth);
}
.sidebar-header { 
  padding: 1.25rem 1rem 0.5rem; 
}
.logo { 
  font-size: 0.9rem; 
  font-weight: 600; 
  color: var(--text-color); 
  letter-spacing: -0.3px; 
  display: flex;
  align-items: center;
  gap: 8px;
}
.logo::before {
  content: '✦';
  color: var(--primary-color);
}
.sidebar-content { 
  flex: 1; 
  padding: 0 0.75rem; 
  overflow-y: auto; 
}
.section-title { 
  display: flex; 
  justify-content: space-between; 
  align-items: center; 
  font-size: 0.65rem; 
  color: var(--text-color-3); 
  margin-bottom: 0.25rem; 
  margin-top: 1.5rem; 
  font-weight: 600; 
  text-transform: uppercase; 
  letter-spacing: 0.8px; 
  padding: 0 0.5rem; 
}
.icon-btn { 
  background: none; 
  border: none; 
  color: var(--text-color-3); 
  cursor: pointer; 
  padding: 4px; 
  border-radius: var(--radius-sm); 
  display: flex; 
  align-items: center; 
  justify-content: center; 
  font-size: 0.85rem; 
  transition: all var(--transition-fast); 
}
.icon-btn:hover { 
  background: var(--border-color); 
  color: var(--text-color); 
}
.workspace-list { 
  list-style: none; 
  padding: 0; 
  margin: 0; 
  display: flex; 
  flex-direction: column; 
  gap: 2px; 
}
.loading-item, .empty-item { 
  color: var(--text-color-3); 
  font-size: 0.8rem; 
  padding: 0.5rem 0.75rem; 
}
.workspace-item { 
  display: flex; 
  align-items: center; 
  gap: 10px; 
  padding: 6px 10px; 
  border-radius: var(--radius-sm); 
  cursor: pointer; 
  color: var(--text-color-2); 
  font-size: 0.875rem; 
  font-weight: 500; 
  transition: all var(--transition-fast); 
  position: relative;
}
.workspace-item:hover { 
  background: var(--bg-color-hover); 
  color: var(--text-color); 
}
.workspace-item.active { 
  background: var(--bg-color-hover); 
  color: var(--text-color); 
  font-weight: 600;
}
.ws-avatar { 
  width: 20px; 
  height: 20px; 
  border-radius: 4px; 
  background: var(--border-color); 
  color: var(--text-color); 
  display: flex; 
  align-items: center; 
  justify-content: center; 
  font-size: 0.65rem; 
  font-weight: 600; 
}
.workspace-item.active .ws-avatar { 
  background: var(--primary-color); 
  color: #fff;
}
.ws-name { 
  font-size: 0.875rem; 
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.sidebar-footer { 
  padding: 1rem; 
  border-top: 1px solid var(--border-color); 
  display: flex; 
  flex-direction: column; 
  gap: 0.75rem; 
}
.user-profile { 
  display: flex; 
  align-items: center; 
  gap: 10px; 
}
.user-avatar { 
  width: 28px; 
  height: 28px; 
  border-radius: 50%; 
  background: var(--primary-color-glass); 
  display: flex; 
  align-items: center; 
  justify-content: center; 
  font-size: 0.8rem; 
  color: var(--primary-color);
  font-weight: 600;
}
.user-info p { margin: 0; }
.user-name { font-weight: 500; font-size: 0.85rem; color: var(--text-color); }
.user-email { font-size: 0.75rem; color: var(--text-color-3); }
.logout-btn { 
  display: flex; 
  align-items: center; 
  gap: 8px; 
  text-decoration: none; 
  color: var(--text-color-3); 
  font-size: 0.85rem; 
  transition: color var(--transition-fast);
}
.logout-btn:hover { 
  color: var(--danger-color); 
}
.theme-toggle-btn {
  background: none;
  border: none;
  font-size: 1.1rem;
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
}
.item-actions { 
  display: none; 
  gap: 4px; 
  position: absolute;
  right: 6px;
  background: var(--bg-color-hover);
  padding: 2px;
  border-radius: var(--radius-sm);
}
.workspace-item:hover .item-actions {
  display: flex;
}
.item-actions button { 
  background: none; 
  border: none; 
  color: var(--text-color-3); 
  cursor: pointer; 
  padding: 4px; 
  border-radius: var(--radius-sm); 
  font-size: 0.75rem; 
  transition: all var(--transition-fast); 
  display: flex;
  align-items: center;
  justify-content: center;
}
.item-actions button:hover { 
  background: var(--border-color); 
  color: var(--text-color); 
}
</style>
