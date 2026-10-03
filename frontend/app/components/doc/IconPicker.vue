<script setup lang="ts">
import { allEmojis } from '~/utils/emojis'
import { Search } from 'lucide-vue-next'

const open = defineModel<boolean>('open', { default: false })
const emit = defineEmits<{ (e: 'select', emoji: string | null): void }>()
const q = ref('')
const input = ref<HTMLInputElement | null>(null)
const SUGGESTED = ['📄', '📝', '📚', '🚀', '💡', '🎯', '🛠️', '📊', '🧭', '🗂️', '✅', '🔒', '🎨', '🧪', '🐛', '📦', '🌍', '⚡', '🔥', '🧠', '📅', '💬', '🏗️', '🦀']
const list = computed(() => {
  const s = q.value.trim().toLowerCase()
  const all = allEmojis as { title: string; emoji: string }[]
  return s ? all.filter((e) => e.title.includes(s)).slice(0, 160) : all.slice(0, 160)
})
watch(open, (v) => { if (v) { q.value = ''; nextTick(() => input.value?.focus()) } })
function pick(e: string | null) {
  emit('select', e)
  open.value = false
}
</script>

<template>
  <UiPopover v-model:open="open" :width="320">
    <template #trigger><slot /></template>
    <div class="ip">
      <div class="ip-search">
        <Search :size="14" class="faint" />
        <input ref="input" v-model="q" :placeholder="$t('editor.searchEmoji')" />
        <button class="ip-remove" @click="pick(null)">{{ $t('common.remove') }}</button>
      </div>
      <div v-if="!q" class="ip-section">{{ $t('editor.suggested') }}</div>
      <div v-if="!q" class="ip-grid">
        <button v-for="e in SUGGESTED" :key="`s${e}`" @click="pick(e)">{{ e }}</button>
      </div>
      <div class="ip-section">{{ $t('editor.allEmojis') }}</div>
      <div class="ip-grid scroll">
        <button v-for="e in list" :key="e.title" :title="e.title" @click="pick(e.emoji)">{{ e.emoji }}</button>
      </div>
    </div>
  </UiPopover>
</template>

<style scoped>
.ip { padding: 8px; }
.ip-search { display: flex; align-items: center; gap: 8px; height: 32px; padding: 0 4px 0 8px; border-radius: var(--r-sm); background: var(--bg-input); box-shadow: 0 0 0 1px var(--border); }
.ip-search input { flex: 1; border: none; outline: none; background: transparent; min-width: 0; }
.ip-remove { font-size: var(--fs-sm); color: var(--text-3); padding: 2px 6px; border-radius: var(--r-xs); }
.ip-remove:hover { background: var(--bg-hover); color: var(--red); }
.ip-section { font-size: var(--fs-xs); color: var(--text-3); margin: 10px 2px 4px; font-weight: 500; }
.ip-grid { display: grid; grid-template-columns: repeat(8, 1fr); gap: 2px; }
.ip-grid.scroll { max-height: 200px; overflow-y: auto; }
.ip-grid button { height: 34px; font-size: 20px; border-radius: var(--r-sm); line-height: 1; }
.ip-grid button:hover { background: var(--bg-hover); }
</style>
