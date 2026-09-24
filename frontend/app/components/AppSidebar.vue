<script setup lang="ts">
import { computed } from 'vue'
import type { User, Workspace, Board, Page } from '../types'

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
  (e: 'show-create-modal'): void
  (e: 'show-board-modal'): void
  (e: 'copy-invite-link'): void
}>()

const getInitials = (name: string) => name ? name.charAt(0).toUpperCase() : '?'
</script>

<template>
  <aside class="sidebar">
    <div class="sidebar-header"><div class="logo">Looty</div></div>
    
    <div class="sidebar-content">
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
            <button @click.stop="emit('open-settings')" title="Paramètres (Membres)">⚙️</button>
            <button v-if="ws.role === 'owner' || ws.role === 'admin'" @click.stop="emit('open-rename', 'workspace', ws.id, ws.name)" title="Renommer">✏️</button>
            <button v-if="ws.role === 'owner' || ws.role === 'admin'" @click.stop="emit('delete-workspace', ws.id)" title="Supprimer">🗑️</button>
          </div>
        </li>
      </ul>

      <div v-if="activeWorkspaceId" class="section-title" style="margin-top:2rem;">
        <span>Tableaux</span>
        <button v-if="activeWorkspaceRole !== 'viewer'" @click="emit('show-board-modal')" class="icon-btn" title="Créer un tableau">+</button>
      </div>
      
      <ul v-if="activeWorkspaceId" class="workspace-list">
        <li v-if="boards.length === 0" class="empty-item">Aucun tableau</li>
        <li v-for="b in boards" :key="b.id"
          :class="['workspace-item', { active: activeBoard?.id === b.id && currentModule === 'kanban' }]" 
          @click="emit('select-board', b)">
          <span class="ws-name" style="flex: 1"># {{ b.name }}</span>
          <div class="item-actions" v-if="activeBoard?.id === b.id && activeWorkspaceRole !== 'viewer'">
            <button @click.stop="emit('open-rename', 'board', b.id, b.name)" title="Renommer">✏️</button>
            <button @click.stop="emit('delete-board', b.id)" title="Supprimer">🗑️</button>
          </div>
        </li>
      </ul>

      <div v-if="activeWorkspaceId" class="section-title" style="margin-top:2rem;">
        <span>Wiki (Pages)</span>
        <button @click="emit('create-page')" class="icon-btn" title="Créer une page">+</button>
      </div>
      
      <ul v-if="activeWorkspaceId" class="workspace-list">
        <li v-if="isLoadingPages" class="loading-item">Chargement...</li>
        <li v-else-if="pages.length === 0" class="empty-item">Aucune page</li>
        <li v-for="page in pages" :key="page.id"
          :class="['workspace-item', { active: activePage?.id === page.id && currentModule === 'wiki' }]" 
          @click="emit('select-page', page)">
          <span class="ws-name" style="flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">📄 {{ page.title || 'Nouvelle page' }}</span>
          <div class="item-actions" v-if="activePage?.id === page.id">
            <button @click.stop="emit('delete-page', page.id)" title="Supprimer">🗑️</button>
          </div>
        </li>
      </ul>
    </div>
    
    <div class="sidebar-footer">
      <div class="user-profile" v-if="user?.user">
        <div class="user-avatar">{{ getInitials(user.user.name) }}</div>
        <div class="user-info">
          <p class="user-name">{{ user.user.name }}</p>
          <p class="user-email">{{ user.user.email }}</p>
        </div>
      </div>
      <a href="/api/auth/logout" class="logout-btn">Déconnexion</a>
    </div>
  </aside>
</template>

<style scoped>
.sidebar { width: 240px; background-color: var(--bg-color-2); display: flex; flex-direction: column; border-right: 1px solid var(--border-color); z-index: 10; }
.sidebar-header { padding: 1rem 1rem 0.5rem; }
.logo { font-size: 0.9rem; font-weight: 600; color: #fff; letter-spacing: -0.3px; opacity: 0.9; }
.sidebar-content { flex: 1; padding: 0 0.5rem; overflow-y: auto; }
.section-title { display: flex; justify-content: space-between; align-items: center; font-size: 0.7rem; color: var(--text-color-3); margin-bottom: 0.25rem; margin-top: 1.25rem; font-weight: 500; text-transform: uppercase; letter-spacing: 0.5px; padding: 0 0.5rem; }
.icon-btn { background: none; border: none; color: var(--text-color-3); cursor: pointer; padding: 2px 4px; border-radius: 4px; display: flex; align-items: center; justify-content: center; font-size: 0.85rem; transition: all 0.15s; }
.icon-btn:hover { background: var(--border-color); color: var(--text-color); }
.workspace-list { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 1px; }
.loading-item, .empty-item { color: var(--text-color-3); font-size: 0.8rem; padding: 0.5rem; }
.workspace-item { display: flex; align-items: center; gap: 8px; padding: 5px 8px; border-radius: 6px; cursor: pointer; color: var(--text-color-2); font-size: 0.85rem; font-weight: 400; transition: all 0.12s; }
.workspace-item:hover { background: var(--bg-color-hover); color: var(--text-color-light); }
.workspace-item.active { background: var(--border-color); color: #fff; }
.ws-avatar { width: 20px; height: 20px; border-radius: 4px; background: linear-gradient(135deg, #5e6ad2, #7c3aed); color: white; display: flex; align-items: center; justify-content: center; font-size: 0.65rem; font-weight: 600; }
.workspace-item.active .ws-avatar { background: linear-gradient(135deg, #818cf8, #a78bfa); }
.ws-name { font-size: 0.85rem; font-weight: 500; }
.sidebar-footer { padding: 0.75rem; border-top: 1px solid var(--border-color); display: flex; flex-direction: column; gap: 0.5rem; }
.user-profile { display: flex; align-items: center; gap: 8px; }
.user-avatar { width: 22px; height: 22px; border-radius: 50%; background: #333; display: flex; align-items: center; justify-content: center; font-size: 0.7rem; color: #fff; }
.user-info p { margin: 0; }
.user-name { font-weight: 500; font-size: 0.8rem; color: var(--text-color-light); }
.user-email { font-size: 0.7rem; color: var(--text-color-3); }
.logout-btn { display: flex; align-items: center; gap: 8px; text-decoration: none; color: var(--text-color-3); font-size: 0.8rem; }
.logout-btn:hover { color: #f87171; }
.item-actions { display: flex; gap: 2px; }
.item-actions button { background: none; border: none; color: var(--text-color-3); cursor: pointer; padding: 2px 3px; border-radius: 4px; font-size: 0.7rem; transition: all 0.12s; }
.item-actions button:hover { background: var(--bg-color-modal); color: var(--text-color); }
</style>
