<template>
  <div class="emoji-picker" ref="pickerRef">
    <div class="emoji-search">
      <input 
        ref="searchInput"
        v-model="query" 
        type="text" 
        placeholder="Rechercher un émoji..." 
        @keydown="onKeyDown"
      />
    </div>
    <div class="command-list" v-if="filteredEmojis.length">
      <div
        v-for="(item, index) in filteredEmojis"
        :key="index"
        class="command-item"
        :class="{ 'is-selected': index === selectedIndex }"
        @click="selectItem(index)"
        @mouseenter="selectedIndex = index"
      >
        <div class="item-icon">{{ item.emoji }}</div>
        <div class="item-content">
          <div class="item-title">{{ item.title }}</div>
        </div>
      </div>
    </div>
    <div v-else class="no-results">Aucun résultat</div>
  </div>
</template>

<script setup>
import { onMounted, onUnmounted, ref, computed, nextTick } from 'vue'

const emit = defineEmits(['select', 'close'])

// The exact same list from EmojiExtension.js
const allEmojis = [
  { title: 'smile', emoji: '😄' },
  { title: 'joy', emoji: '😂' },
  { title: 'heart', emoji: '❤️' },
  { title: 'thumbsup', emoji: '👍' },
  { title: 'fire', emoji: '🔥' },
  { title: 'star', emoji: '⭐' },
  { title: 'rocket', emoji: '🚀' },
  { title: 'party', emoji: '🥳' },
  { title: 'check', emoji: '✅' },
  { title: 'cross', emoji: '❌' },
  { title: 'warning', emoji: '⚠️' },
  { title: 'info', emoji: 'ℹ️' },
  { title: 'idea', emoji: '💡' },
  { title: 'pin', emoji: '📌' },
  { title: 'eyes', emoji: '👀' },
  { title: 'sob', emoji: '😭' },
  { title: 'cry', emoji: '😢' },
  { title: 'sweat_smile', emoji: '😅' },
  { title: 'wink', emoji: '😉' },
  { title: 'sunglasses', emoji: '😎' },
  { title: 'thinking', emoji: '🤔' },
  { title: 'alien', emoji: '👽' },
  { title: 'ghost', emoji: '👻' },
  { title: 'robot', emoji: '🤖' },
  { title: 'cat', emoji: '🐱' },
  { title: 'dog', emoji: '🐶' },
  { title: 'apple', emoji: '🍎' },
  { title: 'pizza', emoji: '🍕' },
  { title: 'coffee', emoji: '☕' },
  { title: 'car', emoji: '🚗' },
  { title: 'house', emoji: '🏠' },
  { title: 'computer', emoji: '💻' },
  { title: 'book', emoji: '📖' },
  { title: 'music', emoji: '🎵' },
  { title: 'art', emoji: '🎨' },
  { title: 'earth', emoji: '🌍' },
  { title: 'sun', emoji: '☀️' },
  { title: 'moon', emoji: '🌙' },
  { title: 'rainbow', emoji: '🌈' },
  { title: 'zap', emoji: '⚡' },
  { title: 'snow', emoji: '❄️' },
  { title: 'fireworks', emoji: '🎆' },
  { title: 'balloon', emoji: '🎈' },
  { title: 'gift', emoji: '🎁' },
  { title: 'medal', emoji: '🏅' },
  { title: 'lock', emoji: '🔒' },
  { title: 'key', emoji: '🔑' },
  { title: 'mag', emoji: '🔍' },
  { title: 'loudspeaker', emoji: '📢' },
  { title: 'bell', emoji: '🔔' },
  { title: 'pushpin', emoji: '📌' },
  { title: 'chart', emoji: '📈' },
  { title: 'calendar', emoji: '📅' },
]

const pickerRef = ref(null)
const searchInput = ref(null)
const query = ref('')
const selectedIndex = ref(0)

const filteredEmojis = computed(() => {
  if (!query.value) return allEmojis
  return allEmojis.filter(item => item.title.toLowerCase().includes(query.value.toLowerCase()))
})

const handleClickOutside = (event) => {
  if (pickerRef.value && !pickerRef.value.contains(event.target)) {
    emit('close')
  }
}

onMounted(() => {
  document.addEventListener('click', handleClickOutside)
  nextTick(() => {
    searchInput.value?.focus()
  })
})

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutside)
})

const selectItem = (index) => {
  const item = filteredEmojis.value[index]
  if (item) {
    emit('select', item.emoji)
  }
}

const onKeyDown = (event) => {
  if (event.key === 'ArrowUp') {
    event.preventDefault()
    selectedIndex.value = ((selectedIndex.value + filteredEmojis.value.length) - 1) % filteredEmojis.value.length
    return
  }

  if (event.key === 'ArrowDown') {
    event.preventDefault()
    selectedIndex.value = (selectedIndex.value + 1) % filteredEmojis.value.length
    return
  }

  if (event.key === 'Enter') {
    event.preventDefault()
    selectItem(selectedIndex.value)
    return
  }
}
</script>

<style scoped>
.emoji-picker {
  background: var(--bg-color-modal);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg);
  width: 250px;
  z-index: 100;
  display: flex;
  flex-direction: column;
}

.emoji-search {
  padding: 8px;
  border-bottom: 1px solid var(--border-color);
}

.emoji-search input {
  width: 100%;
  background: transparent;
  border: none;
  color: var(--text-color);
  outline: none;
  font-size: 0.9rem;
}

.command-list {
  padding: 4px;
  max-height: 250px;
  overflow-y: auto;
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

.no-results {
  padding: 16px;
  text-align: center;
  color: var(--text-color-3);
  font-size: 0.9rem;
}
</style>
