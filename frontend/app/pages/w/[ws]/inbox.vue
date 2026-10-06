<script setup lang="ts">
import { Inbox, CheckCheck, AtSign, UserPlus, MessageSquare, Circle, Bell, BellRing, CalendarClock, CircleCheck, Mail } from 'lucide-vue-next'
import type { Notification } from '~/types'
import { useNotifications } from '~/composables/useNotifications'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { motion } from '~/composables/useMotion'

definePageMeta({ layout: 'workspace' })

const { notifications, isLoading, unread, markRead, markAllRead } = useNotifications()
const { wsId, member, pages, boards } = useWorkspace()
const { t, relativeTime } = useI18n()
const filter = ref<'all' | 'unread'>('all')

const list = computed(() => (notifications.value ?? []).filter((n) => filter.value === 'all' || !n.read_at))
const icons: Record<string, any> = { mention: AtSign, assigned: UserPlus, comment: MessageSquare, due_soon: CalendarClock, completed: CircleCheck, mail: Mail }

function targetLink(n: Notification): string {
  const [kind, id] = n.target.split(':')
  if (kind === 'task' && n.data?.board_id) return links.task(wsId.value, n.data.board_id, id)
  if (kind === 'task') return links.taskById(wsId.value, id)
  if (kind === 'page') return links.doc(wsId.value, id) + (n.data?.comment_id ? `?comment=${n.data.comment_id}` : '')
  if (kind === 'event') return `${links.calendar(wsId.value)}?event=${id}`
  if (kind === 'board') return links.board(wsId.value, id)
  if (kind === 'mail') return links.mail(wsId.value, id)
  return links.home(wsId.value)
}
function targetLabel(n: Notification): string {
  const [kind, id] = n.target.split(':')
  if (n.data?.title) return n.data.title
  if (kind === 'mail' && n.data?.count) return t('mail.newCount', { count: n.data.count })
  if (kind === 'page') return pages.data.value?.find((p) => p.id === id)?.title || t('docs.untitled')
  if (kind === 'board') return boards.data.value?.find((b) => b.id === id)?.name || ''
  return ''
}

function open(n: Notification) {
  if (!n.read_at) markRead(n)
  navigateTo(targetLink(n))
}

// Notifications navigateur
const permission = ref(typeof window !== 'undefined' && 'Notification' in window ? window.Notification.permission : 'denied')
async function enableDesktop() {
  permission.value = await window.Notification.requestPermission()
}

const root = ref<HTMLElement | null>(null)
watch([list, root], () => nextTick(() => root.value && motion.stagger(root.value.querySelectorAll('.n-row'), { y: 4, each: 0.015 })), { once: true })

useHead({ title: () => t('nav.inbox') })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: t('nav.inbox'), icon: Inbox }]">
      <UiSegmented v-model="filter" :options="[{ value: 'all', label: t('inbox.all') }, { value: 'unread', label: `${t('inbox.unread')}${unread ? ` · ${unread}` : ''}` }]" />
      <UiTooltip :text="t('inbox.markAllRead')">
        <UiButton variant="ghost" size="sm" :icon="CheckCheck" :disabled="!unread" @click="markAllRead" />
      </UiTooltip>
    </AppPageHeader>
  
    <div ref="root" class="inbox">
      <div v-if="permission === 'default'" class="desktop-banner">
        <BellRing :size="16" />
        <span>{{ t('inbox.enableDesktop') }}</span>
        <UiButton size="xs" variant="primary" @click="enableDesktop">{{ t('common.enable') }}</UiButton>
      </div>
  
      <div v-if="isLoading" class="list"><div v-for="i in 6" :key="i" class="skeleton" style="height: 52px; margin: 6px 0" /></div>
      <UiEmpty v-else-if="!list.length" :icon="Bell" :title="filter === 'unread' ? t('inbox.allCaughtUp') : t('inbox.empty')" :description="t('inbox.emptyHint')" />
      <div v-else class="list">
        <button v-for="n in list" :key="n.id" class="n-row" :class="{ unread: !n.read_at }" @click="open(n)">
          <span class="n-avatar">
            <UiAvatar :name="n.actor_name" :src="member(n.actor_id)?.avatar_url" :size="28" />
            <span class="n-kind"><component :is="icons[n.kind] ?? Bell" :size="10" /></span>
          </span>
          <span class="n-body">
            <span class="n-title truncate">{{ targetLabel(n) }}</span>
            <span class="n-desc truncate">{{ t(`inbox.kinds.${n.kind}`, { name: n.actor_name || '?', title: '' }) }}</span>
          </span>
          <time class="n-time">{{ relativeTime(n.created_at) }}</time>
          <UiTooltip :text="n.read_at ? t('inbox.markUnread') : t('inbox.markRead')">
            <span class="n-dot" role="button" @click.stop="markRead(n, !n.read_at)"><Circle :size="8" :fill="n.read_at ? 'none' : 'currentColor'" /></span>
          </UiTooltip>
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.inbox { flex: 1; overflow-y: auto; padding: 12px 16px 48px; }
.list, .desktop-banner { max-width: 820px; margin: 0 auto; }
.desktop-banner {
  display: flex; align-items: center; gap: 10px; padding: 10px 12px; margin-bottom: 12px;
  border-radius: var(--r-md); background: var(--accent-soft); color: var(--accent-text); font-size: var(--fs-md);
}
.desktop-banner span { flex: 1; }
.n-row {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  padding: 10px 10px;
  border-radius: var(--r-md);
  text-align: left;
  transition: background var(--dur-fast);
}
.n-row:hover { background: var(--bg-hover); }
.n-avatar { position: relative; flex: none; }
.n-kind {
  position: absolute; right: -3px; bottom: -3px; width: 16px; height: 16px; border-radius: 50%;
  display: grid; place-items: center; background: var(--bg-elevated); color: var(--text-2);
  box-shadow: 0 0 0 2px var(--bg);
}
.n-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
.n-title { font-weight: 500; color: var(--text-2); }
.n-row.unread .n-title { color: var(--text); font-weight: 600; }
.n-desc { font-size: var(--fs-sm); color: var(--text-3); }
.n-time { flex: none; font-size: var(--fs-sm); color: var(--text-3); }
.n-dot { width: 20px; height: 20px; display: grid; place-items: center; color: var(--accent); border-radius: 50%; }
.n-row:not(.unread) .n-dot { color: var(--text-4); opacity: 0; }
.n-row:hover .n-dot { opacity: 1; }
@media (max-width: 639px) { .inbox { padding: 8px 8px 32px; } }
</style>
