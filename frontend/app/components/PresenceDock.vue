<template>
  <div class="presence-dock-wrapper" :class="{ 'is-expanded': isExpanded }">
    <div class="presence-dock" @click="isExpanded = !isExpanded">
      <div v-for="user in visibleUsers" :key="user.user_id" class="dock-avatar" :title="getUserLabel(user)">
        {{ getUserInitials(user) }}
      </div>
      <div v-if="overflowCount > 0" class="dock-avatar overflow-avatar">
        +{{ overflowCount }}
      </div>
      <div v-if="users.length === 0" class="dock-empty">
        Connexion...
      </div>
    </div>
    
    <!-- Expanded List -->
    <div v-if="isExpanded" class="presence-expanded-list">
      <div class="expanded-header">Personnes en ligne ({{ users.length }})</div>
      <div v-for="user in sortedUsers" :key="user.user_id" class="expanded-user-item" @click="navigateToUser(user)">
        <div class="user-avatar-small">{{ getUserInitials(user) }}</div>
        <div class="user-info-small">
          <div class="user-name-small">{{ getUserName(user) }}</div>
          <div class="user-loc-small">
            <span v-if="user.location === currentLocation" class="same-loc-badge">Ici</span>
            {{ getLocationLabel(user.location) }}
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
}>()

const emit = defineEmits(['navigate'])

const isExpanded = ref(false)

const getUserName = (user: any) => {
  const member = props.workspaceMembers?.find(m => m.user_id === user.user_id)
  return member ? member.name : 'Inconnu'
}

const getUserInitials = (user: any) => {
  const name = getUserName(user)
  return name.charAt(0).toUpperCase()
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

const getUserLabel = (user: any) => {
  return `${getUserName(user)} - ${getLocationLabel(user.location)}`
}

const sortedUsers = computed(() => {
  return [...(props.users || [])].sort((a, b) => {
    // Current location first
    if (a.location === props.currentLocation && b.location !== props.currentLocation) return -1
    if (b.location === props.currentLocation && a.location !== props.currentLocation) return 1
    return 0
  })
})

const visibleUsers = computed(() => {
  return sortedUsers.value.slice(0, 5)
})

const overflowCount = computed(() => {
  return Math.max(0, (props.users?.length || 0) - 5)
})

const navigateToUser = (user: any) => {
  if (user.location !== props.currentLocation) {
    if (user.location.startsWith('page:')) {
      window.location.hash = `#wiki-${user.location.split(':')[1]}`
    } else if (user.location.startsWith('board:')) {
      emit('navigate', { type: 'kanban', id: user.location.split(':')[1] })
    } else {
      window.location.hash = `#dashboard`
    }
  }
  isExpanded.value = false
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

.presence-dock {
  background: var(--bg-color-2);
  border: 1px solid var(--border-color);
  border-radius: 32px;
  padding: 6px;
  display: flex;
  align-items: center;
  box-shadow: var(--shadow-md);
  cursor: pointer;
  transition: all 0.2s;
}
.presence-dock:hover {
  transform: translateY(-2px);
  box-shadow: var(--shadow-lg);
}

.dock-avatar {
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
  border: 2px solid var(--bg-color-2);
  margin-left: -8px;
  position: relative;
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
</style>