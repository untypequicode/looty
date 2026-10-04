<script setup lang="ts">
import { Sparkles, Globe, Monitor, Smartphone, Download, ExternalLink } from 'lucide-vue-next'
import { useI18n } from '~/composables/useI18n'
import { useReleases, noteHtml, newer } from '~/composables/useReleases'

definePageMeta({ layout: 'workspace' })

const { t, locale } = useI18n()
const { data, client, appUpdate } = useReleases()
const filter = ref<'all' | 'web' | 'desktop' | 'android'>(client.platform === 'web' ? 'all' : client.platform)
const lang = computed(() => (locale.value === 'fr' ? 'fr' : 'en'))
const icon = { web: Globe, desktop: Monitor, android: Smartphone }
const list = computed(() => (data.value?.releases ?? []).filter((r) => filter.value === 'all' || r.platform === filter.value))
const date = (d: string) => new Date(d).toLocaleDateString(locale.value, { day: 'numeric', month: 'long', year: 'numeric' })
const installed = (p: string, v: string) => (p === 'web' ? v === data.value?.server_version : p === client.platform && v === client.version)
useHead({ title: () => t('releases.whatsNew') })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: t('releases.whatsNew'), icon: Sparkles }]">
      <UiSegmented v-model="filter" :options="[{ value: 'all', label: t('releases.all') }, { value: 'web', label: t('releases.platform.web') }, { value: 'desktop', label: t('releases.platform.desktop') }, { value: 'android', label: t('releases.platform.android') }]" />
    </AppPageHeader>
    <div class="wp">
      <div class="wp-inner">
        <div v-if="data" class="wp-versions">
          <div class="wv"><Globe :size="15" /><span>{{ t('releases.platform.web') }}</span><b>{{ data.server_version }}</b></div>
          <div v-for="p in (['desktop', 'android'] as const)" :key="p" class="wv">
            <component :is="icon[p]" :size="15" /><span>{{ t(`releases.platform.${p}`) }}</span>
            <b v-if="data.latest[p]">{{ data.latest[p]!.version }}</b>
            <small v-if="client.platform === p && client.version" :class="{ old: appUpdate }">{{ t('releases.installed', { v: client.version }) }}</small>
            <a v-if="data.latest[p]" :href="data.latest[p]!.url" target="_blank" rel="noopener" class="wv-dl"><Download :size="13" /> {{ t('releases.download') }}</a>
          </div>
        </div>
        <article v-for="r in list" :key="r.platform + r.version" class="wr">
          <header class="wr-head">
            <span class="wr-ic"><component :is="icon[r.platform]" :size="14" /></span>
            <div class="wr-titles">
              <h2>{{ r.title[lang] }}</h2>
              <span class="wr-meta">{{ t(`releases.platform.${r.platform}`) }} {{ r.version }} · {{ date(r.date) }}</span>
            </div>
            <span v-if="installed(r.platform, r.version)" class="wr-badge">{{ t('releases.current') }}</span>
            <span v-else-if="r.platform === client.platform && client.version && newer(r.version, client.version)" class="wr-badge new">{{ t('releases.updateShort') }}</span>
          </header>
          <ul>
            <!-- eslint-disable-next-line vue/no-v-html -->
            <li v-for="(n, i) in r.notes[lang]" :key="i" v-html="noteHtml(n)" />
          </ul>
          <a v-if="r.platform !== 'web' && data" :href="`${data.repository}/releases/tag/${r.platform}-v${r.version}`" target="_blank" rel="noopener" class="wr-link"><ExternalLink :size="12" /> {{ t('releases.releasePage') }}</a>
        </article>
      </div>
    </div>
  </div>
</template>

<style scoped>
.wp { flex: 1; overflow-y: auto; padding: 16px 16px 48px; }
.wp-inner { max-width: 760px; margin: 0 auto; display: flex; flex-direction: column; gap: 16px; }
.wp-versions { display: flex; flex-direction: column; gap: 6px; padding: 12px 14px; border-radius: var(--r-md); box-shadow: inset 0 0 0 1px var(--border); }
.wv { display: flex; align-items: center; gap: 8px; font-size: var(--fs-md); color: var(--text-2); }
.wv b { color: var(--text); font-variant-numeric: tabular-nums; }
.wv small { color: var(--text-3); }
.wv small.old { color: var(--orange, #e2a03f); }
.wv-dl { margin-left: auto; display: inline-flex; align-items: center; gap: 4px; font-size: var(--fs-sm); color: var(--accent); }
.wr { padding: 16px 0; border-top: 1px solid var(--border); }
.wr-head { display: flex; align-items: center; gap: 10px; }
.wr-ic { width: 28px; height: 28px; border-radius: 7px; display: grid; place-items: center; background: var(--accent-soft); color: var(--accent); flex: none; }
.wr-titles { flex: 1; min-width: 0; }
.wr-titles h2 { margin: 0; font-size: 16px; font-weight: 600; }
.wr-meta { font-size: var(--fs-sm); color: var(--text-3); }
.wr-badge { font-size: 11px; font-weight: 600; padding: 2px 8px; border-radius: var(--r-full); background: var(--bg-hover); color: var(--text-2); }
.wr-badge.new { background: var(--accent); color: #fff; }
ul { margin: 10px 0 0; padding-left: 22px; display: flex; flex-direction: column; gap: 6px; color: var(--text-2); line-height: 1.55; }
:deep(code) { font-size: 12px; padding: 1px 5px; border-radius: 4px; background: var(--bg-hover); }
.wr-link { display: inline-flex; align-items: center; gap: 4px; margin-top: 8px; font-size: var(--fs-sm); color: var(--text-3); }
.wr-link:hover { color: var(--accent); }
</style>
