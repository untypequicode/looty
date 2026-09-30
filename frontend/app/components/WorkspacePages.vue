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
          <button @click="handleToggleFavorite" class="favorite-btn" :class="{ 'is-active': page.is_favorite }" title="Ajouter aux favoris">
            <svg v-if="page.is_favorite" xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="2"><path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"/></svg>
            <svg v-else xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"/></svg>
          </button>
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
            <button v-if="activeWorkspaceRole !== 'viewer'" class="meta-btn action-btn-alpha" @click="toggleShareModal = true" style="display: flex; align-items: center; gap: 4px;">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1"/></svg>
              Publier
            </button>
            <button v-if="activeWorkspaceRole === 'admin' || activeWorkspaceRole === 'owner'" class="meta-btn perm-btn" @click="openPermissionsModal" style="display: flex; align-items: center; gap: 4px; margin-left: 8px;">
              🔐 Accès
            </button>
            <span class="meta-dot">•</span>

            <div class="recent-activity-block" v-if="page.updated_at && page.updated_by && !collaborators.some(c => c.id === page.updated_by)">
              <div class="activity-item" :title="`Dernière modification par ${getMemberName(page.updated_by)}`">
                 <div class="collaborator-avatar offline">
                    {{ getMemberName(page.updated_by).charAt(0).toUpperCase() }}
                 </div>
                 <span class="activity-text">Modifié il y a {{ formatRelativeDate(page.updated_at) }}</span>
              </div>
            </div>


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
        
        <!-- Subpages section -->
        <div v-if="subpages.length > 0 || activeWorkspaceRole !== 'viewer'" class="subpages-section">
          <h3 class="subpages-title">Documents</h3>
          <div class="subpages-grid">
            <div v-for="subpage in subpages" :key="subpage.id" class="subpage-card" @click="navigateToSubpage(subpage.id)">
              <div class="subpage-icon">{{ subpage.icon || '📄' }}</div>
              <div class="subpage-details">
                <div class="subpage-title">{{ subpage.title || 'Sans titre' }}</div>
                <div class="subpage-meta">
                  <span v-if="subpage.updated_at">Mis à jour {{ formatRelativeDate(subpage.updated_at) }}</span>
                  <span v-else>Créé récemment</span>
                </div>
              </div>
            </div>
            <div v-if="activeWorkspaceRole !== 'viewer'" class="subpage-card new-subpage-card" @click="createNewSubpage">
              <div class="subpage-icon" style="color: var(--text-color-3)">+</div>
              <div class="subpage-details"><div class="subpage-title" style="color: var(--text-color-3)">Nouveau doc</div></div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Share Modal -->
    <ClientOnly>
      <Teleport to="body">
        <div v-if="toggleShareModal" class="modal-overlay" @click.self="toggleShareModal = false">
          <div class="modal-content">
            <h2>Lien public la page</h2>
            <div v-if="page.is_public && page.share_token" class="share-info">
              <p>Cette page est accessible publiquement via ce lien :</p>
              <div style="display: flex; gap: 8px; margin-top: 12px; margin-bottom: 24px;">
                <input type="text" readonly :value="shareLink" class="modern-input" style="flex: 1;" />
                <button @click="copyShareLink" class="btn btn-primary">Copier</button>
              </div>
              <button @click="handleRevokeShare" class="btn btn-danger" :disabled="isSharing">Révoquer le lien</button>
            </div>
            <div v-else class="share-info">
              <p>Créer un lien public pour partager cette page avec des personnes externes.</p>
              <button @click="handleSharePage" class="btn btn-primary" style="margin-top: 16px;" :disabled="isSharing">Générer un lien</button>
            </div>
            <div class="modal-actions" style="margin-top: 24px; text-align: right;">
              <button @click="toggleShareModal = false" class="btn btn-secondary">Fermer</button>
            </div>
          </div>
        </div>
      </Teleport>
    </ClientOnly>

    <!-- Permissions Modal -->
    <ClientOnly>
      <Teleport to="body">
        <div v-if="togglePermissionsModal" class="modal-overlay" @click.self="togglePermissionsModal = false">
          <div class="modal-content">
            <h2>Accès à la page</h2>
            <p>Gérez les permissions spécifiques à cette page pour les membres de l'espace.</p>
            <div class="members-list" style="margin-top: 16px;">
              <div v-for="member in workspaceMembers" :key="member.user_id" style="display: flex; justify-content: space-between; align-items: center; padding: 8px 0; border-bottom: 1px solid var(--border-color);">
                <span>{{ member.name }} <small>({{ member.email }})</small></span>
                <select v-model="pagePermissionsMap[member.user_id]" @change="updatePagePerm(member.user_id, $event.target.value)" class="modern-select" style="width: auto;">
                  <option value="none">Hérité ({{ member.role }})</option>
                  <option value="viewer">Lecteur</option>
                  <option value="editor">Éditeur</option>
                  <option value="blocked">Bloqué (Aucun accès)</option>
                </select>
              </div>
            </div>
            <div class="modal-actions" style="margin-top: 24px; text-align: right;">
              <button @click="togglePermissionsModal = false" class="btn btn-secondary">Fermer</button>
            </div>
          </div>
        </div>
      </Teleport>
    </ClientOnly>
  </div>
</template>

<script setup>
import { ref, watch, onMounted, computed } from 'vue'
import WysiwygEditor from './WysiwygEditor.client.vue'
import EmojiPicker from './EmojiPicker.vue'
import { useWorkspace } from '../composables/useWorkspace'

const { activeWorkspaceRole, workspaceMembers, sharePage, revokeShare, toggleFavorite, recordView, pages, createPage } = useWorkspace()

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

const toggleShareModal = ref(false)
const isSharing = ref(false)
const togglePermissionsModal = ref(false)
const pagePermissionsMap = ref({})

const openPermissionsModal = async () => {
  togglePermissionsModal.value = true
  try {
    const res = await $fetch(`/api/workspaces/${props.workspaceId}/pages/${props.page.id}/permissions`)
    const map = {}
    for (const p of res) {
      map[p.user_id] = p.role
    }
    for (const m of workspaceMembers.value) {
      if (!map[m.user_id]) map[m.user_id] = 'none'
    }
    pagePermissionsMap.value = map
  } catch (e) {
    console.error(e)
  }
}

const updatePagePerm = async (userId, role) => {
  try {
    await $fetch(`/api/workspaces/${props.workspaceId}/pages/${props.page.id}/permissions`, {
      method: 'PUT',
      body: { user_id: userId, role }
    })
  } catch (e) {
    console.error(e)
    alert("Erreur lors de la mise à jour de la permission")
  }
}

const shareLink = computed(() => {
  if (!props.page.share_token) return ''
  return typeof window !== 'undefined' ? `${window.location.origin}/public/${props.page.share_token}` : ''
})

const handleSharePage = async () => {
  try {
    isSharing.value = true
    const updatedPage = await sharePage({ workspaceId: props.workspaceId, pageId: props.page.id })
    Object.assign(props.page, updatedPage)
  } catch(e) {
    console.error(e)
  } finally {
    isSharing.value = false
  }
}

const handleRevokeShare = async () => {
  try {
    isSharing.value = true
    const updatedPage = await revokeShare({ workspaceId: props.workspaceId, pageId: props.page.id })
    Object.assign(props.page, updatedPage)
  } catch(e) {
    console.error(e)
  } finally {
    isSharing.value = false
  }
}

const copyShareLink = () => {
  if (shareLink.value) {
    navigator.clipboard.writeText(shareLink.value)
    alert('Lien copié !')
  }
}

const handleToggleFavorite = async () => {
  try {
    const current = props.page.is_favorite
    props.page.is_favorite = !current
    await toggleFavorite({ workspaceId: props.workspaceId, pageId: props.page.id })
  } catch (e) {
    props.page.is_favorite = !props.page.is_favorite
    console.error(e)
  }
}

const recordPageView = async () => {
  if (props.page?.id) {
    try {
      await recordView({ workspaceId: props.workspaceId, pageId: props.page.id })
    } catch(e) { console.error(e) }
  }
}

const subpages = computed(() => {
  return (pages.value || []).filter(p => p.parent_id === props.page.id).sort((a, b) => a.position - b.position)
})

const navigateToSubpage = (id) => {
  window.location.hash = `#wiki-${id}`
}

const createNewSubpage = async () => {
  try {
    const newPage = await createPage({ workspaceId: props.workspaceId, title: '', parentId: props.page.id })
    navigateToSubpage(newPage.id)
  } catch(e) {
    console.error(e)
  }
}

onMounted(() => {
  isMounted.value = true
  recordPageView()
})

watch(() => props.page.id, () => {
  localTitle.value = props.page.title || ''
  localIcon.value = props.page.icon || ''
  localContent.value = props.page.content || ''
  showEmojiPicker.value = false
  recordPageView()
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

.favorite-btn {
  background: transparent;
  border: none;
  cursor: pointer;
  color: var(--text-color-3);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 8px;
  border-radius: 8px;
  transition: all 0.2s ease;
  margin-top: -12px;
}
.favorite-btn:hover {
  color: var(--text-color);
  background: var(--bg-color-2);
}
.favorite-btn.is-active {
  color: #f59e0b;
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

/* Modals */
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0,0,0,0.7);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
  backdrop-filter: blur(4px);
}
.modal-content {
  width: 100%;
  max-width: 450px;
  margin: 0 1rem;
  padding: 1.5rem;
  border-radius: 8px;
  background: var(--bg-color-modal);
  border: 1px solid var(--border-color);
  box-shadow: 0 20px 60px rgba(0,0,0,0.6);
}
.modal-content h2 {
  margin: 0 0 1rem 0;
  font-size: 1.1rem;
  font-weight: 500;
}
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
