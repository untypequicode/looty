<template>
  <div class="items" ref="itemsContainer">
    <template v-if="items.length">
      <button
        class="item"
        :class="{ 'is-selected': index === selectedIndex }"
        v-for="(item, index) in items"
        :key="index"
        :ref="el => { if (el) itemRefs[index] = el }"
        @click="selectItem(index)"
      >
        <div class="item-left">
          <span class="icon">{{ item.icon }}</span>
          <span class="title">{{ item.title }}</span>
        </div>
        <span class="shortcut" v-if="item.shortcut">{{ item.shortcut }}</span>
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
import { ref, watch, nextTick } from 'vue'

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
const itemsContainer = ref(null)
const itemRefs = ref([])

watch(() => props.items, () => {
  selectedIndex.value = 0
  itemRefs.value = []
})

const scrollToSelected = async () => {
  await nextTick()
  const el = itemRefs.value[selectedIndex.value]
  if (el && itemsContainer.value) {
    const container = itemsContainer.value
    if (el.offsetTop < container.scrollTop) {
      container.scrollTop = el.offsetTop
    } else if (el.offsetTop + el.offsetHeight > container.scrollTop + container.offsetHeight) {
      container.scrollTop = el.offsetTop + el.offsetHeight - container.offsetHeight
    }
  }
}

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
  scrollToSelected()
}

function downHandler() {
  selectedIndex.value = (selectedIndex.value + 1) % props.items.length
  scrollToSelected()
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
  padding: 6px;
  background: var(--bg-color-modal);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-xl);
  border: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-height: 320px;
  overflow-y: auto;
  min-width: 280px;
}

.item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  background: transparent;
  border: none;
  padding: 8px 10px;
  border-radius: var(--radius-md);
  cursor: pointer;
  color: var(--text-color);
  transition: background 0.1s ease;
}

.item:hover,
.item.is-selected {
  background: var(--bg-color-hover);
  color: var(--text-color);
}
.item:hover .icon, .item.is-selected .icon,
.item:hover .shortcut, .item.is-selected .shortcut {
  color: var(--text-color);
}

.item-left {
  display: flex;
  align-items: center;
  gap: 12px;
}

.icon {
  font-size: 1.05rem;
  width: 22px;
  text-align: center;
  color: var(--text-color-3);
  font-weight: bold;
  display: flex;
  align-items: center;
  justify-content: center;
}

.title {
  font-weight: 500;
  font-size: 0.95rem;
}

.shortcut {
  font-size: 0.8rem;
  color: var(--text-color-3);
  font-family: monospace;
}
</style>
