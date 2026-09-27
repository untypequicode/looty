<template>
  <div class="page-editor-wrapper">
    <div class="page-editor-inner">
      <div class="page-header">
        <div class="page-title-row">
          <div class="page-icon-wrapper" v-if="activeWorkspaceRole !== 'viewer'">
            <div class="page-icon" :class="{ 'is-empty': !localIcon }" @click="showEmojiPicker = !showEmojiPicker">
              {{ localIcon || '😀' }}
            </div>
            <EmojiPicker v-if="showEmojiPicker" @select="onIconSelect" class="emoji-picker-dropdown" />
          </div>
          <div v-else-if="localIcon" class="page-icon readonly">{{ localIcon }}</div>
          <div v-else class="page-icon readonly is-empty">😀</div>

          <input 
            v-model="localTitle"
            @input="onTitleChange"
            type="text" 
            class="page-title-input" 
            placeholder="Titre de la page"
            :readonly="isReadonly || activeWorkspaceRole === 'viewer'"
          />
        </div>
        <Teleport to="#wiki-header-actions" v-if="isMounted">
          <div class="page-meta">
            <span v-if="page.updated_at && page.updated_by" class="meta-updated">
              Dernière modification par {{ getMemberName(page.updated_by) }} il y a {{ formatRelativeDate(page.updated_at) }}
            </span>
            <span v-else-if="page.created_at">Créée {{ formatRelativeDate(page.created_at) }}</span>
            <span v-else>Créée à l'instant</span>
            <span class="meta-dot" v-if="activeWorkspaceRole !== 'viewer'">•</span>
            <button v-if="activeWorkspaceRole !== 'viewer'" class="meta-btn" @click="isReadonly = !isReadonly" style="display: flex; align-items: center; gap: 4px;">
              <svg v-if="isReadonly" xmlns="http://www.w3.org/2000/svg" width="12" height="12" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z"/></svg>
              <svg v-else xmlns="http://www.w3.org/2000/svg" width="12" height="12" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z"/></svg>
              {{ isReadonly ? 'Lecture' : 'Édition' }}
            </button>
            <span class="meta-dot">•</span>
            <div class="collaborators-list" v-if="collaborators.length > 0">
              <div 
                v-for="user in collaborators" 
                :key="user.name"
                class="collaborator-avatar"
                :style="{ backgroundColor: user.color }"
                :title="user.name"
              >
                {{ user.name.charAt(0).toUpperCase() }}
              </div>
            </div>
            
            <div class="recent-activity-block" v-if="page.updated_at && page.updated_by && !collaborators.some(c => c.id === page.updated_by)">
              <div class="activity-item" :title="`Dernière modification par ${getMemberName(page.updated_by)}`">
                 <div class="collaborator-avatar offline">
                    {{ getMemberName(page.updated_by).charAt(0).toUpperCase() }}
                 </div>
                 <span class="activity-text">Modifié il y a {{ formatRelativeDate(page.updated_at) }}</span>
              </div>
            </div>

            <span class="meta-dot" v-if="collaborators.length > 0 || page.updated_by">•</span>
            <span :class="['save-status', saveStatusClass]">
              {{ saveStatus || 'Sauvegardé' }}
            </span>
          </div>
        </Teleport>
      </div>
      
      <div class="page-content">
        <WysiwygEditor 
          :key="page.id"
          v-model="localContent" 
          :document-id="page.id"
          :current-user="currentUser"
          :editable="!isReadonly && activeWorkspaceRole !== 'viewer'"
          @update:modelValue="debouncedSavePage" 
          @update:collaborators="collaborators = $event"
        />
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, watch, onMounted } from 'vue'
import WysiwygEditor from './WysiwygEditor.client.vue'
import EmojiPicker from './EmojiPicker.vue'
import { useWorkspace } from '../composables/useWorkspace'

const { activeWorkspaceRole, workspaceMembers } = useWorkspace()

const getMemberName = (id) => {
  const member = workspaceMembers.value?.find(m => m.user_id === id)
  return member ? member.name : 'Utilisateur inconnu'
}

const props = defineProps({
  workspaceId: {
    type: String,
    required: true
  },
  page: {
    type: Object,
    required: true
  },
  currentUser: {
    type: Object,
    required: false
  }
})

const emit = defineEmits(['update-title'])

const localTitle = ref(props.page.title || '')
const localIcon = ref(props.page.icon || '')
const localContent = ref(props.page.content || '')
const isReadonly = ref(false)
const showEmojiPicker = ref(false)
const saveStatus = ref('')
const saveStatusClass = ref('')
const isMounted = ref(false)
const collaborators = ref([])
let saveTimeout = null

onMounted(() => {
  isMounted.value = true
})

watch(() => props.page.id, () => {
  localTitle.value = props.page.title || ''
  localIcon.value = props.page.icon || ''
  localContent.value = props.page.content || ''
  showEmojiPicker.value = false
})

watch(() => props.page.title, (newTitle) => {
  if (localTitle.value !== newTitle) localTitle.value = newTitle || ''
})

watch(() => props.page.icon, (newIcon) => {
  if (localIcon.value !== newIcon) localIcon.value = newIcon || ''
})

function onTitleChange() {
  emit('update-title', localTitle.value)
  debouncedSavePage()
}

function onIconSelect(emoji) {
  localIcon.value = emoji
  showEmojiPicker.value = false
  savePage()
}

function debouncedSavePage() {
  if (saveTimeout) clearTimeout(saveTimeout)
  saveTimeout = setTimeout(() => {
    savePage()
  }, 1000)
}

async function savePage() {
  saveStatus.value = 'Enregistrement...'
  saveStatusClass.value = 'saving'
  try {
    const res = await fetch(`/api/workspaces/${props.workspaceId}/pages/${props.page.id}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ 
        title: localTitle.value, 
        icon: localIcon.value,
        content: localContent.value,
        parent_id: props.page.parent_id
      })
    })
    if (res.ok) {
      saveStatus.value = 'Enregistré'
      saveStatusClass.value = 'saved'
      setTimeout(() => {
        if (saveStatus.value === 'Enregistré') {
          saveStatus.value = ''
          saveStatusClass.value = ''
        }
      }, 2000)
    } else {
      saveStatus.value = 'Erreur'
      saveStatusClass.value = 'error'
    }
  } catch(e) {
    console.error('Erreur sauvegarde:', e)
    saveStatus.value = 'Erreur'
    saveStatusClass.value = 'error'
  }
}

function formatRelativeDate(dateStr) {
  // Simple formatting for now
  return "récemment"
}
</script>

<style scoped>
.page-editor-wrapper {
  display: flex;
  justify-content: center;
  padding: 2rem 1rem 6rem 1rem; /* Space at bottom like Notion */
  height: 100%;
}

.page-editor-inner {
  width: 100%;
  max-width: 1200px;
  display: flex;
  flex-direction: column;
}

.page-header {
  margin-bottom: 2rem;
}

.page-title-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 1rem;
}

.page-icon-wrapper {
  position: relative;
  display: flex;
  align-items: center;
}

.page-icon {
  font-size: 2.5rem;
  line-height: 1;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  user-select: none;
  border-radius: var(--radius-md);
  padding: 4px;
  transition: all 0.2s;
}
.page-icon.is-empty {
  filter: grayscale(100%);
  opacity: 0.3;
}
.page-icon:not(.readonly):hover {
  background: var(--bg-color-hover);
  opacity: 1;
  filter: grayscale(0%);
}
.page-icon.readonly {
  cursor: default;
}

.emoji-picker-dropdown {
  position: absolute;
  top: 100%;
  left: 0;
  margin-top: 8px;
  z-index: 50;
}

.page-title-input {
  background: transparent;
  border: none;
  font-size: 3rem;
  font-weight: 700;
  color: var(--text-color);
  width: 100%;
  outline: none;
  margin: 0 0 1rem 0;
  padding: 0;
  font-family: inherit;
  line-height: 1.2;
}
.page-title-input::placeholder {
  color: #3f3f46;
}

@media (max-width: 768px) {
  .page-title-input { font-size: 2rem; }
  .page-editor-wrapper { padding: 1rem 1rem 4rem 1rem; }
  .page-icon { font-size: 2rem; }
}

.page-meta {
  display: flex;
  align-items: center;
  gap: 12px;
  color: var(--text-color-3);
  font-size: 0.85rem;
}

.meta-dot {
  color: var(--text-color-3);
}

.meta-action {
  cursor: pointer;
  transition: color 0.2s;
}
.meta-action:hover {
  color: #fff;
}

.page-content {
  flex: 1;
}

/* Ensure WysiwygEditor is completely borderless inside this container */
:deep(.tiptap) {
  padding: 0 !important;
  font-size: 1.05rem;
  line-height: 1.6;
}

.meta-btn {
  background: var(--bg-color-2);
  border: 1px solid var(--border-color);
  color: var(--text-color);
  padding: 4px 8px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 0.8rem;
  font-weight: 500;
  transition: all 0.2s;
}
.meta-btn:hover {
  background: var(--bg-color-hover);
}

.collaborators-list {
  display: flex;
  align-items: center;
  gap: -4px;
}
.collaborator-avatar {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  font-size: 10px;
  font-weight: bold;
  border: 2px solid var(--bg-color-main);
  margin-left: -6px;
  z-index: 1;
}
.collaborator-avatar:first-child {
  margin-left: 0;
}
.collaborator-avatar.offline {
  background-color: var(--text-color-3);
  filter: grayscale(100%);
  opacity: 0.6;
}

.recent-activity-block {
  display: flex;
  align-items: center;
  margin-left: 8px;
}
.activity-item {
  display: flex;
  align-items: center;
  gap: 6px;
  background: var(--bg-color-modal);
  border: 1px solid var(--border-color);
  padding: 2px 8px 2px 2px;
  border-radius: 16px;
}
.activity-text {
  font-size: 0.75rem;
  color: var(--text-color-2);
}

.save-status {
  font-size: 0.85rem;
  color: var(--text-color-3);
  transition: color 0.2s;
}
.save-status.saving { color: var(--warning-color); }
.save-status.saved { color: var(--success-color); }
.save-status.error { color: var(--danger-color); font-weight: bold; }
</style>
