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

    <div v-if="attachments.length > 0" style="margin-top: 16px;">
      <!-- Images Grid -->
      <div v-if="imageAttachments.length > 0" class="image-gallery">
        <div v-for="att in imageAttachments" :key="att.id" class="gallery-item" @click="openPreview(att)">
          <img :src="`/uploads/${att.id}.${getFileExtension(att.filename)}`" class="gallery-img" />
          <div class="gallery-overlay">
            <a :href="`/uploads/${att.id}.${getFileExtension(att.filename)}`" download class="icon-action" title="Télécharger" @click.stop>⬇️</a>
            <button class="icon-action delete-action" @click.stop="deleteAttachment(att.id)" title="Supprimer">🗑</button>
          </div>
        </div>
      </div>

      <!-- Other Files List -->
      <div v-if="otherAttachments.length > 0" class="attachments-list" :style="{ marginTop: imageAttachments.length > 0 ? '16px' : '0' }">
        <div v-for="att in otherAttachments" :key="att.id" class="attachment-item">
          <div class="attachment-info" style="display: flex; align-items: center; gap: 12px; overflow: hidden;">
            <div class="attachment-icon">📄</div>
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

    <!-- Image Viewer Modal -->
    <Teleport to="body">
      <div v-if="previewImageUrl" class="image-viewer-overlay" @click="closePreview">
        <div class="image-viewer-content" @click.stop>
          <button class="close-viewer-btn" @click="closePreview">✖</button>
          <img :src="previewImageUrl" class="preview-image" />
        </div>
      </div>
    </Teleport>
  </div>
</template>

<script setup>
import { ref, onMounted, computed } from 'vue'

const props = defineProps({
  taskId: {
    type: String,
    required: true
  }
})

const attachments = ref([])
const confirmAction = (msg) => window.confirm(msg)
const isDragging = ref(false)
const isUploading = ref(false)
const fileInput = ref(null)
const previewImageUrl = ref(null)

const imageAttachments = computed(() => attachments.value.filter(a => isImage(a.filename)))
const otherAttachments = computed(() => attachments.value.filter(a => !isImage(a.filename)))

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
  if (!confirmAction("Voulez-vous vraiment supprimer cette pièce jointe ?")) return
  try {
    await $fetch(`/api/boards/tasks/${props.taskId}/attachments/${attachmentId}`, { method: 'DELETE' })
    attachments.value = attachments.value.filter(a => a.id !== attachmentId)
    emit('upload-success') // Reuse emit to notify parent if needed
  } catch (e) {
    console.error(e)
    alert("Erreur lors de la suppression")
  }
}

function openPreview(att) {
  if (isImage(att.filename)) {
    previewImageUrl.value = `/uploads/${att.id}.${getFileExtension(att.filename)}`
  }
}

function closePreview() {
  previewImageUrl.value = null
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
.image-gallery {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: 12px;
  margin-bottom: 16px;
}
.gallery-item {
  position: relative;
  aspect-ratio: 1;
  border-radius: 8px;
  overflow: hidden;
  cursor: pointer;
  background: #1e1e22;
  border: 1px solid #3f3f46;
}
.gallery-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  transition: transform 0.2s;
}
.gallery-item:hover .gallery-img {
  transform: scale(1.05);
}
.gallery-overlay {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  opacity: 0;
  transition: opacity 0.2s;
}
.gallery-item:hover .gallery-overlay {
  opacity: 1;
}
.icon-action {
  background: #3f3f46;
  border: none;
  border-radius: 50%;
  width: 36px;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: white;
  text-decoration: none;
  cursor: pointer;
  font-size: 1.1rem;
}
.icon-action:hover {
  background: #4f46e5;
}
.delete-action:hover {
  background: #f87171;
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
.image-viewer-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background: rgba(0, 0, 0, 0.85);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 10000;
}
.image-viewer-content {
  position: relative;
  max-width: 90vw;
  max-height: 90vh;
}
.preview-image {
  max-width: 100%;
  max-height: 90vh;
  object-fit: contain;
  border-radius: 8px;
  box-shadow: 0 10px 25px rgba(0, 0, 0, 0.5);
}
.close-viewer-btn {
  position: absolute;
  top: -40px;
  right: -40px;
  background: none;
  border: none;
  color: #fff;
  font-size: 2rem;
  cursor: pointer;
}
.close-viewer-btn:hover {
  color: #f87171;
}
</style>
