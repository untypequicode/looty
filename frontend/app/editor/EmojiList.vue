<template>
  <div class="command-list" v-if="items.length">
    <div
      v-for="(item, index) in items"
      :key="index"
      class="command-item"
      :class="{ 'is-selected': index === selectedIndex }"
      @click="selectItem(index)"
    >
      <div class="item-icon">{{ item.emoji }}</div>
      <div class="item-content">
        <div class="item-title">{{ item.title }}</div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, watch, onMounted } from 'vue'

const props = defineProps({
  items: {
    type: Array,
    required: true,
  },
  command: {
    type: Function,
    required: true,
  },
})

const selectedIndex = ref(0)

watch(() => props.items, () => {
  selectedIndex.value = 0
})

const selectItem = (index) => {
  const item = props.items[index]
  if (item) {
    props.command(item)
  }
}

const onKeyDown = ({ event }) => {
  if (event.key === 'ArrowUp') {
    selectedIndex.value = ((selectedIndex.value + props.items.length) - 1) % props.items.length
    return true
  }

  if (event.key === 'ArrowDown') {
    selectedIndex.value = (selectedIndex.value + 1) % props.items.length
    return true
  }

  if (event.key === 'Enter') {
    selectItem(selectedIndex.value)
    return true
  }

  return false
}

defineExpose({ onKeyDown })
</script>

<style scoped>
.command-list {
  background: var(--bg-color-modal);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg);
  padding: 4px;
  max-height: 300px;
  overflow-y: auto;
  min-width: 150px;
}

.command-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 6px 8px;
  cursor: pointer;
  border-radius: var(--radius-md);
  color: var(--text-color-2);
  transition: all 0.2s;
}

.command-item:hover,
.command-item.is-selected {
  background: var(--bg-color-hover);
  color: var(--text-color);
}

.item-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  font-size: 1.2rem;
}

.item-content {
  flex: 1;
  display: flex;
  flex-direction: column;
}

.item-title {
  font-size: 0.9rem;
  font-weight: 500;
  color: var(--text-color);
}
</style>
