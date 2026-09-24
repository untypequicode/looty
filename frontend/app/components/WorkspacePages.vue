<template>
  <div class="page-editor-wrapper">
    <div class="page-editor-inner">
      <div class="page-header">
        <input 
          v-model="localTitle"
          @input="onTitleChange"
          type="text" 
          class="page-title-input" 
          placeholder="Titre de la page"
          :readonly="isReadonly || activeWorkspaceRole === 'viewer'"
        />
        <Teleport to="#wiki-header-actions" v-if="isMounted">
          <div class="page-meta">
            <span v-if="page.created_at">Créée {{ formatRelativeDate(page.created_at) }}</span>
            <span v-else>Créée à l'instant</span>
            <span class="meta-dot" v-if="activeWorkspaceRole !== 'viewer'">•</span>
            <button v-if="activeWorkspaceRole !== 'viewer'" class="meta-btn" @click="isReadonly = !isReadonly">
              {{ isReadonly ? '🔒 Lecture' : '✏️ Édition' }}
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
            <span class="meta-dot" v-if="collaborators.length > 0">•</span>
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
import { useWorkspace } from '../composables/useWorkspace'

const { activeWorkspaceRole } = useWorkspace()

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

const localTitle = ref(props.page.title)
const localContent = ref(props.page.content)
const isReadonly = ref(false)
const saveStatus = ref('')
const saveStatusClass = ref('')
const isMounted = ref(false)
const collaborators = ref([])
let saveTimeout = null

onMounted(() => {
  isMounted.value = true
})

watch(() => props.page.id, () => {
  localTitle.value = props.page.title
  localContent.value = props.page.content
})

function onTitleChange() {
  emit('update-title', localTitle.value)
  debouncedSavePage()
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
  max-width: 900px;
  display: flex;
  flex-direction: column;
}

.page-header {
  margin-bottom: 2rem;
}

.page-title-input {
  background: transparent;
  border: none;
  font-size: 3rem;
  font-weight: 700;
  color: #fff;
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

.page-meta {
  display: flex;
  align-items: center;
  gap: 12px;
  color: #8a8f98;
  font-size: 0.85rem;
}

.meta-dot {
  color: #3f3f46;
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
  background: #27282b;
  border: 1px solid #3f3f46;
  color: #e4e4e7;
  padding: 4px 8px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 0.8rem;
  font-weight: 500;
  transition: all 0.2s;
}
.meta-btn:hover {
  background: #3f3f46;
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
  border: 2px solid #0e0e11;
  margin-left: -6px;
  z-index: 1;
}
.collaborator-avatar:first-child {
  margin-left: 0;
}

.save-status {
  font-size: 0.85rem;
  color: #8a8f98;
  transition: color 0.2s;
}
.save-status.saving { color: #ed8936; }
.save-status.saved { color: #48bb78; }
.save-status.error { color: #f56565; font-weight: bold; }
</style>
