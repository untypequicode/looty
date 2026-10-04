<script setup lang="ts">
import { Activity, FileText, CircleDot, MessageSquare, Users, ChevronDown, Check } from 'lucide-vue-next'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { useWorkspace } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { presence } from '~/composables/useRealtime'
import { useActivityFeed, feedVerb, type FeedItem } from '~/composables/useActivityFeed'

definePageMeta({ layout: 'workspace' })

const route = useRoute()
const router = useRouter()
const { wsId, members, member } = useWorkspace()
const { t, relativeTime, locale } = useI18n()
const user = computed<string | null>(() => (route.query.user as string) || null)
const feed = useActivityFeed(wsId, user)
const items = computed(() => (feed.data.value?.pages ?? []).flatMap((p) => p.items))
const who = computed(() => (user.value ? member(user.value) : null))

const days = computed(() => {
  const out: { key: string; label: string; items: FeedItem[] }[] = []
  const today = new Date().toDateString()
  const yesterday = new Date(Date.now() - 86400000).toDateString()
  for (const i of items.value) {
    const d = new Date(i.created_at)
    const key = d.toDateString()
    let g = out[out.length - 1]
    if (!g || g.key !== key) {
      const label = key === today ? t('date.today') : key === yesterday ? t('date.yesterday') : d.toLocaleDateString(locale.value, { weekday: 'long', day: 'numeric', month: 'long' })
      out.push((g = { key, label, items: [] }))
    }
    g.items.push(i)
  }
  return out
})

const filterOpen = ref(false)
const filterItems = computed<MenuItem[]>(() => [
  { label: t('feed.everyone'), icon: Users, checked: !user.value, onSelect: () => router.replace({ query: {} }) },
  { divider: true },
  ...(members.data.value ?? []).map((m) => ({ label: presence.value[m.user_id] ? `${m.name} ·  ${t('presence.status.active').toLowerCase()}` : m.name, checked: user.value === m.user_id, onSelect: () => router.replace({ query: { user: m.user_id } }) })),
])

const icon = (i: FeedItem) => (i.source === 'page' ? FileText : i.source === 'comment' ? MessageSquare : CircleDot)
function link(i: FeedItem): string | null {
  if (i.source === 'page') return i.type === 'delete' ? `/w/${wsId.value}/trash` : `/w/${wsId.value}/doc/${i.target_id}`
  return i.board_id ? `/w/${wsId.value}/board/${i.board_id}?task=${i.target_id}` : null
}
const time = (iso: string) => new Date(iso).toLocaleTimeString(locale.value, { hour: '2-digit', minute: '2-digit' })
useHead({ title: () => t('feed.title') })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: t('feed.title'), icon: Activity }]">
      <UiMenu v-model:open="filterOpen" :items="filterItems" placement="bottom-end" :width="260">
        <UiButton size="sm" variant="ghost">
          <UiAvatar v-if="who" :name="who.name" :src="who.avatar_url" :size="18" />
          {{ who ? who.name : t('feed.everyone') }} <ChevronDown :size="14" />
        </UiButton>
      </UiMenu>
    </AppPageHeader>
    <div class="af">
      <div class="af-inner">
        <div v-if="feed.isLoading.value"><div v-for="i in 8" :key="i" class="skeleton" style="height: 40px; margin: 6px 0" /></div>
        <UiEmpty v-else-if="!items.length" :icon="Activity" :title="t('feed.empty')" />
        <section v-for="d in days" :key="d.key" class="af-day">
          <h3 class="af-day-title">{{ d.label }}</h3>
          <component :is="link(i) ? 'NuxtLink' : 'div'" v-for="i in d.items" :key="i.id" :to="link(i) ?? undefined" class="af-row">
            <span class="af-av">
              <UiAvatar :name="member(i.user_id)?.name || i.user_name || '?'" :src="member(i.user_id)?.avatar_url" :size="26" />
              <span class="af-kind"><component :is="icon(i)" :size="10" /></span>
            </span>
            <span class="af-body">
              <span class="af-line">
                <b>{{ member(i.user_id)?.name || i.user_name || t('trash.someone') }}</b>
                <span class="af-verb">{{ feedVerb(i) }}</span>
                <span class="af-target"><span v-if="i.icon" class="af-emoji">{{ i.icon }}</span>{{ i.title || t('docs.untitled') }}</span>
              </span>
              <span v-if="i.snippet" class="af-snippet">“{{ i.snippet }}”</span>
              <span v-if="i.board_name" class="af-meta">{{ i.board_name }}</span>
            </span>
            <time class="af-time" :title="relativeTime(i.created_at)">{{ time(i.created_at) }}</time>
          </component>
        </section>
        <div v-if="feed.hasNextPage.value" class="af-more">
          <UiButton size="sm" variant="ghost" :loading="feed.isFetchingNextPage.value" @click="feed.fetchNextPage()">{{ t('feed.more') }}</UiButton>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.af { flex: 1; overflow-y: auto; padding: 8px 16px 48px; }
.af-inner { max-width: 820px; margin: 0 auto; }
.af-day { margin-top: 16px; }
.af-day-title { margin: 0 0 4px; padding: 0 10px; font-size: var(--fs-sm); font-weight: 600; color: var(--text-2); text-transform: capitalize; }
.af-row { display: flex; align-items: flex-start; gap: 12px; padding: 8px 10px; border-radius: var(--r-md); }
a.af-row:hover { background: var(--bg-hover); }
.af-av { position: relative; flex: none; }
.af-kind { position: absolute; right: -3px; bottom: -3px; width: 15px; height: 15px; border-radius: 50%; display: grid; place-items: center; background: var(--bg-elevated); color: var(--text-2); box-shadow: 0 0 0 2px var(--bg); }
.af-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
.af-line { display: flex; flex-wrap: wrap; gap: 0 5px; align-items: baseline; font-size: var(--fs-md); }
.af-verb { color: var(--text-3); }
.af-target { font-weight: 500; color: var(--text); }
.af-emoji { margin-right: 4px; }
.af-snippet { font-size: var(--fs-sm); color: var(--text-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.af-meta { font-size: var(--fs-sm); color: var(--text-3); }
.af-time { flex: none; font-size: var(--fs-sm); color: var(--text-3); padding-top: 2px; }
.af-more { display: flex; justify-content: center; padding: 12px; }
</style>
