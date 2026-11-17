<script setup lang="ts">
import { Plus, Search, MoreHorizontal, Pencil, Trash2, MessageSquare } from 'lucide-vue-next'
import { useI18n } from '~/composables/useI18n'
import type { MenuItem } from '~/components/ui/Menu.vue'
import type { AiConversation } from '~/composables/useAi'

/** Vos conversations (privées), groupées par date ; la recherche porte sur les titres et les messages. */
const props = defineProps<{ conversations: AiConversation[]; activeId: string | null; loading?: boolean }>()
const query = defineModel<string>('query', { default: '' })
const emit = defineEmits<{ select: [id: string]; new: []; rename: [AiConversation]; remove: [AiConversation] }>()
const { t } = useI18n()

const groups = computed(() => {
  const day = 86_400_000
  const startOfToday = new Date().setHours(0, 0, 0, 0)
  const buckets: { key: string; label: string; items: AiConversation[] }[] = [
    { key: 'today', label: t('ai.list.today'), items: [] },
    { key: 'week', label: t('ai.list.week'), items: [] },
    { key: 'older', label: t('ai.list.older'), items: [] },
  ]
  for (const c of props.conversations) {
    const at = +new Date(c.updated_at)
    buckets[at >= startOfToday ? 0 : at >= startOfToday - 6 * day ? 1 : 2]!.items.push(c)
  }
  return buckets.filter((b) => b.items.length)
})

const openMenu = ref<string | null>(null)
const menuFor = (c: AiConversation): MenuItem[] => [
  { label: t('common.rename'), icon: Pencil, onSelect: () => emit('rename', c) },
  { label: t('common.delete'), icon: Trash2, danger: true, onSelect: () => emit('remove', c) },
]
</script>

<template>
  <nav class="cl" :aria-label="t('ai.list.title')">
    <div class="cl-head">
      <UiButton variant="primary" size="sm" :icon="Plus" block @click="emit('new')">{{ t('ai.chat.new') }}</UiButton>
      <UiInput v-model="query" :icon="Search" size="sm" :placeholder="t('ai.list.search')" :aria-label="t('ai.list.search')" />
    </div>
    <div class="cl-scroll">
      <div v-if="loading && !conversations.length" class="cl-center"><UiSpinner /></div>
      <p v-else-if="!conversations.length" class="cl-empty">{{ query.trim() ? t('ai.list.noMatch') : t('ai.list.empty') }}</p>
      <section v-for="g in groups" :key="g.key" class="cl-group">
        <h3 class="cl-title">{{ g.label }}</h3>
        <div v-for="c in g.items" :key="c.id" class="cl-row" :class="{ on: c.id === activeId }" @contextmenu.prevent="openMenu = c.id">
          <button class="cl-main" :aria-current="c.id === activeId ? 'true' : undefined" @click="emit('select', c.id)">
            <MessageSquare :size="13" class="cl-icon" />
            <span class="cl-name truncate">{{ c.title || t('ai.chat.untitled') }}</span>
          </button>
          <UiMenu :open="openMenu === c.id" :items="menuFor(c)" placement="bottom-end" @update:open="(v: boolean) => (openMenu = v ? c.id : null)">
            <button class="cl-more" :aria-label="t('common.more')"><MoreHorizontal :size="14" /></button>
          </UiMenu>
        </div>
      </section>
    </div>
  </nav>
</template>

<style scoped>
.cl { display: flex; flex-direction: column; min-height: 0; height: 100%; border-right: 1px solid var(--border); }
.cl-head { display: flex; flex-direction: column; gap: 8px; padding: 12px; }
.cl-scroll { flex: 1; overflow-y: auto; padding: 0 8px 12px; }
.cl-center { display: flex; justify-content: center; padding: 16px; }
.cl-empty { color: var(--text-3); font-size: var(--fs-sm); padding: 8px; margin: 0; }
.cl-group { margin-bottom: 10px; }
.cl-title { margin: 8px 8px 4px; font-size: var(--fs-xs); font-weight: 600; color: var(--text-4); text-transform: uppercase; letter-spacing: 0.04em; }
.cl-row { display: flex; align-items: center; border-radius: var(--r-sm); }
.cl-row:hover, .cl-row.on { background: var(--bg-hover); }
.cl-row.on { color: var(--text); }
.cl-main { flex: 1; min-width: 0; display: flex; align-items: center; gap: 8px; padding: 6px 8px; text-align: left; color: var(--text-2); font-size: var(--fs-sm); }
.cl-row.on .cl-main { color: var(--text); font-weight: 500; }
.cl-icon { flex: none; color: var(--text-4); }
.cl-more { display: grid; place-items: center; width: 24px; height: 24px; margin-right: 4px; border-radius: var(--r-sm); color: var(--text-3); opacity: 0; }
.cl-row:hover .cl-more, .cl-row.on .cl-more, .cl-more:focus-visible { opacity: 1; }
.cl-more:hover { background: var(--bg-active, var(--bg-hover)); color: var(--text); }
</style>
