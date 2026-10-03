<script setup lang="ts">
import { Search, Clock, Smile, Users, PawPrint, Apple, Plane, Trophy, Lightbulb, Hash, Flag, Sparkles } from 'lucide-vue-next'
import type { Component } from 'vue'
import { loadEmojis, searchEmojis, recentEmojis, pushRecentEmoji, EMOJI_GROUPS, type EmojiItem } from '~/utils/emojiData'
import { useI18n } from '~/composables/useI18n'

const open = defineModel<boolean>('open', { default: false })
const emit = defineEmits<{ (e: 'select', emoji: string | null): void }>()
const { t, locale } = useI18n()
const q = ref('')
const input = ref<HTMLInputElement | null>(null)
const scroller = ref<HTMLElement | null>(null)
const all = shallowRef<EmojiItem[]>([])
const recent = ref<string[]>([])
const active = ref<string>('suggested')

const SUGGESTED = ['📄', '📝', '📚', '🚀', '💡', '🎯', '🛠️', '📊', '🧭', '🗂️', '✅', '🔒', '🎨', '🧪', '🐛', '📦', '🌍', '⚡', '🔥', '🧠', '📅', '💬', '🏗️', '🦀', '📌', '🧩', '🔬', '🎓', '💼', '🏠', '❤️', '⭐']
const GROUP_ICONS: Record<number, Component> = { 0: Smile, 1: Users, 3: PawPrint, 4: Apple, 5: Plane, 6: Trophy, 7: Lightbulb, 8: Hash, 9: Flag }

watch(open, async (v) => {
  if (!v) return
  q.value = ''
  recent.value = recentEmojis()
  active.value = recent.value.length ? 'recent' : 'suggested'
  nextTick(() => input.value?.focus())
  if (!all.value.length) all.value = await loadEmojis()
})

const label = (e: EmojiItem) => (locale.value === 'fr' ? e.fr : e.en)
const results = computed(() => (q.value.trim() ? searchEmojis(all.value, q.value) : []))
const sections = computed(() => [
  ...(recent.value.length ? [{ key: 'recent', title: t('editor.emojiRecent'), icon: Clock, items: recent.value.map((emoji) => ({ emoji, title: emoji })) }] : []),
  { key: 'suggested', title: t('editor.suggested'), icon: Sparkles, items: SUGGESTED.map((emoji) => ({ emoji, title: emoji })) },
  ...EMOJI_GROUPS.map((g) => ({
    key: `g${g}`,
    title: t(`editor.emojiGroups.${g}`),
    icon: GROUP_ICONS[g]!,
    items: all.value.filter((e) => e.group === g).map((e) => ({ emoji: e.emoji, title: label(e) })),
  })),
])

function jump(key: string) {
  active.value = key
  scroller.value?.querySelector<HTMLElement>(`[data-section="${key}"]`)?.scrollIntoView({ block: 'start' })
}
function onScroll() {
  const el = scroller.value
  if (!el) return
  const top = el.scrollTop + 8
  let current = sections.value[0]?.key ?? ''
  for (const s of el.querySelectorAll<HTMLElement>('[data-section]')) if (s.offsetTop <= top) current = s.dataset.section!
  active.value = current
}
function pick(e: string | null) {
  if (e) pushRecentEmoji(e)
  emit('select', e)
  open.value = false
}
</script>

<template>
  <UiPopover v-model:open="open" :width="356">
    <template #trigger><slot /></template>
    <div class="ip">
      <div class="ip-search">
        <Search :size="14" class="faint" />
        <input ref="input" v-model="q" :placeholder="$t('editor.searchEmoji')" @keydown.enter.prevent="results[0] && pick(results[0].emoji)" />
        <button class="ip-remove" @click="pick(null)">{{ $t('common.remove') }}</button>
      </div>
      <div v-if="!q" class="ip-tabs">
        <button v-for="s in sections" :key="s.key" :class="{ on: active === s.key }" :title="s.title" @click="jump(s.key)"><component :is="s.icon" :size="15" /></button>
      </div>
      <div ref="scroller" class="ip-scroll" @scroll.passive="onScroll">
        <template v-if="q">
          <div class="ip-grid">
            <button v-for="e in results" :key="e.emoji" :title="label(e)" @click="pick(e.emoji)">{{ e.emoji }}</button>
          </div>
          <div v-if="!results.length" class="ip-empty">{{ all.length ? $t('editor.emojiNone') : $t('common.loading') }}</div>
        </template>
        <template v-else>
          <section v-for="s in sections" :key="s.key" :data-section="s.key">
            <div class="ip-section">{{ s.title }}</div>
            <div class="ip-grid">
              <button v-for="e in s.items" :key="`${s.key}${e.emoji}`" :title="e.title" @click="pick(e.emoji)">{{ e.emoji }}</button>
            </div>
          </section>
          <div v-if="!all.length" class="ip-empty">{{ $t('common.loading') }}</div>
        </template>
      </div>
    </div>
  </UiPopover>
</template>

<style scoped>
.ip { padding: 8px; display: flex; flex-direction: column; gap: 6px; }
.ip-search { display: flex; align-items: center; gap: 8px; height: 32px; padding: 0 4px 0 8px; border-radius: var(--r-sm); background: var(--bg-input); box-shadow: 0 0 0 1px var(--border); }
.ip-search input { flex: 1; border: none; outline: none; background: transparent; min-width: 0; }
.ip-remove { font-size: var(--fs-sm); color: var(--text-3); padding: 2px 6px; border-radius: var(--r-xs); }
.ip-remove:hover { background: var(--bg-hover); color: var(--red); }
.ip-tabs { display: flex; justify-content: space-between; border-bottom: 1px solid var(--border); padding-bottom: 4px; }
.ip-tabs button { width: 30px; height: 26px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-3); }
.ip-tabs button:hover { background: var(--bg-hover); color: var(--text-2); }
.ip-tabs button.on { color: var(--accent); background: var(--accent-soft); }
.ip-scroll { height: 280px; overflow-y: auto; overscroll-behavior: contain; position: relative; }
.ip-section { font-size: var(--fs-xs); color: var(--text-3); margin: 8px 2px 4px; font-weight: 500; position: sticky; top: 0; background: var(--bg-elevated); padding: 2px 0; z-index: 1; }
.ip-grid { display: grid; grid-template-columns: repeat(9, 1fr); gap: 2px; }
.ip-grid button { height: 34px; font-size: 21px; border-radius: var(--r-sm); line-height: 1; }
.ip-grid button:hover { background: var(--bg-hover); }
.ip-empty { padding: 24px; text-align: center; color: var(--text-3); font-size: var(--fs-sm); }
</style>
