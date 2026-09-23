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
        />
        <div class="page-meta">
          <span v-if="page.created_at">Vous avez créé cette page {{ formatRelativeDate(page.created_at) }}</span>
          <span v-else>Créée il y a moins d'une minute</span>
          <span>•</span>
          <span class="meta-action">Commenter</span>
          <span>•</span>
          <span>Consulté par vous</span>
        </div>
      </div>
      
      <div class="page-content">
        <WysiwygEditor v-model="localContent" @update:modelValue="debouncedSavePage" />
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, watch } from 'vue'
import WysiwygEditor from './WysiwygEditor.vue'

const props = defineProps({
  workspaceId: {
    type: String,
    required: true
  },
  page: {
    type: Object,
    required: true
  }
})

const emit = defineEmits(['update-title'])

const localTitle = ref(props.page.title)
const localContent = ref(props.page.content)
let saveTimeout = null

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
  try {
    await fetch(`/api/workspaces/${props.workspaceId}/pages/${props.page.id}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ 
        title: localTitle.value, 
        content: localContent.value,
        parent_id: props.page.parent_id
      })
    })
  } catch(e) {
    console.error('Erreur sauvegarde:', e)
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
  gap: 8px;
  color: #8a8f98;
  font-size: 0.85rem;
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
</style>
