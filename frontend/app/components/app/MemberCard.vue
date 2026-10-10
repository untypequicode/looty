<script setup lang="ts">
import { Eye, EyeOff, LogIn, Activity, Clock } from 'lucide-vue-next'
import type { WorkspaceMember, Presence } from '~/types'
import { useWorkspace } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { useLocationInfo, following } from '~/composables/usePresenceInfo'
import { useAuth } from '~/composables/useAuth'
import { useActivityFeed, feedVerb } from '~/composables/useActivityFeed'

const props = defineProps<{ member: WorkspaceMember; presence?: Presence | null }>()
const emit = defineEmits<{ close: [] }>()
const { wsId } = useWorkspace()
const { t, relativeTime } = useI18n()

const loc = useLocationInfo(computed(() => props.presence?.location))
const status = computed(() => (!props.presence ? 'offline' : props.presence.status === 'idle' ? 'idle' : 'active'))
const since = computed(() => (props.presence?.since ? relativeTime(new Date(props.presence.since).toISOString()) : ''))
const feed = useActivityFeed(wsId, computed(() => props.member.user_id), 5)
const recent = computed(() => feed.data.value?.pages[0]?.items ?? [])
const isFollowing = computed(() => following.value === props.member.user_id)
const { user } = useAuth()
const followsMe = computed(() => !!props.presence?.following && props.presence.following === user.value?.id)

function join() {
  if (loc.value?.path) navigateTo(loc.value.path)
  emit('close')
}
function toggleFollow() {
  following.value = isFollowing.value ? null : props.member.user_id
  emit('close')
}
function itemLink(i: { source: string; target_id: string; board_id?: string | null }) {
  return i.source === 'page' ? `/w/${wsId.value}/doc/${i.target_id}` : i.board_id ? `/w/${wsId.value}/board/${i.board_id}?task=${i.target_id}` : `/w/${wsId.value}/task/${i.target_id}`
}
</script>

<template>
  <div class="mc">
    <div class="mc-head">
      <UiAvatar :name="member.name" :src="member.avatar_url" :size="40" />
      <div class="mc-id">
        <div class="mc-name truncate">{{ member.name }} <span v-if="member.role === 'guest'" class="mc-guest">{{ t('pagePerms.guest') }}</span></div>
        <div class="mc-status" :class="status"><span class="mc-dot" />{{ t(`presence.status.${status}`) }}<span v-if="followsMe" class="mc-follows"><Eye :size="11" /> {{ t('presence.followsYou') }}</span></div>
      </div>
    </div>
    <div v-if="presence && loc" class="mc-where">
      <span class="mc-where-lab">{{ t('presence.currently') }}</span>
      <component :is="loc.path ? 'button' : 'span'" class="mc-loc" @click="loc.path && join()">
        <span v-if="'emoji' in loc && loc.emoji" class="mc-emoji">{{ loc.emoji }}</span><component :is="loc.icon" v-else :size="14" />
        <span class="truncate">{{ loc.label }}</span>
      </component>
      <span v-if="since" class="mc-since"><Clock :size="11" /> {{ since }}</span>
    </div>
    <div class="mc-actions">
      <UiButton v-if="presence && loc?.path" size="sm" :icon="LogIn" @click="join">{{ t('presence.join') }}</UiButton>
      <UiButton v-if="presence && presence.location !== 'hidden'" size="sm" :variant="isFollowing ? 'primary' : 'ghost'" :icon="isFollowing ? EyeOff : Eye" @click="toggleFollow">
        {{ isFollowing ? t('presence.stopFollowing') : t('presence.follow') }}
      </UiButton>
      <UiButton size="sm" variant="ghost" :icon="Activity" :to="`/w/${wsId}/activity?user=${member.user_id}`" @click="emit('close')">{{ t('presence.activity') }}</UiButton>
    </div>
    <div class="mc-recent">
      <div class="mc-recent-head">{{ t('presence.recent') }}</div>
      <div v-if="feed.isLoading.value" class="skeleton" style="height: 60px" />
      <p v-else-if="!recent.length" class="mc-none">{{ t('presence.noActivity') }}</p>
      <NuxtLink v-for="i in recent" :key="i.id" :to="itemLink(i)" class="mc-item" @click="emit('close')">
        <span class="mc-verb">{{ feedVerb(i) }}</span>
        <span class="mc-target truncate">{{ i.title || t('docs.untitled') }}</span>
        <time class="mc-time">{{ relativeTime(i.created_at) }}</time>
      </NuxtLink>
    </div>
  </div>
</template>

<style scoped>
.mc { display: flex; flex-direction: column; gap: 12px; padding: 14px; }
.mc-head { display: flex; align-items: center; gap: 12px; }
.mc-id { min-width: 0; }
.mc-name { font-weight: 600; font-size: 15px; }
.mc-guest { margin-left: 4px; padding: 0 6px; border-radius: var(--r-full); background: var(--bg-hover); color: var(--text-3); font-size: var(--fs-xs); font-weight: 500; vertical-align: middle; }
.mc-follows { display: inline-flex; align-items: center; gap: 3px; margin-left: 4px; padding: 0 6px; border-radius: var(--r-full); background: var(--accent-soft); color: var(--accent-text); font-size: var(--fs-xs); }
.mc-status { display: flex; align-items: center; gap: 6px; font-size: var(--fs-sm); color: var(--text-3); }
.mc-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--text-3); }
.mc-status.active .mc-dot { background: var(--green, #4cb782); }
.mc-status.idle .mc-dot { background: var(--orange, #e2a03f); }
.mc-where { display: flex; flex-direction: column; gap: 4px; }
.mc-where-lab { font-size: var(--fs-xs, 11px); color: var(--text-3); text-transform: uppercase; letter-spacing: .03em; font-weight: 600; }
.mc-loc { display: flex; align-items: center; gap: 8px; padding: 7px 9px; border-radius: var(--r-md); background: var(--bg-hover); text-align: left; min-width: 0; color: var(--text); }
button.mc-loc:hover { background: var(--bg-active, var(--bg-hover)); box-shadow: inset 0 0 0 1px var(--border-strong); }
.mc-emoji { font-size: 14px; }
.mc-since { display: flex; align-items: center; gap: 4px; font-size: var(--fs-sm); color: var(--text-3); }
.mc-actions { display: flex; flex-wrap: wrap; gap: 6px; }
.mc-recent { border-top: 1px solid var(--border); padding-top: 10px; display: flex; flex-direction: column; gap: 2px; }
.mc-recent-head { font-size: var(--fs-xs, 11px); color: var(--text-3); text-transform: uppercase; letter-spacing: .03em; font-weight: 600; margin-bottom: 4px; }
.mc-none { margin: 0; color: var(--text-3); font-size: var(--fs-sm); }
.mc-item { display: flex; align-items: baseline; gap: 6px; padding: 4px 6px; margin: 0 -6px; border-radius: var(--r-sm); font-size: var(--fs-sm); min-width: 0; }
.mc-item:hover { background: var(--bg-hover); }
.mc-verb { color: var(--text-3); flex: none; }
.mc-target { flex: 1; min-width: 0; color: var(--text); }
.mc-time { color: var(--text-3); flex: none; font-size: 11px; }
</style>
