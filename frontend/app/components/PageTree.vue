<template>
  <ul class="workspace-list">
    <li v-for="page in currentLevelPages" :key="page.id">
      <div
        :class="['workspace-item', { 
          active: activePage?.id === page.id && currentModule === 'wiki',
          'drag-over-before': dropTarget?.id === page.id && dropTarget?.position === 'before',
          'drag-over-after': dropTarget?.id === page.id && dropTarget?.position === 'after',
          'drag-over-inside': dropTarget?.id === page.id && dropTarget?.position === 'inside'
        }]"
        :style="{ paddingLeft: (level * 12 + 10) + 'px' }"
        draggable="true"
        @dragstart="onDragStart($event, page)"
        @dragover.prevent="onDragOver($event, page)"
        @dragleave="onDragLeave($event, page)"
        @drop.prevent="onDrop($event, page)"
        @click="handleSelectPage(page)"
      >
        <button 
          v-if="hasChildren(page.id)" 
          class="collapse-btn" 
          @click.stop="toggleCollapse(page.id)"
        >
          <svg v-if="isCollapsed(page.id)" xmlns="http://www.w3.org/2000/svg" width="12" height="12" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7"/></svg>
          <svg v-else xmlns="http://www.w3.org/2000/svg" width="12" height="12" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7"/></svg>
        </button>
        <span v-else class="collapse-placeholder"></span>

        <span class="ws-name" style="flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; display: flex; align-items: center; gap: 6px;">
          <span v-if="page.icon" style="font-size: 1rem;">{{ page.icon }}</span>
          <svg v-else xmlns="http://www.w3.org/2000/svg" width="14" height="14" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" class="item-icon"><path stroke-linecap="round" stroke-linejoin="round" d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z"/></svg>
          {{ page.title || 'Nouvelle page' }}
        </span>
        <div class="item-actions" v-if="activePage?.id === page.id">
          <button @click.stop="handleCreateSubpage(page.id)" title="Créer une sous-page">
            <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4v16m8-8H4"/></svg>
          </button>
          <button @click.stop="$emit('delete-page', page.id)" title="Supprimer">
            <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/></svg>
          </button>
        </div>
      </div>
      
      <PageTree
        v-if="hasChildren(page.id) && !isCollapsed(page.id)"
        :pages="pages"
        :parent-id="page.id"
        :level="level + 1"
        :active-page="activePage"
        :current-module="currentModule"
        @select-page="$emit('select-page', $event)"
        @delete-page="$emit('delete-page', $event)"
        @create-subpage="$emit('create-subpage', $event)"
        @move-page="$emit('move-page', $event)"
      />
    </li>
  </ul>
</template>

<script setup>
import { computed, ref } from 'vue'

const props = defineProps({
  pages: { type: Array, required: true },
  parentId: { type: String, default: null },
  rootIds: { type: Array, default: null },
  level: { type: Number, default: 0 },
  activePage: { type: Object, default: null },
  currentModule: { type: String, default: '' },
})

const emit = defineEmits(['select-page', 'delete-page', 'create-subpage', 'move-page'])

const collapsedState = ref({})
const dropTarget = ref(null)

const currentLevelPages = computed(() => {
  if (props.level === 0 && props.rootIds) {
    return props.pages.filter(p => props.rootIds.includes(p.id)).sort((a, b) => (a.position || 0) - (b.position || 0))
  }
  return props.pages.filter(p => p.parent_id === props.parentId).sort((a, b) => (a.position || 0) - (b.position || 0))
})

function handleSelectPage(page) {
  collapsedState.value[page.id] = false
  emit('select-page', page)
}

function hasChildren(pageId) {
  return props.pages.some(p => p.parent_id === pageId)
}

function isCollapsed(pageId) {
  return collapsedState.value[pageId] !== false // Default collapsed if undefined
}

function toggleCollapse(pageId) {
  collapsedState.value[pageId] = !isCollapsed(pageId)
}

function handleCreateSubpage(pageId) {
  collapsedState.value[pageId] = false
  emit('create-subpage', pageId)
}

function onDragStart(event, page) {
  event.dataTransfer.setData('text/plain', JSON.stringify(page))
  event.dataTransfer.effectAllowed = 'move'
}

function onDragOver(event, page) {
  const rect = event.currentTarget.getBoundingClientRect()
  const y = event.clientY - rect.top
  const height = rect.height
  let position = 'inside'
  
  if (y < height * 0.25) position = 'before'
  else if (y > height * 0.75) position = 'after'
  
  dropTarget.value = { id: page.id, position }
}

function onDragLeave(event, page) {
  if (dropTarget.value?.id === page.id) {
    dropTarget.value = null
  }
}

function onDrop(event, targetPage) {
  const draggedPageStr = event.dataTransfer.getData('text/plain')
  if (!draggedPageStr) return
  
  try {
    const draggedPage = JSON.parse(draggedPageStr)
    
    // Prevent dragging into itself or its descendants
    let current = targetPage
    while (current) {
      if (current.id === draggedPage.id) {
        dropTarget.value = null
        return
      }
      current = props.pages.find(p => p.id === current.parent_id)
    }

    let parentId = targetPage.parent_id
    let newPosition = targetPage.position || 0

    if (dropTarget.value?.position === 'inside') {
      parentId = targetPage.id
      const children = props.pages.filter(p => p.parent_id === targetPage.id)
      const lastChild = children.sort((a, b) => (b.position || 0) - (a.position || 0))[0]
      newPosition = lastChild ? (lastChild.position || 0) + 1000 : 1000
      collapsedState.value[targetPage.id] = false // Auto unfold
    } else if (dropTarget.value?.position === 'before') {
      const siblings = props.pages.filter(p => p.parent_id === targetPage.parent_id).sort((a, b) => (a.position || 0) - (b.position || 0))
      const idx = siblings.findIndex(p => p.id === targetPage.id)
      const prev = idx > 0 ? siblings[idx - 1] : null
      const prevPos = prev ? (prev.position || 0) : ((targetPage.position || 0) - 1000)
      newPosition = prev ? (prevPos + (targetPage.position || 0)) / 2 : prevPos
    } else if (dropTarget.value?.position === 'after') {
      const siblings = props.pages.filter(p => p.parent_id === targetPage.parent_id).sort((a, b) => (a.position || 0) - (b.position || 0))
      const idx = siblings.findIndex(p => p.id === targetPage.id)
      const next = idx < siblings.length - 1 ? siblings[idx + 1] : null
      const nextPos = next ? (next.position || 0) : ((targetPage.position || 0) + 1000)
      newPosition = next ? (nextPos + (targetPage.position || 0)) / 2 : nextPos
    }

    emit('move-page', { pageId: draggedPage.id, parentId, position: newPosition })
  } catch (err) {
    console.error(err)
  }
  
  dropTarget.value = null
}
</script>

<style scoped>
.workspace-list { 
  list-style: none; 
  padding: 0; 
  margin: 0; 
  display: flex; 
  flex-direction: column; 
  gap: 2px; 
}
.workspace-item { 
  display: flex; 
  align-items: center; 
  gap: 6px; 
  padding: 6px 10px; 
  border-radius: var(--radius-sm); 
  cursor: pointer; 
  color: var(--text-color-2); 
  font-size: 0.875rem; 
  font-weight: 500; 
  transition: all var(--transition-fast); 
  position: relative;
  user-select: none;
}
.workspace-item:hover { 
  background: var(--bg-color-hover); 
  color: var(--text-color); 
}
.workspace-item.active { 
  background: var(--bg-color-hover); 
  color: var(--text-color); 
  font-weight: 600;
}

/* Elegant Drag & Drop Lines */
.workspace-item.drag-over-before {
  box-shadow: inset 0 2px 0 var(--primary-color);
}
.workspace-item.drag-over-after {
  box-shadow: inset 0 -2px 0 var(--primary-color);
}
.workspace-item.drag-over-inside {
  background: var(--bg-color-hover);
  box-shadow: inset 2px 0 0 var(--primary-color);
}

.collapse-btn {
  background: none;
  border: none;
  color: var(--text-color-3);
  cursor: pointer;
  padding: 2px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
}
.collapse-btn:hover {
  background: var(--border-color);
  color: var(--text-color);
}
.collapse-placeholder {
  width: 16px;
  height: 16px;
}
.item-actions { 
  display: flex; 
  align-items: center; 
  gap: 4px; 
  opacity: 0; 
  transition: opacity var(--transition-fast); 
}
.workspace-item:hover .item-actions,
.workspace-item.active .item-actions { 
  opacity: 1; 
}
.item-actions button { 
  background: none; 
  border: none; 
  color: var(--text-color-3); 
  cursor: pointer; 
  padding: 4px; 
  border-radius: 4px; 
  display: flex; 
  align-items: center; 
  justify-content: center; 
}
.item-actions button:hover { 
  background: var(--border-color); 
  color: var(--text-color); 
}
</style>
