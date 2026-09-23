<script setup lang="ts">
import type { WorkspaceMember, User } from '../types'

const props = defineProps<{
  workspaceMembers: WorkspaceMember[]
  activeWorkspaceRole: string | null
  currentUser: User | undefined
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'change-role', memberId: string, role: string): void
  (e: 'kick-member', memberId: string): void
}>()

const getInitials = (name: string) => name ? name.charAt(0).toUpperCase() : '?'
</script>

<template>
  <div class="modal-overlay" @click.self="emit('close')">
    <div class="modal settings-modal">
      <h2>Paramètres de l'espace</h2>
      
      <div class="members-list">
        <div v-for="m in workspaceMembers" :key="m.user_id" class="member-row">
          <div class="member-avatar">{{ getInitials(m.name) }}</div>
          
          <div class="member-info">
            <div class="member-name">{{ m.name }}</div>
            <div class="member-email">{{ m.email }}</div>
          </div>
          
          <div class="member-role" v-if="activeWorkspaceRole === 'owner'">
            <select 
              class="modern-select" 
              :value="m.role" 
              @change="(e) => emit('change-role', m.user_id, (e.target as HTMLSelectElement).value)"
            >
              <option value="owner">Owner</option>
              <option value="admin">Admin</option>
              <option value="member">Membre</option>
            </select>
          </div>
          <div class="member-role-text" v-else>
            {{ m.role.charAt(0).toUpperCase() + m.role.slice(1) }}
          </div>
          
          <button 
            v-if="activeWorkspaceRole === 'owner' && currentUser?.id !== m.user_id" 
            @click="emit('kick-member', m.user_id)" 
            class="kick-btn" 
            title="Exclure"
          >
            ❌
          </button>
        </div>
      </div>
      
      <div class="modal-actions" style="margin-top: 20px">
        <button type="button" class="primary-btn" @click="emit('close')">Fermer</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.7); display: flex; align-items: center; justify-content: center; z-index: 100; backdrop-filter: blur(4px); }
.modal { width: 100%; max-width: 420px; padding: 1.25rem; border-radius: 8px; background: var(--bg-color-modal); border: 1px solid var(--border-color); box-shadow: 0 20px 60px rgba(0,0,0,0.6); }
.settings-modal { max-width: 500px; width: 100%; }
.modal h2 { margin: 0 0 0.75rem 0; font-size: 1rem; font-weight: 500; }
.members-list { display: flex; flex-direction: column; gap: 8px; margin-top: 12px; max-height: 400px; overflow-y: auto; }
.member-row { display: flex; align-items: center; gap: 10px; padding: 8px; background: rgba(255,255,255,0.02); border: 1px solid var(--border-color); border-radius: 6px; }
.member-avatar { width: 30px; height: 30px; border-radius: 50%; background: linear-gradient(135deg, #5e6ad2, #7c3aed); color: white; display: flex; align-items: center; justify-content: center; font-weight: bold; font-size: 0.8rem; }
.member-info { flex: 1; display: flex; flex-direction: column; }
.member-name { font-weight: 500; font-size: 0.85rem; color: #eee; }
.member-email { font-size: 0.7rem; color: var(--text-color-3); }
.modern-select { background: #111; color: #eee; border: 1px solid var(--border-color); padding: 4px 8px; border-radius: 6px; outline: none; cursor: pointer; font-family: inherit; font-size: 0.85rem; }
.member-role-text { font-size: 0.8rem; color: var(--text-color-3); font-weight: 500; }
.kick-btn { background: none; border: none; cursor: pointer; opacity: 0.4; transition: 0.15s; }
.kick-btn:hover { opacity: 1; }
.modal-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 1rem; }
.primary-btn { padding: 6px 14px; border-radius: 6px; border: none; cursor: pointer; background: var(--primary-color); color: #fff; font-weight: 500; font-size: 0.85rem; transition: all 0.12s; }
.primary-btn:hover { background: var(--primary-color-hover); }
</style>
