<template>
  <div class="attachments-section" style="margin-top: 2rem;">
    <h3 style="margin: 0 0 12px 0; font-size: 1rem; font-weight: 500;">Pièces jointes</h3>
    
    <div class="drop-zone" 
         @dragover.prevent="isDragging = true" 
         @dragleave.prevent="isDragging = false" 
         @drop.prevent="onDrop"
         :class="{ 'is-dragging': isDragging }">
      <p v-if="!isUploading">Glissez-déposez un fichier ici ou cliquez pour parcourir.</p>
      <p v-else>Upload en cours...</p>
      <input type="file" @change="onFileSelect" style="display: none;" ref="fileInput">
      <button v-if="!isUploading" type="button" class="secondary-btn" @click="$refs.fileInput.click()">Sélectionner un fichier</button>
    </div>

    <div v-if="attachments.length > 0" class="attachments-list" style="margin-top: 16px;">
      <div v-for="att in attachments" :key="att.id" class="attachment-item">
        <div class="attachment-info" style="display: flex; align-items: center; gap: 12px; overflow: hidden;">
          <img v-if="isImage(att.filename)" :src="`/uploads/${att.id}.${getFileExtension(att.filename)}`" class="attachment-thumb" />
          <div v-else class="attachment-icon">📄</div>
          <div style="display: flex; flex-direction: column; overflow: hidden;">
             <a :href="`/uploads/${att.id}.${getFileExtension(att.filename)}`" target="_blank" class="attachment-link">
               {{ att.filename }}
             </a>
             <span class="attachment-size">{{ formatBytes(att.size_bytes) }}</span>
          </div>
        </div>
        <div class="attachment-actions" style="display: flex; gap: 8px;">
          <a :href="`/uploads/${att.id}.${getFileExtension(att.filename)}`" download class="secondary-btn small-btn" style="text-decoration: none;" title="Télécharger">⬇️</a>
          <button class="secondary-btn small-btn" @click="deleteAttachment(att.id)" style="color: #f87171;" title="Supprimer">🗑</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'

const props = defineProps({
  taskId: {
    type: String,
    required: true
  }
})

const attachments = ref([])
const isDragging = ref(false)
const isUploading = ref(false)
const fileInput = ref(null)

const emit = defineEmits(['upload-success'])

async function loadAttachments() {
  try {
    const res = await $fetch(`/api/boards/tasks/${props.taskId}/attachments`)
    attachments.value = res
  } catch (e) {
    console.error(e)
  }
}

onMounted(() => {
  loadAttachments()
})

async function onDrop(e) {
  isDragging.value = false
  if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
    await uploadFile(e.dataTransfer.files[0])
  }
}

async function onFileSelect(e) {
  if (e.target.files && e.target.files.length > 0) {
    await uploadFile(e.target.files[0])
  }
}

async function uploadFile(file) {
  isUploading.value = true
  const formData = new FormData()
  formData.append('file', file)
  
  try {
    const newAtt = await $fetch(`/api/boards/tasks/${props.taskId}/attachments`, {
      method: 'POST',
      body: formData
    })
    attachments.value.unshift(newAtt)
    emit('upload-success')
  } catch (e) {
    console.error(e)
    alert("Erreur lors de l'upload du fichier")
  } finally {
    isUploading.value = false
    if (fileInput.value) fileInput.value.value = ''
  }
}

function getFileExtension(filename) {
  return filename.split('.').pop() || ''
}

function formatBytes(bytes) {
  if (!bytes) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i]
}

function isImage(filename) {
  const ext = getFileExtension(filename).toLowerCase()
  return ['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg'].includes(ext)
}

async function deleteAttachment(attachmentId) {
  if (!confirm("Voulez-vous vraiment supprimer cette pièce jointe ?")) return
  try {
    await $fetch(`/api/boards/tasks/${props.taskId}/attachments/${attachmentId}`, { method: 'DELETE' })
    attachments.value = attachments.value.filter(a => a.id !== attachmentId)
    emit('upload-success') // Reuse emit to notify parent if needed
  } catch (e) {
    console.error(e)
    alert("Erreur lors de la suppression")
  }
}
</script>

<style scoped>
.drop-zone {
  border: 2px dashed #3f3f46;
  border-radius: 8px;
  padding: 24px;
  text-align: center;
  background: #1e1e22;
  color: #a1a1aa;
  transition: all 0.2s;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
}
.drop-zone.is-dragging {
  border-color: #4f46e5;
  background: #2a2a35;
}
.attachments-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.attachment-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  background: #2f3136;
  border-radius: 6px;
}
.attachment-link {
  color: #60a5fa;
  text-decoration: none;
  font-size: 0.9rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 80%;
}
.attachment-link:hover {
  text-decoration: underline;
}
.attachment-size {
  color: #8a8f98;
  font-size: 0.8rem;
}
.attachment-thumb {
  width: 40px;
  height: 40px;
  border-radius: 4px;
  object-fit: cover;
  border: 1px solid #3f3f46;
}
.attachment-icon {
  width: 40px;
  height: 40px;
  border-radius: 4px;
  background: #3f3f46;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 1.2rem;
}
</style>
