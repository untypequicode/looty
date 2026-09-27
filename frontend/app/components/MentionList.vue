<template>
  <div class="mention-list">
    <template v-if="items.length">
      <button
        v-for="(item, index) in items"
        :key="index"
        :class="{ 'is-selected': index === selectedIndex }"
        @click="selectItem(index)"
        class="mention-item"
      >
        <span class="mention-name">{{ item.name }}</span>
      </button>
    </template>
    <div v-else class="mention-empty">
      Aucun membre trouvé
    </div>
  </div>
</template>

<script>
export default {
  props: {
    items: {
      type: Array,
      required: true,
    },
    command: {
      type: Function,
      required: true,
    },
  },

  data() {
    return {
      selectedIndex: 0,
    }
  },

  watch: {
    items() {
      this.selectedIndex = 0
    },
  },

  methods: {
    onKeyDown({ event }) {
      if (event.key === 'ArrowUp') {
        this.upHandler()
        return true
      }

      if (event.key === 'ArrowDown') {
        this.downHandler()
        return true
      }

      if (event.key === 'Enter') {
        this.enterHandler()
        return true
      }

      return false
    },

    upHandler() {
      this.selectedIndex = ((this.selectedIndex + this.items.length) - 1) % this.items.length
    },

    downHandler() {
      this.selectedIndex = (this.selectedIndex + 1) % this.items.length
    },

    enterHandler() {
      this.selectItem(this.selectedIndex)
    },

    selectItem(index) {
      const item = this.items[index]
      if (item) {
        this.command({ id: item.name })
      }
    },
  },
}
</script>

<style scoped>
.mention-list {
  background: var(--bg-color-modal);
  border-radius: 6px;
  border: 1px solid var(--border-color);
  box-shadow: 0 4px 6px rgba(0, 0, 0, 0.3);
  padding: 4px;
  display: flex;
  flex-direction: column;
  min-width: 150px;
}

.mention-item {
  background: transparent;
  border: none;
  text-align: left;
  padding: 6px 10px;
  color: var(--text-color);
  border-radius: 4px;
  cursor: pointer;
  display: flex;
  align-items: center;
}

.mention-item:hover,
.mention-item.is-selected {
  background: var(--bg-color-hover);
}

.mention-empty {
  padding: 6px 10px;
  color: var(--text-color-3);
  font-size: 0.9rem;
}
</style>
