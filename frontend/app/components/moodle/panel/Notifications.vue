<script setup lang="ts">
import { Bell } from 'lucide-vue-next'
import { errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useMoodleData, type MoodleNotification } from '~/composables/useMoodle'

/** Panneau « Notifications » : les dernières notifications Moodle, ouvertes sur le site. */
const props = defineProps<{ ws: string }>()
const emit = defineEmits<{ (e: 'open', url: string): void }>()
const { t, relativeTime } = useI18n()
const notifs = useMoodleData<{ notifications: MoodleNotification[]; unread: number }>(toRef(props, 'ws'), 'notifications', ref(true), 120_000)
</script>

<template>
  <div>
    <div v-if="notifs.isLoading.value"><div v-for="i in 3" :key="i" class="skeleton" style="height: 40px; margin: 4px 0" /></div>
    <p v-else-if="notifs.error.value" class="mp-err">{{ errorMessage(notifs.error.value) }}</p>
    <p v-else-if="!notifs.data.value?.notifications.length" class="mp-empty">{{ t('moodle.noNotifications') }}</p>
    <component :is="n.url ? 'a' : 'div'" v-for="n in notifs.data.value?.notifications" :key="n.id" class="nt" :class="{ unread: !n.read }" :href="n.url || undefined" target="_blank" rel="noopener" @click="n.url && ($event.preventDefault(), emit('open', n.url))">
      <Bell :size="13" class="nt-ic" />
      <span class="nt-body"><b>{{ n.subject }}</b><span v-if="n.text && n.text !== n.subject" class="nt-text truncate">{{ n.text }}</span></span>
      <time>{{ relativeTime(n.time) }}</time>
    </component>
  </div>
</template>

<style scoped>
.nt { display: flex; align-items: flex-start; gap: 8px; padding: 7px 6px; border-radius: var(--r-md); color: var(--text); text-decoration: none; }
.nt:hover { background: var(--bg-hover); }
.nt-ic { color: var(--text-3); margin-top: 3px; flex: none; }
.nt.unread .nt-ic { color: var(--accent); }
.nt-body { flex: 1; min-width: 0; display: flex; flex-direction: column; font-size: var(--fs-sm); }
.nt-body b { font-weight: 500; }
.nt.unread .nt-body b { font-weight: 600; }
.nt-text { color: var(--text-3); }
.nt time { font-size: var(--fs-xs); color: var(--text-3); flex: none; }
</style>
