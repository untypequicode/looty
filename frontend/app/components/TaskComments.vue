<template>
  <div class="comments-section">
    <h3 class="section-heading">
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none" style="opacity: 0.5;">
        <path d="M2 3a1 1 0 011-1h10a1 1 0 011 1v7a1 1 0 01-1 1H5l-3 3V3z" stroke="currentColor" stroke-width="1.5" fill="none"/>
      </svg>
      Commentaires
      <span class="comment-count" v-if="comments.length > 0">{{ comments.length }}</span>
    </h3>
    
    <div class="comments-list" ref="commentsListRef">
      <div v-for="comment in comments" :key="comment.id" class="comment-item">
        <div class="comment-header">
          <div class="comment-avatar">{{ getUserInitial(comment.user_id) }}</div>
          <div class="comment-meta">
            <span class="comment-user">{{ getUserName(comment.user_id) }}</span>
            <span class="comment-time">{{ formatDate(comment.created_at) }}</span>
          </div>
          <button class="comment-delete" @click="deleteComment(comment.id)" title="Supprimer">
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none"><path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.5"/></svg>
          </button>
        </div>
        <div class="comment-body ProseMirror" style="padding: 0; min-height: 0;" v-html="comment.content"></div>
      </div>
      <div v-if="comments.length === 0" class="empty-comments">
        Aucun commentaire pour le moment.
      </div>
    </div>

    <form @submit.prevent="submitComment" class="comment-form">
      <div style="flex: 1;">
        <SimpleEditor 
          v-model="newComment" 
          placeholder-text="Écrire un commentaire..."
        />
      </div>
      <button type="submit" class="comment-submit" :disabled="!newComment || newComment === '<p></p>' || newComment === '<p><br></p>'">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none"><path d="M2 8l12-6-4 6 4 6L2 8z" fill="currentColor"/></svg>
      </button>
    </form>
  </div>
</template>

<script setup>
import { ref, onMounted, nextTick } from 'vue'

const props = defineProps({
  taskId: { type: String, required: true },
  workspaceMembers: { type: Array, default: () => [] }
})

const comments = ref([])
const newComment = ref('')
const commentsListRef = ref(null)

async function loadComments() {
  try {
    const res = await $fetch(`/api/boards/tasks/${props.taskId}/comments`)
    comments.value = res
  } catch (e) { console.error(e) }
}

onMounted(() => { loadComments() })

async function submitComment() {
  const content = (newComment.value || '').trim()
  if (!content || content === '<p></p>' || content === '<p><br></p>') return
  try {
    const comment = await $fetch(`/api/boards/tasks/${props.taskId}/comments`, {
      method: 'POST',
      body: { content: content }
    })
    comments.value.push(comment)
    newComment.value = ''
    await nextTick()
    if (commentsListRef.value) {
      commentsListRef.value.scrollTop = commentsListRef.value.scrollHeight
    }
  } catch (e) { console.error(e) }
}

async function deleteComment(commentId) {
  try {
    await $fetch(`/api/boards/tasks/${props.taskId}/comments/${commentId}`, { method: 'DELETE' })
    comments.value = comments.value.filter(c => c.id !== commentId)
  } catch (e) { console.error(e) }
}

function getUserName(userId) {
  const member = props.workspaceMembers.find(m => m.user_id === userId)
  return member ? member.name : 'Utilisateur'
}

function getUserInitial(userId) {
  return getUserName(userId).charAt(0).toUpperCase()
}

function formatDate(dateStr) {
  if (!dateStr) return ''
  const d = new Date(dateStr)
  const now = new Date()
  const diff = now - d
  if (diff < 60000) return "À l'instant"
  if (diff < 3600000) return `Il y a ${Math.floor(diff / 60000)} min`
  if (diff < 86400000) return `Il y a ${Math.floor(diff / 3600000)}h`
  return d.toLocaleDateString() + ' ' + d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}
</script>

<style scoped>
.comments-section {
  margin-top: 2rem;
}
.section-heading {
  margin: 0 0 12px 0;
  font-size: 0.8rem;
  font-weight: 500;
  color: var(--text-color-3);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  display: flex;
  align-items: center;
  gap: 6px;
}
.comment-count {
  background: var(--bg-color-modal);
  color: var(--text-color-3);
  padding: 1px 6px;
  border-radius: 10px;
  font-size: 0.7rem;
}
.comments-list {
  max-height: 300px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-bottom: 12px;
}
.comment-item {
  padding: 10px 12px;
  background: var(--bg-color-2);
  border: 1px solid var(--border-color);
  border-radius: 6px;
}
.comment-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}
.comment-avatar {
  width: 22px;
  height: 22px;
  border-radius: 50%;
  background: var(--primary-color);
  color: white;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.7rem;
  font-weight: 600;
  flex-shrink: 0;
}
.comment-meta {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 8px;
}
.comment-user {
  font-weight: 500;
  font-size: 0.85rem;
  color: var(--text-color);
}
.comment-time {
  font-size: 0.75rem;
  color: var(--text-color-3);
}
.comment-delete {
  background: none;
  border: none;
  color: var(--text-color-3);
  cursor: pointer;
  padding: 2px;
  border-radius: 4px;
  opacity: 0;
  transition: opacity 0.2s;
}
.comment-item:hover .comment-delete {
  opacity: 1;
}
.comment-delete:hover {
  color: var(--danger-color);
  background: rgba(248, 113, 113, 0.1);
}
.comment-body {
  font-size: 0.9rem;
  color: var(--text-color-2);
  line-height: 1.5;
  white-space: pre-wrap;
}
.empty-comments {
  color: var(--text-color-3);
  font-size: 0.85rem;
  font-style: italic;
  padding: 8px 0;
}
.comment-form {
  display: flex;
  gap: 8px;
  align-items: flex-end;
}
.comment-input {
  flex: 1;
  padding: 8px 12px;
  border-radius: 6px;
  background: var(--bg-color-2);
  border: 1px solid var(--border-color);
  color: var(--text-color);
  font-family: inherit;
  font-size: 0.875rem;
  resize: none;
  outline: none;
  transition: border-color 0.2s;
}
.comment-input:focus {
  border-color: var(--primary-color);
}
.comment-submit {
  background: var(--primary-color);
  border: none;
  color: white;
  padding: 8px 12px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.2s;
  display: flex;
  align-items: center;
}
.comment-submit:hover {
  background: var(--primary-color-hover);
}
.comment-submit:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}
</style>
