<template>
  <div class="presence-dock-wrapper" :class="{ 'is-expanded': isExpanded }">
    <div class="presence-docks-container">
      <!-- Mentions Pills -->
      <div v-for="mention in (activeMentions || [])" :key="mention.id" class="presence-dock mention-dock" @click="handleMentionClick(mention)">
        <div class="dock-avatar mention-avatar">
          💬
        </div>
        <div class="activity-text">
          {{ getMemberName(mention.author_id) }} vous a mentionné !
        </div>
        <button class="close-mention-btn" @click.stop="$emit('clear-mention', mention.id)">×</button>
      </div>

      <!-- Recent Activity Pill -->
      <div class="presence-dock activity-dock" v-if="currentPage && currentPage.updated_at" :title="currentPage.updated_by ? `Dernière modification par ${getMemberName(currentPage.updated_by)}` : 'Dernière modification'">
        <div class="activity-text">
          Modifié il y a {{ formatRelativeDate(currentPage.updated_at) }}
        </div>
        <div class="dock-avatar activity-avatar" v-if="currentPage.updated_by">
          {{ getMemberInitials(currentPage.updated_by) }}
        </div>
        <div class="dock-avatar activity-avatar" v-else>
          ✏️
        </div>
      </div>

      <!-- Users Pills -->
      <div class="users-docks-wrapper" @click="isExpanded = !isExpanded">
        <!-- Global Pill -->
        <div class="presence-dock">
        <div v-for="user in visibleGlobalUsers" :key="'g'+user.user_id" class="dock-avatar" :title="getUserLabel(user)">
          {{ getUserInitials(user) }}
        </div>
        <div v-if="globalOverflowCount > 0" class="dock-avatar overflow-avatar">
          +{{ globalOverflowCount }}
        </div>
        <div v-if="globalUsers.length === 0" class="dock-empty">
          Connexion...
        </div>
      </div>

      <!-- Local Pill -->
      <div class="presence-dock local-dock" v-if="localUsers.length > 0">
        <div v-for="user in visibleLocalUsers" :key="'l'+user.user_id" class="dock-avatar is-here" :class="{ 'is-offline': user.is_offline }" :title="getUserLabel(user)">
          {{ getUserInitials(user) }}
        </div>
        <div v-if="localOverflowCount > 0" class="dock-avatar overflow-avatar is-here">
          +{{ localOverflowCount }}
        </div>
        </div>
      </div>
    </div>
    
    <!-- Expanded List -->
    <div v-if="isExpanded" class="presence-expanded-list">
      <div class="expanded-header">Personnes en ligne ({{ users.length }})</div>
      <div v-for="user in sortedUsers" :key="user.user_id" class="expanded-user-item" :class="{ 'is-offline': user.is_offline }" @click="navigateToUser(user)">
        <div class="user-avatar-small">{{ getUserInitials(user) }}</div>
        <div class="user-info-small">
          <div class="user-name-small">{{ getUserName(user) }}</div>
          <div class="user-loc-small">
            <span v-if="user.location === currentLocation" class="same-loc-badge">Ici</span>
            {{ getLocationLabel(user.location) }}
            <span v-if="user.is_offline" class="offline-badge">Hors ligne</span>
          </div>
        </div>
      </div>
      <div v-if="users.length === 0" class="empty-list">Personne n'est en ligne.</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'

const props = defineProps<{
  users: any[]
  currentLocation: string
  workspaceMembers: any[]
  pages: any[]
  boards: any[]
  currentPage?: any
  activeMentions?: any[]
}>()

const emit = defineEmits(['navigate'])

const isExpanded = ref(false)

const getUserName = (user: any) => {
  return getMemberName(user.user_id)
}

const getMemberName = (userId: string) => {
  const member = props.workspaceMembers?.find(m => m.user_id === userId)
  return member ? member.name : 'Inconnu'
}

const getMemberInitials = (userId: string) => {
  const name = getMemberName(userId)
  return name.charAt(0).toUpperCase()
}

const getUserInitials = (user: any) => {
  return getMemberInitials(user.user_id)
}

const getLocationLabel = (loc: string) => {
  if (!loc || loc === 'home') return 'Accueil'
  if (loc.startsWith('page:')) {
    const id = loc.split(':')[1]
    const page = props.pages?.find(p => p.id === id)
    return page ? `Page : ${page.title || 'Sans titre'}` : 'Page'
  }
  if (loc.startsWith('board:')) {
    const id = loc.split(':')[1]
    const board = props.boards?.find(b => b.id === id)
    return board ? `Tableau : ${board.name}` : 'Tableau'
  }
  return 'En ligne'
}

const formatRelativeDate = (dateStr: string) => {
  if (!dateStr) return ''
  const diff = Math.floor((Date.now() - new Date(dateStr).getTime()) / 1000)
  if (diff < 60) return `quelques secondes`
  if (diff < 3600) return `${Math.floor(diff/60)} minutes`
  if (diff < 86400) return `${Math.floor(diff/3600)} heures`
  return `${Math.floor(diff/86400)} jours`
}

const formatLastSeen = (timestamp: number) => {
  if (!timestamp) return ''
  const diff = Math.floor((Date.now() - timestamp) / 1000)
  if (diff < 60) return `${diff} sec`
  if (diff < 3600) return `${Math.floor(diff/60)} min`
  return `${Math.floor(diff/3600)} h`
}

const getUserLabel = (user: any) => {
  let label = `${getUserName(user)} - ${getLocationLabel(user.location)}`
  if (user.is_offline) {
    label += ` (Hors ligne depuis ${formatLastSeen(user.last_seen_at)})`
  }
  return label
}

const sortedUsers = computed(() => {
  return [...(props.users || [])].sort((a, b) => {
    // Offline users last
    if (a.is_offline && !b.is_offline) return 1
    if (!a.is_offline && b.is_offline) return -1
    // Current location first
    if (a.location === props.currentLocation && b.location !== props.currentLocation) return -1
    if (b.location === props.currentLocation && a.location !== props.currentLocation) return 1
    return 0
  })
})

const globalUsers = computed(() => {
  return props.users.filter(u => !u.is_offline)
})

const localUsers = computed(() => {
  return props.users.filter(u => u.location === props.currentLocation)
})

const visibleGlobalUsers = computed(() => globalUsers.value.slice(0, 5))
const globalOverflowCount = computed(() => Math.max(0, globalUsers.value.length - 5))

const visibleLocalUsers = computed(() => localUsers.value.slice(0, 5))
const localOverflowCount = computed(() => Math.max(0, localUsers.value.length - 5))

const currentPage = computed(() => {
  if (props.currentLocation.startsWith('page:')) {
    const id = props.currentLocation.split(':')[1]
    return props.pages?.find(p => p.id === id)
  }
  return null
})

const navigateToUser = (user: any) => {
  if (user.location !== props.currentLocation) {
    if (user.location.startsWith('page:')) {
      window.location.hash = `#wiki-${user.location.split(':')[1]}`
    } else if (user.location.startsWith('board:')) {
      window.location.hash = `#kanban-${user.location.split(':')[1]}`
    } else {
      window.location.hash = `#dashboard`
    }
  }
  isExpanded.value = false
}

const handleMentionClick = (mention: any) => {
  if (mention.location !== props.currentLocation) {
    if (mention.location.startsWith('page:')) {
      window.location.hash = `#wiki-${mention.location.split(':')[1]}`
    } else if (mention.location.startsWith('board:')) {
      window.location.hash = `#kanban-${mention.location.split(':')[1]}`
    } else {
      window.location.hash = `#dashboard`
    }
  }
  emit('clear-mention', mention.id)
}
</script>

<style scoped>
.presence-dock-wrapper {
  position: fixed;
  bottom: 24px;
  right: 24px;
  z-index: 1000;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 12px;
}

.presence-docks-container {
  display: flex;
  gap: 8px;
  align-items: center;
}

.users-docks-wrapper {
  display: flex;
  gap: 8px;
  align-items: center;
  cursor: pointer;
}

.presence-dock {
  background-color: var(--bg-color-modal);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  padding: 6px 12px;
  display: flex;
  align-items: center;
  box-shadow: var(--shadow-lg);
  cursor: pointer;
  opacity: 0.85;
  transition: all 0.3s cubic-bezier(0.2, 0.8, 0.2, 1);
}
.presence-dock:hover {
  opacity: 1;
  transform: translateY(-2px);
  box-shadow: var(--shadow-glass);
}

.presence-dock.local-dock {
  border-color: var(--success-color);
}

.activity-dock {
  background-color: var(--bg-color-modal);
  border: 1px solid var(--border-color);
  color: var(--text-color-3);
  padding: 6px 12px;
  font-size: 0.8rem;
  border-radius: var(--radius-md);
  cursor: default;
  opacity: 0.85;
  transition: all 0.3s cubic-bezier(0.2, 0.8, 0.2, 1);
}
.activity-dock:hover {
  opacity: 1;
  transform: translateY(-2px);
  box-shadow: var(--shadow-glass);
}

.activity-text {
  margin-right: 8px;
}

.activity-avatar {
  width: 24px;
  height: 24px;
  font-size: 0.75rem;
  background: var(--bg-color-hover);
  color: var(--text-color);
  border: 1px solid var(--border-color);
}

.dock-avatar {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: var(--primary-color);
  color: white;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.75rem;
  font-weight: 600;
  border: 2px solid var(--bg-color-modal);
  margin-left: -6px;
  position: relative;
  transition: all 0.2s;
}
.dock-avatar:hover {
  z-index: 10;
  transform: scale(1.1);
}
.dock-avatar:first-child {
  margin-left: 0;
}
.overflow-avatar {
  background: var(--bg-color-hover);
  color: var(--text-color);
}
.dock-empty {
  padding: 0 12px;
  font-size: 0.9rem;
  color: var(--text-color-3);
}

.presence-expanded-list {
  background: var(--bg-color-modal);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg);
  width: 250px;
  max-height: 400px;
  overflow-y: auto;
  animation: slideUp 0.2s ease-out;
}

.expanded-header {
  padding: 12px 16px;
  font-size: 0.85rem;
  font-weight: 600;
  color: var(--text-color-2);
  border-bottom: 1px solid var(--border-color);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.expanded-user-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  cursor: pointer;
  transition: background 0.2s;
}
.expanded-user-item:hover {
  background: var(--bg-color-hover);
}

.user-avatar-small {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  background: var(--primary-color);
  color: white;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.9rem;
  font-weight: 600;
  flex-shrink: 0;
  transition: all 0.2s;
}

.dock-avatar.is-here {
  border-color: var(--success-color);
  z-index: 2;
}

.dock-avatar.is-offline {
  opacity: 0.4;
  filter: grayscale(100%);
  border-color: var(--border-color);
}

.expanded-user-item.is-offline {
  opacity: 0.6;
}

.offline-badge {
  background: #888;
  color: white;
  font-size: 0.65rem;
  padding: 2px 6px;
  border-radius: 12px;
  font-weight: 600;
  margin-left: 4px;
}

.user-info-small {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.user-name-small {
  font-weight: 500;
  font-size: 0.95rem;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.user-loc-small {
  font-size: 0.8rem;
  color: var(--text-color-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  display: flex;
  align-items: center;
  gap: 6px;
}

.same-loc-badge {
  background: var(--success-color);
  color: white;
  font-size: 0.65rem;
  padding: 2px 6px;
  border-radius: 12px;
  font-weight: 600;
}

.empty-list {
  padding: 24px;
  text-align: center;
  color: var(--text-color-3);
  font-size: 0.9rem;
}

@keyframes slideUp {
  from { opacity: 0; transform: translateY(10px); }
  to { opacity: 1; transform: translateY(0); }
}

.mention-dock {
  border-color: var(--primary-color);
}

.mention-avatar {
  background: var(--primary-color);
  color: white;
  border-color: var(--bg-color-modal);
}

.close-mention-btn {
  background: none;
  border: none;
  color: var(--text-color-3);
  font-size: 1.2rem;
  line-height: 1;
  padding: 0 4px;
  cursor: pointer;
}
.close-mention-btn:hover {
  color: var(--text-color-1);
}
</style>