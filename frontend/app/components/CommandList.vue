<template>
  <div class="items">
    <template v-if="items.length">
      <button
        class="item"
        :class="{ 'is-selected': index === selectedIndex }"
        v-for="(item, index) in items"
        :key="index"
        @click="selectItem(index)"
      >
        <span class="icon">{{ item.icon }}</span>
        <div class="content">
          <div class="title">{{ item.title }}</div>
          <div class="subtitle">{{ item.subtitle }}</div>
        </div>
      </button>
    </template>
    <div class="item" v-else>
      <div class="content">
        <div class="title">Aucun résultat</div>
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

function onKeyDown({ event }) {
  if (event.key === 'ArrowUp') {
    upHandler()
    return true
  }

  if (event.key === 'ArrowDown') {
    downHandler()
    return true
  }

  if (event.key === 'Enter') {
    enterHandler()
    return true
  }

  return false
}

function upHandler() {
  selectedIndex.value = ((selectedIndex.value + props.items.length) - 1) % props.items.length
}

function downHandler() {
  selectedIndex.value = (selectedIndex.value + 1) % props.items.length
}

function enterHandler() {
  selectItem(selectedIndex.value)
}

function selectItem(index) {
  const item = props.items[index]
  if (item) {
    props.command(item)
  }
}

defineExpose({
  onKeyDown,
})
</script>

<style scoped>
.items {
  padding: 8px;
  background: #18191c;
  border-radius: 8px;
  box-shadow: 0 8px 16px rgba(0, 0, 0, 0.4);
  border: 1px solid #3f3f46;
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 300px;
  overflow-y: auto;
  min-width: 250px;
}

.item {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  text-align: left;
  background: transparent;
  border: none;
  padding: 8px 12px;
  border-radius: 6px;
  cursor: pointer;
  color: #eee;
  transition: background 0.1s ease;
}

.item:hover,
.item.is-selected {
  background: #27282b;
}

.icon {
  font-size: 1.2rem;
  width: 24px;
  text-align: center;
  color: #8a8f98;
}

.content {
  display: flex;
  flex-direction: column;
}

.title {
  font-weight: 500;
  font-size: 0.95rem;
}

.subtitle {
  font-size: 0.75rem;
  color: #8a8f98;
}
</style>
